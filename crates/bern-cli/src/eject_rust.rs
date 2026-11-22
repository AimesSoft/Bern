//! Offline self-contained Rust workspace bundler for `bern eject --engine source`.
//!
//! Collects every local crate reachable from the ejected target's Rust entry
//! manifest(s) — application protocol, engine, optional modules, helper local
//! crates, and the exact subset of Bern runtime crates used by the target —
//! copies them into the ejected project directory, resolves all
//! `workspace = true` inheritances, rewrites every local path to clean relative
//! paths, and strips external `dev-dependencies` so the ejected project builds
//! forever without the original Bern checkout or workspace.

use bern_compiler::Config;
use bern_target_api::Framework;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::{Component, Path, PathBuf};
use std::process::Command;

/// Result of offline Rust workspace bundling.
pub struct BundleReport {
    pub app_crates: usize,
    pub bern_crates: usize,
    pub notes: Vec<String>,
}

/// Cached `[workspace]` inheritance tables loaded from a Cargo workspace root.
#[derive(Clone, Default)]
struct WorkspaceTables {
    root: PathBuf,
    package: toml::map::Map<String, toml::Value>,
    dependencies: toml::map::Map<String, toml::Value>,
}

#[derive(Clone)]
struct CollectedCrate {
    /// Canonical source directory of the original crate.
    orig_dir: PathBuf,
    /// Destination directory inside the ejected project (absolute).
    dest_dir: PathBuf,
    /// Whether this crate belongs to the Bern runtime (`vendor/bern/...`).
    is_bern: bool,
}

