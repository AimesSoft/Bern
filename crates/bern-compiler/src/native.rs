//! Co-located `NativeView` source resolution, validation and l10n linting.
//!
//! Each backend file property on a `NativeView` (`swiftui`, `winui3`,
//! `compose`, `gtk`, `gpui`, `web`) is written relative to the `.rs` file that
//! declares the `NativeView`. This pass resolves those paths on disk, reads
//! their contents into [`ir::native::NativeFileDef`], checks cross-view symbol
//! collisions, and warns on obvious hardcoded user-visible text that bypasses
//! `NativeText(content: tr! { ... })`.

use crate::config::{self, Config};
use bern_ir::{self as ir, SourceLocation, native};
use bern_target_api::Diagnostic;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};

/// Resolve co-located native UI files on every `NativeView` in `components`
/// and `root`, updating each `Prop::NativeFile { path }` to its normalized
/// project-relative path and returning the collected [`ir::native::NativeFileDef`]s.
pub fn resolve_and_lint(
    config: &Config,
    components: &mut [ir::ComponentDef],
    root: Option<&mut ir::Element>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<ir::native::NativeFileDef> {
    let source_map = build_source_map(config);
    let mut files: BTreeMap<String, ir::native::NativeFileDef> = BTreeMap::new();
    let mut swift_symbols: BTreeMap<String, SourceLocation> = BTreeMap::new();

    for component in components.iter_mut() {
        resolve_in_node(
            &config.dir,
            &source_map,
            &mut component.body,
            &mut files,
            &mut swift_symbols,
            diagnostics,
        );
    }
    if let Some(root) = root {
        resolve_in_element(
            &config.dir,
            &source_map,
            root,
            &mut files,
            &mut swift_symbols,
            diagnostics,
        );
    }

    files.into_values().collect()
}

fn build_source_map(config: &Config) -> BTreeMap<String, (PathBuf, PathBuf)> {
    // display path -> (disk path of .rs file, allowed root directory)
    let mut map = BTreeMap::new();
    for file in &config.ui_sources {
        map.insert(
            file.display.clone(),
            (file.path.clone(), config.dir.clone()),
        );
    }
    for module in &config.modules {
        for file in &module.ui_sources {
            map.insert(
                file.display.clone(),
                (file.path.clone(), config.dir.clone()),
            );
        }
    }
    for manifest in &config.libraries {
        if let Ok(lib) = config::load_library(manifest) {
            for file in lib.sources {
                let display = format!("{}/{}", lib.package.name, file.display);
                map.insert(display, (file.path, lib.dir.clone()));
            }
        }
    }
    map
}

fn resolve_in_node(
    project_dir: &Path,
    source_map: &BTreeMap<String, (PathBuf, PathBuf)>,
    node: &mut ir::Node,
    files: &mut BTreeMap<String, ir::native::NativeFileDef>,
    swift_symbols: &mut BTreeMap<String, SourceLocation>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match node {
        ir::Node::Element(element) => resolve_in_element(
            project_dir,
            source_map,
            element,
            files,
            swift_symbols,
            diagnostics,
        ),
        ir::Node::Component(_) => {}
        ir::Node::If(if_node) => {
            for (_, body) in &mut if_node.branches {
                for child in body {
                    resolve_in_node(
                        project_dir,
                        source_map,
                        child,
                        files,
                        swift_symbols,
                        diagnostics,
                    );
                }
            }
            for child in &mut if_node.otherwise {
                resolve_in_node(
                    project_dir,
                    source_map,
                    child,
                    files,
                    swift_symbols,
                    diagnostics,
                );
            }
        }
        ir::Node::IfLet(if_let) => {
            for child in &mut if_let.then {
                resolve_in_node(
                    project_dir,
                    source_map,
                    child,
                    files,
                    swift_symbols,
                    diagnostics,
                );
            }
            for child in &mut if_let.otherwise {
                resolve_in_node(
                    project_dir,
                    source_map,
                    child,
                    files,
                    swift_symbols,
                    diagnostics,
                );
            }
        }
        ir::Node::Match(m) => {
            for arm in &mut m.arms {
                for child in &mut arm.body {
                    resolve_in_node(
                        project_dir,
                        source_map,
                        child,
                        files,
                        swift_symbols,
                        diagnostics,
                    );
                }
            }
        }
    }
}

fn resolve_in_element(
    project_dir: &Path,
    source_map: &BTreeMap<String, (PathBuf, PathBuf)>,
    element: &mut ir::Element,
    files: &mut BTreeMap<String, ir::native::NativeFileDef>,
    swift_symbols: &mut BTreeMap<String, SourceLocation>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if element.kind == "NativeView" {
        if let Some(id) = element.id.clone()
            && element.props.contains_key("swiftui")
        {
            let type_name = format!("{}NativeView", native::pascal_ident(&id));
            if let Some(prev) = swift_symbols.insert(type_name.clone(), element.source.clone()) {
                diagnostics.push(
                    Diagnostic::error(format!(
                        "two `NativeView` elements with `swiftui` produce the same Swift view type `{type_name}` (first at {}:{}); use distinct `id` values",
                        prev.file, prev.line
                    ))
                    .at(element.source.clone()),
                );
            }
        }

        let owner_source = element.source.clone();
        for &backend in native::NATIVE_BACKENDS {
            let Some(ir::Prop::NativeFile { path: written }) = element.props.get(backend).cloned()
            else {
                continue;
            };
            let Some((rs_path, allowed_root)) = source_map.get(&owner_source.file) else {
                diagnostics.push(
                    Diagnostic::error(format!(
                        "cannot locate declaring source `{}` on disk to resolve `{backend}: \"{written}\"`",
                        owner_source.file
                    ))
                    .at(owner_source.clone()),
                );
                continue;
            };
            let Some(rs_dir) = rs_path.parent() else {
                continue;
            };
            let Some(resolved) = resolve_relative(rs_dir, &written, allowed_root) else {
                diagnostics.push(
                    Diagnostic::error(format!(
                        "`{backend}: \"{written}\"` must be a relative path inside the project"
                    ))
                    .at(owner_source.clone()),
                );
                continue;
            };
            let source = match std::fs::read_to_string(&resolved) {
                Ok(text) => text,
                Err(error) => {
                    diagnostics.push(
                        Diagnostic::error(format!(
                            "cannot read native view file `{written}` (resolved to {}): {error}",
                            resolved.display()
                        ))
                        .at(owner_source.clone()),
                    );
                    continue;
                }
            };
            let normalized =
                normalize_path(project_dir, allowed_root, &resolved, &owner_source.file);
            element.props.insert(
                backend.to_string(),
                ir::Prop::NativeFile {
                    path: normalized.clone(),
                },
            );
            if !files.contains_key(&normalized) {
                lint_hardcoded_text(backend, &normalized, &source, diagnostics);
                files.insert(
                    normalized.clone(),
                    ir::native::NativeFileDef {
                        path: normalized,
                        backend: backend.to_string(),
                        source,
                    },
                );
            }
        }
    }

    for child in &mut element.children {
        resolve_in_node(
            project_dir,
            source_map,
            child,
            files,
            swift_symbols,
            diagnostics,
        );
    }
}

fn resolve_relative(base_dir: &Path, written: &str, allowed_root: &Path) -> Option<PathBuf> {
    let rel = Path::new(written);
    if rel.is_absolute() {
        return None;
    }
    let mut joined = base_dir.to_path_buf();
    for comp in rel.components() {
        match comp {
            Component::CurDir => {}
            Component::Normal(part) => joined.push(part),
            Component::ParentDir => {
                if !joined.pop() {
                    return None;
                }
            }
            Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    let root = allowed_root
        .canonicalize()
        .unwrap_or_else(|_| allowed_root.to_path_buf());
    let candidate = joined.canonicalize().unwrap_or(joined);
    if !candidate.starts_with(&root) {
        return None;
    }
    Some(candidate)
}

fn normalize_path(
    project_dir: &Path,
    allowed_root: &Path,
    resolved: &Path,
    owner_display: &str,
) -> String {
    let project_root = project_dir
        .canonicalize()
        .unwrap_or_else(|_| project_dir.to_path_buf());
    if let Ok(rel) = resolved.strip_prefix(&project_root) {
        return path_to_slash(rel);
    }
    let lib_root = allowed_root
        .canonicalize()
        .unwrap_or_else(|_| allowed_root.to_path_buf());
    if let Ok(rel) = resolved.strip_prefix(&lib_root) {
        let prefix = owner_display.split('/').next().unwrap_or("library");
        return format!("{prefix}/{}", path_to_slash(rel));
    }
    path_to_slash(resolved)
}

fn path_to_slash(path: &Path) -> String {
    path.components()
        .filter_map(|c| match c {
            Component::Normal(part) => Some(part.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

/// Lightweight heuristic lint catching obvious hardcoded user-visible text
/// literals in co-located native files. Suppress a line with `bern-ignore-l10n`.
pub fn lint_hardcoded_text(
    backend: &str,
    path: &str,
    source: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let patterns: &[&str] = match backend {
        "swiftui" => &["Text(\"", "Label(\""],
        "winui3" => &[
            ".Text(L\"",
            ".Header(winrt::box_value(L\"",
            ".Header(box_value(L\"",
            ".Content(winrt::box_value(L\"",
            ".Content(box_value(L\"",
        ],
        "compose" => &["Text(\"", "Text(text = \""],
        "gtk" => &["Label::new(Some(\"", ".set_label(\"", ".set_text(\""],
        "gpui" => &[".child(\""],
        "web" => &[
            ".textContent = \"",
            ".textContent = '",
            ".innerText = \"",
            ".innerText = '",
        ],
        _ => &[],
    };
    if patterns.is_empty() {
        return;
    }

    let mut reported = BTreeSet::new();
    for (idx, line) in source.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("//")
            || trimmed.starts_with("/*")
            || trimmed.starts_with('*')
            || line.contains("bern-ignore-l10n")
        {
            continue;
        }
        for &pat in patterns {
            let Some(pos) = line.find(pat) else {
                continue;
            };
            let quote = pat.chars().last().unwrap_or('"');
            let rest = &line[pos + pat.len()..];
            let Some(lit) = extract_literal(rest, quote) else {
                continue;
            };
            if !looks_like_human_text(&lit) {
                continue;
            }
            let line_no = idx as u32 + 1;
            if reported.insert((line_no, lit.clone())) {
                diagnostics.push(
                    Diagnostic::warning(format!(
                        "hardcoded display text \"{lit}\" in native `{backend}` view; pass localized strings from Rust via `NativeText(name: ..., content: tr! {{ ... }})` (or suppress with `// bern-ignore-l10n`)"
                    ))
                    .at(SourceLocation {
                        file: path.to_string(),
                        line: line_no,
                        column: pos as u32 + 1,
                    }),
                );
            }
        }
    }
}

fn extract_literal(rest: &str, quote: char) -> Option<String> {
    let mut out = String::new();
    let mut escaped = false;
    for ch in rest.chars() {
        if escaped {
            out.push(ch);
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == quote {
            return Some(out);
        }
        out.push(ch);
    }
    None
}

fn looks_like_human_text(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return false;
    }
    // Swift string interpolation / JS template-like placeholders with only variables
    if trimmed.starts_with("\\(") && trimmed.ends_with(')') {
        return false;
    }
    // Purely numeric, punctuation or symbols (e.g. "%", "—", "100") are fine.
    trimmed.chars().any(char::is_alphabetic)
}

/// Reserved identifiers across Swift, C++, Kotlin, Rust and JS that cannot be
/// used as `NativeValue`/`NativeText`/`NativeDraft` names or entry ids.
pub fn is_reserved_native_ident(name: &str) -> bool {
    matches!(
        name,
        "props"
            | "actions"
            | "theme"
            | "effect"
            | "on_cleanup"
            | "self"
            | "Self"
            | "super"
            | "crate"
            | "fn"
            | "let"
            | "mut"
            | "type"
            | "match"
            | "if"
            | "else"
            | "for"
            | "while"
            | "loop"
            | "return"
            | "in"
            | "class"
            | "struct"
            | "enum"
            | "var"
            | "val"
            | "fun"
            | "func"
            | "const"
            | "static"
            | "public"
            | "private"
            | "default"
            | "switch"
            | "case"
            | "new"
            | "delete"
            | "this"
            | "null"
            | "true"
            | "false"
            | "void"
            | "int"
            | "double"
            | "bool"
            | "string"
    )
}