/// Bundle every local crate and Bern runtime crate referenced by
/// `entry_manifests` (relative to `ejected_dir`) into an offline, self-contained
/// Rust layout under `ejected_dir.join(rust_root)`.
pub fn bundle_offline_workspace(
    ejected_dir: &Path,
    rust_root_rel: &Path,
    entry_manifests: &[PathBuf],
    config: &Config,
    workspace_dir: &Path,
    framework: &Framework,
) -> Result<BundleReport, String> {
    if entry_manifests.is_empty() {
        return Ok(BundleReport {
            app_crates: 0,
            bern_crates: 0,
            notes: Vec::new(),
        });
    }

    let ejected_canon = canonical_or_abs(ejected_dir);
    let rust_root = if rust_root_rel == Path::new(".") || rust_root_rel.as_os_str().is_empty() {
        ejected_canon.clone()
    } else {
        ejected_canon.join(rust_root_rel)
    };
    let project_dir = canonical_or_abs(&config.dir);
    let bern_crates_root = match framework {
        Framework::Path(dir) => Some(canonical_or_abs(dir)),
        _ => None,
    };

    // If the project consumes Bern via Git or crates.io, opportunistically ask
    // `cargo metadata` for already-cached Bern runtime crate directories so
    // they are vendored offline as well.
    let mut cached_bern_sources: BTreeMap<String, PathBuf> = BTreeMap::new();
    let mut notes = Vec::new();
    if bern_crates_root.is_none() {
        for manifest_rel in entry_manifests {
            let abs = ejected_canon.join(manifest_rel);
            if let Some(found) = discover_cached_bern_crates(&abs) {
                cached_bern_sources.extend(found);
            }
        }
        if cached_bern_sources.is_empty() {
            notes.push(
                "Bern runtime crates were not found in the local Cargo cache; their original git/registry entries were kept in Cargo.toml".into(),
            );
        }
    }

    let mut ws_cache: BTreeMap<PathBuf, Option<WorkspaceTables>> = BTreeMap::new();
    let mut collected: BTreeMap<PathBuf, CollectedCrate> = BTreeMap::new();
    let mut queue: VecDeque<PathBuf> = VecDeque::new();
    let mut used_crate_names: BTreeSet<String> = BTreeSet::new();

    // Seed BFS from every entry manifest written into `ejected_dir`.
    for manifest_rel in entry_manifests {
        let abs_manifest = ejected_canon.join(manifest_rel);
        let text = std::fs::read_to_string(&abs_manifest)
            .map_err(|e| format!("cannot read {}: {e}", abs_manifest.display()))?;
        let doc: toml::Value = text
            .parse()
            .map_err(|e| format!("invalid TOML in {}: {e}", abs_manifest.display()))?;
        let base_dir = abs_manifest.parent().unwrap_or(&ejected_canon);
        let deps = collect_dep_paths(&doc, base_dir, None, &cached_bern_sources);
        for dep_dir in deps {
            enqueue_crate(
                &dep_dir,
                &project_dir,
                bern_crates_root.as_deref(),
                &cached_bern_sources,
                &rust_root,
                &mut collected,
                &mut queue,
                &mut used_crate_names,
            )?;
        }
    }

    // Recursively follow all local path and workspace-inherited path dependencies.
    while let Some(orig_dir) = queue.pop_front() {
        let manifest_path = orig_dir.join("Cargo.toml");
        let text = std::fs::read_to_string(&manifest_path)
            .map_err(|e| format!("cannot read {}: {e}", manifest_path.display()))?;
        let doc: toml::Value = text
            .parse()
            .map_err(|e| format!("invalid TOML in {}: {e}", manifest_path.display()))?;
        let ws = find_workspace_tables(&orig_dir, &mut ws_cache);
        let deps = collect_dep_paths(&doc, &orig_dir, ws.as_ref(), &cached_bern_sources);
        for dep_dir in deps {
            // Skip crates that already live inside an already-collected crate
            // directory (such as `bern-gpui/vendor/gpui-pre`), because copying
            // the enclosing crate copies them automatically.
            if enclosing_collected(&dep_dir, &collected).is_some() {
                continue;
            }
            enqueue_crate(
                &dep_dir,
                &project_dir,
                bern_crates_root.as_deref(),
                &cached_bern_sources,
                &rust_root,
                &mut collected,
                &mut queue,
                &mut used_crate_names,
            )?;
        }
    }

    // Drop any collected crate that turned out to be nested inside another
    // collected crate (for example `bern-gpui/vendor/gpui-pre` discovered
    // before `bern-gpui`).
    let keys: Vec<PathBuf> = collected.keys().cloned().collect();
    for key in keys {
        let nested = collected
            .keys()
            .any(|other| other != &key && key.starts_with(other));
        if nested {
            collected.remove(&key);
        }
    }

    // Copy every collected crate directory (excluding build artifacts and VCS).
    for item in collected.values() {
        copy_crate_dir(&item.orig_dir, &item.dest_dir)?;
    }

    // Copy `Bern.toml` (and enabled module `Bern.toml` manifests) alongside
    // `rust_root/app/` so proc-macros (`#[bern::protocol]`, `#[bern::value]`,
    // `tr!`) find project languages and module metadata inside the ejected tree.
    let has_app_crates_in_project = collected
        .values()
        .any(|c| !c.is_bern && c.orig_dir.starts_with(&project_dir));
    if has_app_crates_in_project {
        copy_app_bern_manifests(&project_dir, &rust_root.join("app"), config)?;
    }

    // Rewrite every copied crate's `Cargo.toml`: inline `workspace = true`,
    // rewrite paths to relative paths, drop external dev-dependencies and
    // nested `[patch.crates-io]` / empty `[workspace]` tables.
    for item in collected.values() {
        let manifest_path = item.dest_dir.join("Cargo.toml");
        let text = std::fs::read_to_string(&manifest_path)
            .map_err(|e| format!("cannot read {}: {e}", manifest_path.display()))?;
        let mut doc: toml::Value = text
            .parse()
            .map_err(|e| format!("invalid TOML in {}: {e}", manifest_path.display()))?;
        let ws = find_workspace_tables(&item.orig_dir, &mut ws_cache);
        rewrite_manifest(
            &mut doc,
            &item.orig_dir,
            &item.dest_dir,
            ws.as_ref(),
            &collected,
            &cached_bern_sources,
            false,
        )?;
        let serialized = toml::to_string_pretty(&doc)
            .map_err(|e| format!("cannot serialize {}: {e}", manifest_path.display()))?;
        std::fs::write(&manifest_path, serialized)
            .map_err(|e| format!("cannot write {}: {e}", manifest_path.display()))?;
    }

    // Rewrite every entry manifest (`rust/glue/Cargo.toml`, root `Cargo.toml`, etc.)
    // so every local path points inside the ejected project using relative paths.
    for manifest_rel in entry_manifests {
        let abs_manifest = ejected_canon.join(manifest_rel);
        let entry_dir = abs_manifest.parent().unwrap_or(&ejected_canon);
        let text = std::fs::read_to_string(&abs_manifest)
            .map_err(|e| format!("cannot read {}: {e}", abs_manifest.display()))?;
        let mut doc: toml::Value = text
            .parse()
            .map_err(|e| format!("invalid TOML in {}: {e}", abs_manifest.display()))?;
        rewrite_manifest(
            &mut doc,
            entry_dir,
            entry_dir,
            None,
            &collected,
            &cached_bern_sources,
            true,
        )?;
        let serialized = format!(
            "# Ejected from Bern — edit freely.\n{}",
            toml::to_string_pretty(&doc)
                .map_err(|e| format!("cannot serialize {}: {e}", abs_manifest.display()))?
        );
        std::fs::write(&abs_manifest, serialized)
            .map_err(|e| format!("cannot write {}: {e}", abs_manifest.display()))?;

        // Copy `Cargo.lock` next to each entry workspace root if present.
        let src_lock = workspace_dir.join("Cargo.lock");
        let dst_lock = entry_dir.join("Cargo.lock");
        if src_lock.is_file() && !dst_lock.exists() {
            let _ = std::fs::copy(&src_lock, &dst_lock);
        }
    }

    // Copy `rust-toolchain.toml` if the workspace or Bern checkout specifies one.
    let toolchain_candidates = [
        Some(workspace_dir.join("rust-toolchain.toml")),
        bern_crates_root
            .as_ref()
            .and_then(|d| d.parent().map(|p| p.join("rust-toolchain.toml"))),
    ];
    if let Some(src_tc) = toolchain_candidates
        .into_iter()
        .flatten()
        .find(|p| p.is_file())
    {
        let _ = std::fs::create_dir_all(&rust_root);
        let _ = std::fs::copy(&src_tc, rust_root.join("rust-toolchain.toml"));
        if rust_root != ejected_canon {
            let _ = std::fs::copy(&src_tc, ejected_canon.join("rust-toolchain.toml"));
        }
    }

    let app_crates = collected.values().filter(|c| !c.is_bern).count();
    let bern_crates = collected.values().filter(|c| c.is_bern).count();

    Ok(BundleReport {
        app_crates,
        bern_crates,
        notes,
    })
}

fn canonical_or_abs(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| {
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir()
                .map(|cwd| cwd.join(path))
                .unwrap_or_else(|_| path.to_path_buf())
        }
    })
}

fn enclosing_collected<'a>(
    path: &Path,
    collected: &'a BTreeMap<PathBuf, CollectedCrate>,
) -> Option<&'a CollectedCrate> {
    collected
        .iter()
        .find(|(orig, _)| path == orig.as_path() || path.starts_with(orig))
        .map(|(_, item)| item)
}

#[allow(clippy::too_many_arguments)]
fn enqueue_crate(
    dep_dir: &Path,
    project_dir: &Path,
    bern_crates_root: Option<&Path>,
    cached_bern_sources: &BTreeMap<String, PathBuf>,
    rust_root: &Path,
    collected: &mut BTreeMap<PathBuf, CollectedCrate>,
    queue: &mut VecDeque<PathBuf>,
    used_crate_names: &mut BTreeSet<String>,
) -> Result<(), String> {
    let orig_dir = canonical_or_abs(dep_dir);
    if collected.contains_key(&orig_dir) {
        return Ok(());
    }
    let manifest = orig_dir.join("Cargo.toml");
    if !manifest.is_file() {
        return Err(format!(
            "local Rust dependency `{}` has no Cargo.toml",
            orig_dir.display()
        ));
    }
    let pkg_name = read_package_name(&manifest).unwrap_or_else(|| {
        orig_dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "crate".into())
    });

    let is_bern = bern_crates_root.is_some_and(|root| orig_dir.starts_with(root))
        || cached_bern_sources
            .values()
            .any(|cached| canonical_or_abs(cached) == orig_dir);

    let dest_dir = if is_bern {
        let folder = unique_folder(&pkg_name, used_crate_names);
        rust_root.join("vendor/bern").join(folder)
    } else if let Ok(rel) = orig_dir.strip_prefix(project_dir) {
        rust_root.join("app").join(rel)
    } else {
        let folder = unique_folder(&pkg_name, used_crate_names);
        rust_root.join("crates").join(folder)
    };

    collected.insert(
        orig_dir.clone(),
        CollectedCrate {
            orig_dir: orig_dir.clone(),
            dest_dir,
            is_bern,
        },
    );
    queue.push_back(orig_dir);
    Ok(())
}

fn unique_folder(base: &str, used: &mut BTreeSet<String>) -> String {
    let mut candidate = base.to_string();
    let mut idx = 2usize;
    while !used.insert(candidate.to_ascii_lowercase()) {
        candidate = format!("{base}-{idx}");
        idx += 1;
    }
    candidate
}

fn read_package_name(manifest: &Path) -> Option<String> {
    let text = std::fs::read_to_string(manifest).ok()?;
    let doc: toml::Value = text.parse().ok()?;
    doc.get("package")?
        .get("name")?
        .as_str()
        .map(str::to_string)
}

/// Search upward from `crate_dir` for the enclosing Cargo workspace root
/// defining `[workspace.package]` or `[workspace.dependencies]`.
fn find_workspace_tables(
    crate_dir: &Path,
    cache: &mut BTreeMap<PathBuf, Option<WorkspaceTables>>,
) -> Option<WorkspaceTables> {
    let mut current = Some(crate_dir.to_path_buf());
    while let Some(dir) = current {
        if let Some(cached) = cache.get(&dir) {
            if cached.is_some() {
                return cached.clone();
            }
        } else {
            let manifest = dir.join("Cargo.toml");
            let parsed = if manifest.is_file() {
                std::fs::read_to_string(&manifest)
                    .ok()
                    .and_then(|t| t.parse::<toml::Value>().ok())
                    .and_then(|doc| {
                        let ws = doc.get("workspace")?.as_table()?;
                        let pkg = ws
                            .get("package")
                            .and_then(toml::Value::as_table)
                            .cloned()
                            .unwrap_or_default();
                        let deps = ws
                            .get("dependencies")
                            .and_then(toml::Value::as_table)
                            .cloned()
                            .unwrap_or_default();
                        if pkg.is_empty() && deps.is_empty() {
                            None
                        } else {
                            Some(WorkspaceTables {
                                root: dir.clone(),
                                package: pkg,
                                dependencies: deps,
                            })
                        }
                    })
            } else {
                None
            };
            let found = parsed.is_some();
            cache.insert(dir.clone(), parsed.clone());
            if found {
                return parsed;
            }
        }
        current = dir.parent().map(Path::to_path_buf);
    }
    None
}

/// Extract all local dependency directories (and cached Bern crate directories)
/// from `dependencies`, `build-dependencies`, `target.*.dependencies`,
/// `target.*.build-dependencies`, and `patch.crates-io` (excluding
/// `dev-dependencies`).
fn collect_dep_paths(
    doc: &toml::Value,
    crate_dir: &Path,
    ws: Option<&WorkspaceTables>,
    cached_bern: &BTreeMap<String, PathBuf>,
) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut visit_table = |table: &toml::map::Map<String, toml::Value>| {
        for (dep_key, spec) in table {
            if let Some(resolved_dir) = resolve_dep_dir(dep_key, spec, crate_dir, ws, cached_bern) {
                out.push(resolved_dir);
            }
        }
    };
    for section in ["dependencies", "build-dependencies"] {
        if let Some(table) = doc.get(section).and_then(toml::Value::as_table) {
            visit_table(table);
        }
    }
    if let Some(targets) = doc.get("target").and_then(toml::Value::as_table) {
        for target_val in targets.values() {
            if let Some(target_tbl) = target_val.as_table() {
                for section in ["dependencies", "build-dependencies"] {
                    if let Some(table) = target_tbl.get(section).and_then(toml::Value::as_table) {
                        visit_table(table);
                    }
                }
            }
        }
    }
    if let Some(patch) = doc
        .get("patch")
        .and_then(toml::Value::as_table)
        .and_then(|p| p.get("crates-io"))
        .and_then(toml::Value::as_table)
    {
        visit_table(patch);
    }
    out
}

fn resolve_dep_dir(
    dep_key: &str,
    spec: &toml::Value,
    crate_dir: &Path,
    ws: Option<&WorkspaceTables>,
    cached_bern: &BTreeMap<String, PathBuf>,
) -> Option<PathBuf> {
    if let Some(table) = spec.as_table() {
        if let Some(p) = table.get("path").and_then(toml::Value::as_str) {
            return Some(canonical_or_abs(&crate_dir.join(p)));
        }
        if table.get("workspace").and_then(toml::Value::as_bool) == Some(true)
            && let Some(ws) = ws
            && let Some(ws_dep) = ws.dependencies.get(dep_key)
            && let Some(ws_tbl) = ws_dep.as_table()
            && let Some(p) = ws_tbl.get("path").and_then(toml::Value::as_str)
        {
            return Some(canonical_or_abs(&ws.root.join(p)));
        }
        let pkg = table
            .get("package")
            .and_then(toml::Value::as_str)
            .unwrap_or(dep_key);
        if let Some(cached) = cached_bern.get(pkg) {
            return Some(canonical_or_abs(cached));
        }
    } else if let Some(cached) = cached_bern.get(dep_key) {
        return Some(canonical_or_abs(cached));
    }
    None
}

/// Rewrite one `Cargo.toml` in place: expand workspace inheritance, rewrite
/// every path into the ejected directory using relative paths, and strip
/// external dev-dependencies.
fn rewrite_manifest(
    doc: &mut toml::Value,
    orig_dir: &Path,
    dest_dir: &Path,
    ws: Option<&WorkspaceTables>,
    collected: &BTreeMap<PathBuf, CollectedCrate>,
    cached_bern: &BTreeMap<String, PathBuf>,
    is_entry: bool,
) -> Result<(), String> {
    let Some(root) = doc.as_table_mut() else {
        return Ok(());
    };

    // 1. Expand `[package]` `*.workspace = true`.
    if let Some(pkg) = root.get_mut("package").and_then(toml::Value::as_table_mut) {
        let keys: Vec<String> = pkg.keys().cloned().collect();
        for key in keys {
            let inherits = pkg
                .get(&key)
                .and_then(toml::Value::as_table)
                .and_then(|t| t.get("workspace"))
                .and_then(toml::Value::as_bool)
                == Some(true);
            if !inherits {
                continue;
            }
            if key == "readme" {
                pkg.remove(&key);
                continue;
            }
            if let Some(val) = ws.and_then(|w| w.package.get(&key)).cloned() {
                pkg.insert(key, val);
            } else {
                pkg.remove(&key);
            }
        }
    }

    // 2. Remove `[lints] workspace = true` so ejected crates never fail on
    //    missing parent workspace lint tables.
    if root
        .get("lints")
        .and_then(toml::Value::as_table)
        .and_then(|t| t.get("workspace"))
        .and_then(toml::Value::as_bool)
        == Some(true)
    {
        root.remove("lints");
    }

    // 3. Rewrite normal and build dependencies.
    for section in ["dependencies", "build-dependencies"] {
        if let Some(table) = root.get_mut(section).and_then(toml::Value::as_table_mut) {
            rewrite_dep_table(table, orig_dir, dest_dir, ws, collected, cached_bern);
        }
    }

    // 4. Prune external/workspace dev-dependencies in vendored crates.
    if !is_entry
        && let Some(dev) = root
            .get_mut("dev-dependencies")
            .and_then(toml::Value::as_table_mut)
    {
        prune_dev_table(dev, orig_dir, dest_dir, ws, collected, cached_bern);
    }

    // 5. Rewrite target-specific dependency sections.
    if let Some(targets) = root.get_mut("target").and_then(toml::Value::as_table_mut) {
        for (_, target_val) in targets.iter_mut() {
            if let Some(target_tbl) = target_val.as_table_mut() {
                for section in ["dependencies", "build-dependencies"] {
                    if let Some(table) = target_tbl
                        .get_mut(section)
                        .and_then(toml::Value::as_table_mut)
                    {
                        rewrite_dep_table(table, orig_dir, dest_dir, ws, collected, cached_bern);
                    }
                }
                if !is_entry
                    && let Some(dev) = target_tbl
                        .get_mut("dev-dependencies")
                        .and_then(toml::Value::as_table_mut)
                {
                    prune_dev_table(dev, orig_dir, dest_dir, ws, collected, cached_bern);
                }
            }
        }
    }

    // 6. Rewrite `[patch.crates-io]` on entry manifests; strip on vendored crates
    //    (Cargo ignores `[patch]` in non-root manifests and warns about it).
    if is_entry {
        if let Some(patch) = root
            .get_mut("patch")
            .and_then(toml::Value::as_table_mut)
            .and_then(|p| p.get_mut("crates-io"))
            .and_then(toml::Value::as_table_mut)
        {
            rewrite_dep_table(patch, orig_dir, dest_dir, ws, collected, cached_bern);
        }
        // Keep `[workspace]` on entry manifests so the ejected project stays
        // isolated even if placed inside another Cargo workspace.
        if !root.contains_key("workspace") {
            root.insert(
                "workspace".into(),
                toml::Value::Table(toml::map::Map::new()),
            );
        }
    } else {
        root.remove("patch");
        root.remove("workspace");
    }

    Ok(())
}

fn rewrite_dep_table(
    table: &mut toml::map::Map<String, toml::Value>,
    orig_dir: &Path,
    dest_dir: &Path,
    ws: Option<&WorkspaceTables>,
    collected: &BTreeMap<PathBuf, CollectedCrate>,
    cached_bern: &BTreeMap<String, PathBuf>,
) {
    let keys: Vec<String> = table.keys().cloned().collect();
    for key in keys {
        let Some(spec) = table.get_mut(&key) else {
            continue;
        };
        let mut resolved_base_dir = orig_dir.to_path_buf();

        // Expand `workspace = true` first.
        if let Some(dep_tbl) = spec.as_table().cloned()
            && dep_tbl.get("workspace").and_then(toml::Value::as_bool) == Some(true)
            && let Some(ws) = ws
            && let Some(ws_dep) = ws.dependencies.get(&key)
        {
            let mut merged = match ws_dep {
                toml::Value::Table(t) => t.clone(),
                toml::Value::String(ver) => {
                    let mut m = toml::map::Map::new();
                    m.insert("version".into(), toml::Value::String(ver.clone()));
                    m
                }
                _ => toml::map::Map::new(),
            };
            if merged.contains_key("path") {
                resolved_base_dir = ws.root.clone();
            }
            for (k, v) in dep_tbl {
                if k == "workspace" {
                    continue;
                }
                if k == "features"
                    && let (Some(base_arr), Some(extra_arr)) = (
                        merged
                            .get_mut("features")
                            .and_then(toml::Value::as_array_mut),
                        v.as_array(),
                    )
                {
                    for item in extra_arr {
                        if !base_arr.contains(item) {
                            base_arr.push(item.clone());
                        }
                    }
                } else {
                    merged.insert(k, v);
                }
            }
            *spec = toml::Value::Table(merged);
        }

        // Convert cached Bern crates from git/registry to vendored local paths.
        let pkg_name = spec
            .as_table()
            .and_then(|t| t.get("package").and_then(toml::Value::as_str))
            .unwrap_or(&key)
            .to_string();
        if spec.as_table().is_none_or(|t| !t.contains_key("path"))
            && let Some(cached_dir) = cached_bern.get(&pkg_name)
        {
            let mut m = match spec.clone() {
                toml::Value::Table(t) => t,
                toml::Value::String(ver) => {
                    let mut map = toml::map::Map::new();
                    map.insert("version".into(), toml::Value::String(ver));
                    map
                }
                _ => toml::map::Map::new(),
            };
            m.remove("git");
            m.remove("branch");
            m.remove("tag");
            m.remove("rev");
            m.insert(
                "path".into(),
                toml::Value::String(cached_dir.display().to_string()),
            );
            *spec = toml::Value::Table(m);
            resolved_base_dir = PathBuf::new();
        }

        // Rewrite `path` to a clean relative path inside the ejected project.
        if let Some(dep_tbl) = spec.as_table_mut()
            && let Some(raw_path) = dep_tbl
                .get("path")
                .and_then(toml::Value::as_str)
                .map(str::to_string)
        {
            let abs = if Path::new(&raw_path).is_absolute() {
                canonical_or_abs(Path::new(&raw_path))
            } else {
                canonical_or_abs(&resolved_base_dir.join(&raw_path))
            };
            if let Some(mapped) = map_into_ejected(&abs, collected) {
                let rel = relative_path(dest_dir, &mapped);
                dep_tbl.insert(
                    "path".into(),
                    toml::Value::String(rel.to_string_lossy().replace('\\', "/")),
                );
                // Strip git keys if any remained alongside path.
                dep_tbl.remove("git");
                dep_tbl.remove("branch");
                dep_tbl.remove("tag");
                dep_tbl.remove("rev");
            }
        }
    }
}

fn prune_dev_table(
    table: &mut toml::map::Map<String, toml::Value>,
    orig_dir: &Path,
    dest_dir: &Path,
    ws: Option<&WorkspaceTables>,
    collected: &BTreeMap<PathBuf, CollectedCrate>,
    cached_bern: &BTreeMap<String, PathBuf>,
) {
    rewrite_dep_table(table, orig_dir, dest_dir, ws, collected, cached_bern);
    let to_remove: Vec<String> = table
        .iter()
        .filter_map(|(k, v)| {
            let tbl = v.as_table()?;
            if tbl.get("workspace").and_then(toml::Value::as_bool) == Some(true) {
                return Some(k.clone());
            }
            if let Some(p) = tbl.get("path").and_then(toml::Value::as_str) {
                // Remove dev-dependency paths that escape the vendored tree.
                let resolved = dest_dir.join(p);
                if !resolved.join("Cargo.toml").is_file() {
                    return Some(k.clone());
                }
            }
            None
        })
        .collect();
    for k in to_remove {
        table.remove(&k);
    }
}

/// Map an original canonical directory (either an exact collected crate or a
/// nested sub-crate such as `bern-gpui/vendor/gpui-pre`) to its destination
/// inside the ejected project.
fn map_into_ejected(orig: &Path, collected: &BTreeMap<PathBuf, CollectedCrate>) -> Option<PathBuf> {
    if let Some(item) = collected.get(orig) {
        return Some(item.dest_dir.clone());
    }
    for (root, item) in collected {
        if let Ok(sub) = orig.strip_prefix(root) {
            return Some(item.dest_dir.join(sub));
        }
    }
    None
}

/// Compute a portable relative path from directory `from` to directory `to`.
pub fn relative_path(from: &Path, to: &Path) -> PathBuf {
    let from_comps: Vec<Component<'_>> = from.components().collect();
    let to_comps: Vec<Component<'_>> = to.components().collect();
    let common = from_comps
        .iter()
        .zip(&to_comps)
        .take_while(|(a, b)| a == b)
        .count();
    let mut rel = PathBuf::new();
    for _ in common..from_comps.len() {
        rel.push("..");
    }
    for comp in &to_comps[common..] {
        rel.push(comp.as_os_str());
    }
    if rel.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        rel
    }
}

/// Copy one crate directory recursively while skipping build outputs, VCS
/// metadata, and nested ejected directories.
fn copy_crate_dir(from: &Path, to: &Path) -> Result<(), String> {
    std::fs::create_dir_all(to).map_err(|e| format!("cannot create {}: {e}", to.display()))?;
    let entries =
        std::fs::read_dir(from).map_err(|e| format!("cannot read {}: {e}", from.display()))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("cannot read entry in {}: {e}", from.display()))?;
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if matches!(
            name_str.as_ref(),
            "target" | ".git" | ".DS_Store" | "ejected" | "node_modules"
        ) || name_str.ends_with(".xcodeproj")
        {
            continue;
        }
        let src = entry.path();
        let dst = to.join(&name);
        let ty = entry
            .file_type()
            .map_err(|e| format!("cannot stat {}: {e}", src.display()))?;
        if ty.is_dir() {
            copy_crate_dir(&src, &dst)?;
        } else if ty.is_file() {
            std::fs::copy(&src, &dst)
                .map_err(|e| format!("cannot copy {} to {}: {e}", src.display(), dst.display()))?;
        }
    }
    Ok(())
}

/// Copy `Bern.toml` and enabled module manifests into `rust/app/` so
/// `bern-macros` (`#[bern::protocol]`, `#[bern::value]`, `tr!`) continues to
/// resolve project languages and module composition offline.
fn copy_app_bern_manifests(
    project_dir: &Path,
    dest_app_dir: &Path,
    config: &Config,
) -> Result<(), String> {
    let root_manifest = project_dir.join("Bern.toml");
    if !root_manifest.is_file() {
        return Ok(());
    }
    std::fs::create_dir_all(dest_app_dir)
        .map_err(|e| format!("cannot create {}: {e}", dest_app_dir.display()))?;
    let raw = std::fs::read_to_string(&root_manifest)
        .map_err(|e| format!("cannot read {}: {e}", root_manifest.display()))?;
    let mut doc: toml::Value = raw
        .parse()
        .map_err(|e| format!("cannot parse {}: {e}", root_manifest.display()))?;

    // Keep only enabled `[[modules]]` entries so excluded modules are not
    // expected on disk by `bern-macros`.
    let enabled_ids: BTreeSet<&str> = config.modules.iter().map(|m| m.id.as_str()).collect();
    if let Some(modules) = doc.get_mut("modules").and_then(toml::Value::as_array_mut) {
        modules.retain(|entry| {
            let Some(path) = entry.get("path").and_then(toml::Value::as_str) else {
                return false;
            };
            let mod_manifest = project_dir.join(path).join("Bern.toml");
            let id = std::fs::read_to_string(&mod_manifest)
                .ok()
                .and_then(|t| t.parse::<toml::Value>().ok())
                .and_then(|v| v.get("module")?.get("id")?.as_str().map(str::to_string));
            match id {
                Some(id) => enabled_ids.contains(id.as_str()),
                None => false,
            }
        });
    }
    let cleaned =
        toml::to_string_pretty(&doc).map_err(|e| format!("cannot serialize Bern.toml: {e}"))?;
    std::fs::write(dest_app_dir.join("Bern.toml"), cleaned)
        .map_err(|e| format!("cannot write {}/Bern.toml: {e}", dest_app_dir.display()))?;

    for module in &config.modules {
        if let Ok(rel) = canonical_or_abs(&module.dir).strip_prefix(project_dir) {
            let src = module.dir.join("Bern.toml");
            if src.is_file() {
                let dst_dir = dest_app_dir.join(rel);
                std::fs::create_dir_all(&dst_dir)
                    .map_err(|e| format!("cannot create {}: {e}", dst_dir.display()))?;
                std::fs::copy(&src, dst_dir.join("Bern.toml"))
                    .map_err(|e| format!("cannot copy {}: {e}", src.display()))?;
            }
        }
    }
    Ok(())
}

/// Ask `cargo metadata --offline` for already-cached `bern*` crate directories
/// when a project references Bern via Git or crates.io.
fn discover_cached_bern_crates(manifest: &Path) -> Option<BTreeMap<String, PathBuf>> {
    let output = Command::new("cargo")
        .args([
            "metadata",
            "--format-version",
            "1",
            "--offline",
            "--manifest-path",
        ])
        .arg(manifest)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let meta: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;
    let packages = meta.get("packages")?.as_array()?;
    let mut map = BTreeMap::new();
    for pkg in packages {
        let Some(name) = pkg.get("name").and_then(serde_json::Value::as_str) else {
            continue;
        };
        if name != "bern" && !name.starts_with("bern-") {
            continue;
        }
        let Some(manifest_path) = pkg.get("manifest_path").and_then(serde_json::Value::as_str)
        else {
            continue;
        };
        if let Some(dir) = Path::new(manifest_path).parent() {
            map.insert(name.to_string(), dir.to_path_buf());
        }
    }
    Some(map)
}
