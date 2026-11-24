//! Standalone Xcode project (`.xcodeproj`) ejection for Apple targets
//! (macOS, iOS, tvOS, watchOS).
//!
//! Generates a zero-dependency `project.pbxproj` and shared `.xcscheme`,
//! organizes Swift sources into `App/`, `Models/`, `Styles/`, `Views/`,
//! `Windows/`, `Native/`, and `Runtime/`, and wires either:
//! - `EjectEngine::Source`: a Run Script build phase (`scripts/build-rust.sh`)
//!   that builds the vendored offline Rust workspace for Xcode's active SDK,
//!   architecture, and configuration; or
//! - `EjectEngine::Prebuilt`: a pre-compiled `Frameworks/BernEngine.xcframework`
//!   so the project builds in Xcode without Rust installed.

use crate::swift::{Generator, Platform};
use crate::{
    APPKIT, HEADER, ICON_DOCUMENT, ICON_FILE, MEDIA, MINIMUM_MACOS, RUNTIME, STATUS_IMAGE,
    STATUS_IMAGE_2X, STYLE, alternate_icon, alternate_icons, glue, icon_document,
    icon_document_files, ios, ios16, listing_native_libs, native_libs_file, product_name,
    resources, runtime_with_svg, tvos, watchos,
};
use bern_ir::Application;
use bern_target_api::eject::{
    EjectEngine, EjectOptions, EjectedProject, clean_generated_file, copy_declared_resources,
    readme, sanitized_context,
};
use bern_target_api::{
    BuildContext, BuildStep, Diagnostic, GeneratedFile, brand_assets, icon, skill,
};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// Deterministic 24-character uppercase hexadecimal Xcode object identifier.
fn pbx_id(seed: &str) -> String {
    let mut h1: u64 = 0xcbf29ce484222325;
    let mut h2: u64 = 0x84222325cbf29ce4;
    for byte in seed.as_bytes() {
        h1 ^= u64::from(*byte);
        h1 = h1.wrapping_mul(0x100000001b3);
        h2 = h2.wrapping_mul(0x100000001b3) ^ u64::from(*byte);
    }
    format!("{h1:016X}{:08X}", (h2 >> 32) as u32)
}

fn pbx_quote(value: &str) -> String {
    let simple = !value.is_empty()
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'/' | b'$'));
    if simple {
        value.to_string()
    } else {
        let escaped = value
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n");
        format!("\"{escaped}\"")
    }
}

fn file_type(path: &Path, is_folder: bool) -> &'static str {
    if is_folder {
        return match path.extension().and_then(|e| e.to_str()) {
            Some("xcassets") => "folder.assetcatalog",
            Some("xcframework") => "wrapper.xcframework",
            Some("icon") => "folder.iconcomposer.icon",
            _ => "folder",
        };
    }
    match path.extension().and_then(|e| e.to_str()) {
        Some("swift") => "sourcecode.swift",
        Some("h") => "sourcecode.c.h",
        Some("plist") => "text.plist.xml",
        Some("strings") => "text.plist.strings",
        Some("icns") => "image.icns",
        Some("png") => "image.png",
        Some("jpg" | "jpeg") => "image.jpeg",
        Some("svg") => "text.xml",
        Some("xcframework") => "wrapper.xcframework",
        Some("a") => "archive.ar",
        _ => "file",
    }
}

struct PbxSpec {
    product: String,
    bundle_id: String,
    version: String,
    platform: Platform,
    deployment_target: String,
    default_language: String,
    languages: Vec<String>,
    /// Swift files relative to project root (e.g. `Sources/Views/Home.swift`).
    swift_files: Vec<PathBuf>,
    /// Header file relative to project root (`Sources/Runtime/bern.h`).
    bridging_header: PathBuf,
    /// `Resources/Info.plist`.
    info_plist: PathBuf,
    /// Individual resource files (copied flat into bundle Resources).
    resource_files: Vec<PathBuf>,
    /// Folder references (copied preserving directory hierarchy into bundle Resources).
    resource_folders: Vec<PathBuf>,
    /// Localization variant groups keyed by table name (e.g. `Localizable.strings`,
    /// `InfoPlist.strings`, `ServicesMenu.strings`) -> list of `(lang, path)`.
    localization_groups: BTreeMap<String, Vec<(String, PathBuf)>>,
    /// Asset catalog folder if present (`Resources/Assets.xcassets`).
    asset_catalog: Option<PathBuf>,
    /// Icon Composer document folders (`Resources/AppIcon.icon`, etc.).
    icon_documents: Vec<PathBuf>,
    /// Optional `ASSETCATALOG_COMPILER_APPICON_NAME`.
    asset_app_icon_name: Option<String>,
    engine: EjectEngine,
}

fn generate_pbxproj(spec: &PbxSpec) -> String {
    let project_id = pbx_id("project");
    let main_group_id = pbx_id("group:root");
    let products_group_id = pbx_id("group:Products");
    let sources_group_id = pbx_id("group:Sources");
    let resources_group_id = pbx_id("group:Resources");
    let frameworks_group_id = pbx_id("group:Frameworks");

    let target_id = pbx_id(&format!("target:{}", spec.product));
    let product_ref_id = pbx_id(&format!("product:{}.app", spec.product));
    let project_cfg_list_id = pbx_id("cfglist:project");
    let target_cfg_list_id = pbx_id("cfglist:target");
    let project_debug_id = pbx_id("cfg:project:Debug");
    let project_release_id = pbx_id("cfg:project:Release");
    let target_debug_id = pbx_id("cfg:target:Debug");
    let target_release_id = pbx_id("cfg:target:Release");

    let sources_phase_id = pbx_id("phase:sources");
    let frameworks_phase_id = pbx_id("phase:frameworks");
    let resources_phase_id = pbx_id("phase:resources");
    let rust_script_phase_id = pbx_id("phase:rust_script");

    let mut build_files: Vec<(String, String, String)> = Vec::new();
    let mut file_refs: Vec<(String, String, String, String, String)> = Vec::new();
    let mut source_build_ids: Vec<(String, String)> = Vec::new();
    let mut resource_build_ids: Vec<(String, String)> = Vec::new();
    let mut framework_build_ids: Vec<(String, String)> = Vec::new();

    // Organize Sources/<SubGroup>/<File>.swift into visual Xcode groups.
    let mut source_subgroups: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
    for path in &spec.swift_files {
        let rel = path.to_string_lossy().replace('\\', "/");
        let ref_id = pbx_id(&format!("fileref:{rel}"));
        let build_id = pbx_id(&format!("buildfile:src:{rel}"));
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| rel.clone());
        let subgroup = path
            .parent()
            .and_then(|p| p.strip_prefix("Sources").ok())
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "App".into());
        file_refs.push((
            ref_id.clone(),
            name.clone(),
            rel.clone(),
            file_type(path, false).into(),
            "SOURCE_ROOT".into(),
        ));
        build_files.push((build_id.clone(), ref_id.clone(), name.clone()));
        source_build_ids.push((build_id, name.clone()));
        source_subgroups
            .entry(subgroup)
            .or_default()
            .push((ref_id, name));
    }

    // Bridging header in Sources/Runtime group.
    {
        let rel = spec.bridging_header.to_string_lossy().replace('\\', "/");
        let ref_id = pbx_id(&format!("fileref:{rel}"));
        let name = spec
            .bridging_header
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "bern.h".into());
        file_refs.push((
            ref_id.clone(),
            name.clone(),
            rel,
            "sourcecode.c.h".into(),
            "SOURCE_ROOT".into(),
        ));
        source_subgroups
            .entry("Runtime".into())
            .or_default()
            .push((ref_id, name));
    }

    // Resources group children.
    let mut resource_group_children: Vec<(String, String)> = Vec::new();

    // Info.plist (not in Copy Bundle Resources; referenced via INFOPLIST_FILE).
    {
        let rel = spec.info_plist.to_string_lossy().replace('\\', "/");
        let ref_id = pbx_id(&format!("fileref:{rel}"));
        file_refs.push((
            ref_id.clone(),
            "Info.plist".into(),
            rel,
            "text.plist.xml".into(),
            "SOURCE_ROOT".into(),
        ));
        resource_group_children.push((ref_id, "Info.plist".into()));
    }

    // Asset catalog.
    if let Some(catalog) = &spec.asset_catalog {
        let rel = catalog.to_string_lossy().replace('\\', "/");
        let name = catalog
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Assets.xcassets".into());
        let ref_id = pbx_id(&format!("fileref:{rel}"));
        let build_id = pbx_id(&format!("buildfile:res:{rel}"));
        file_refs.push((
            ref_id.clone(),
            name.clone(),
            rel,
            "folder.assetcatalog".into(),
            "SOURCE_ROOT".into(),
        ));
        build_files.push((build_id.clone(), ref_id.clone(), name.clone()));
        resource_build_ids.push((build_id, name.clone()));
        resource_group_children.push((ref_id, name));
    }

    // Icon Composer documents (`.icon`).
    for icon_doc in &spec.icon_documents {
        let rel = icon_doc.to_string_lossy().replace('\\', "/");
        let name = icon_doc
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "AppIcon.icon".into());
        let ref_id = pbx_id(&format!("fileref:{rel}"));
        let build_id = pbx_id(&format!("buildfile:res:{rel}"));
        file_refs.push((
            ref_id.clone(),
            name.clone(),
            rel,
            "folder.iconcomposer.icon".into(),
            "SOURCE_ROOT".into(),
        ));
        build_files.push((build_id.clone(), ref_id.clone(), name.clone()));
        resource_build_ids.push((build_id, name.clone()));
        resource_group_children.push((ref_id, name));
    }

    // Individual flat resource files.
    for path in &spec.resource_files {
        let rel = path.to_string_lossy().replace('\\', "/");
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| rel.clone());
        let ref_id = pbx_id(&format!("fileref:{rel}"));
        let build_id = pbx_id(&format!("buildfile:res:{rel}"));
        file_refs.push((
            ref_id.clone(),
            name.clone(),
            rel,
            file_type(path, false).into(),
            "SOURCE_ROOT".into(),
        ));
        build_files.push((build_id.clone(), ref_id.clone(), name.clone()));
        resource_build_ids.push((build_id, name.clone()));
        resource_group_children.push((ref_id, name));
    }

    // Resource folder references (preserve subpaths inside bundle Resources).
    for folder in &spec.resource_folders {
        let rel = folder.to_string_lossy().replace('\\', "/");
        let name = folder
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| rel.clone());
        let ref_id = pbx_id(&format!("fileref:folder:{rel}"));
        let build_id = pbx_id(&format!("buildfile:folder:{rel}"));
        file_refs.push((
            ref_id.clone(),
            name.clone(),
            rel,
            "folder".into(),
            "SOURCE_ROOT".into(),
        ));
        build_files.push((build_id.clone(), ref_id.clone(), name.clone()));
        resource_build_ids.push((build_id, name.clone()));
        resource_group_children.push((ref_id, name));
    }

    // Localization variant groups (`Localizable.strings`, `InfoPlist.strings`, `ServicesMenu.strings`).
    struct VariantGroup {
        id: String,
        name: String,
        children: Vec<(String, String)>,
    }
    let mut variant_groups: Vec<VariantGroup> = Vec::new();
    for (group_name, entries) in &spec.localization_groups {
        if entries.is_empty() {
            continue;
        }
        let vg_id = pbx_id(&format!("vgroup:{group_name}"));
        let build_id = pbx_id(&format!("buildfile:vgroup:{group_name}"));
        let mut children = Vec::new();
        for (lang, path) in entries {
            let rel = path.to_string_lossy().replace('\\', "/");
            let ref_id = pbx_id(&format!("fileref:loc:{rel}"));
            file_refs.push((
                ref_id.clone(),
                lang.clone(),
                rel,
                "text.plist.strings".into(),
                "SOURCE_ROOT".into(),
            ));
            children.push((ref_id, lang.clone()));
        }
        variant_groups.push(VariantGroup {
            id: vg_id.clone(),
            name: group_name.clone(),
            children,
        });
        build_files.push((build_id.clone(), vg_id.clone(), group_name.clone()));
        resource_build_ids.push((build_id, group_name.clone()));
        resource_group_children.push((vg_id, group_name.clone()));
    }

    // Prebuilt XCFramework.
    let mut frameworks_group_children: Vec<(String, String)> = Vec::new();
    if spec.engine == EjectEngine::Prebuilt {
        let rel = "Frameworks/BernEngine.xcframework";
        let ref_id = pbx_id(&format!("fileref:{rel}"));
        let build_id = pbx_id(&format!("buildfile:fw:{rel}"));
        file_refs.push((
            ref_id.clone(),
            "BernEngine.xcframework".into(),
            rel.into(),
            "wrapper.xcframework".into(),
            "SOURCE_ROOT".into(),
        ));
        build_files.push((
            build_id.clone(),
            ref_id.clone(),
            "BernEngine.xcframework".into(),
        ));
        framework_build_ids.push((build_id, "BernEngine.xcframework".into()));
        frameworks_group_children.push((ref_id, "BernEngine.xcframework".into()));
    }

    // Build PBXGroup objects for source subgroups.
    let mut groups_section = String::new();
    let mut sources_group_children: Vec<(String, String)> = Vec::new();
    let order = [
        "App", "Models", "Styles", "Views", "Windows", "Native", "Runtime",
    ];
    let mut ordered_keys: Vec<String> = order
        .iter()
        .filter(|k| source_subgroups.contains_key(**k))
        .map(|k| (*k).to_string())
        .collect();
    for key in source_subgroups.keys() {
        if !ordered_keys.contains(key) {
            ordered_keys.push(key.clone());
        }
    }
    for name in ordered_keys {
        let gid = pbx_id(&format!("group:Sources/{name}"));
        sources_group_children.push((gid.clone(), name.clone()));
        let children = &source_subgroups[&name];
        groups_section.push_str(&format!(
            "\t\t{gid} /* {name} */ = {{\n\t\t\tisa = PBXGroup;\n\t\t\tchildren = (\n"
        ));
        for (cid, cname) in children {
            groups_section.push_str(&format!("\t\t\t\t{cid} /* {cname} */,\n"));
        }
        groups_section.push_str(&format!(
            "\t\t\t);\n\t\t\tname = {};\n\t\t\tsourceTree = \"<group>\";\n\t\t}};\n",
            pbx_quote(&name)
        ));
    }

    // Platform build settings.
    let (sdk_root, deploy_key, extra_platform_settings, platform_frameworks): (
        &str,
        &str,
        Vec<(&str, String)>,
        Vec<&str>,
    ) = match spec.platform {
        Platform::Macos => (
            "macosx",
            "MACOSX_DEPLOYMENT_TARGET",
            vec![
                ("COMBINE_HIDPI_IMAGES", "YES".into()),
                (
                    "LD_RUNPATH_SEARCH_PATHS",
                    "(\"$(inherited)\", \"@executable_path/../Frameworks\")".into(),
                ),
            ],
            vec!["SwiftUI", "AppKit", "AVFoundation", "AVKit"],
        ),
        Platform::Ios => (
            "iphoneos",
            "IPHONEOS_DEPLOYMENT_TARGET",
            vec![
                ("SUPPORTED_PLATFORMS", "\"iphoneos iphonesimulator\"".into()),
                ("TARGETED_DEVICE_FAMILY", "\"1,2\"".into()),
                (
                    "LD_RUNPATH_SEARCH_PATHS",
                    "(\"$(inherited)\", \"@executable_path/Frameworks\")".into(),
                ),
            ],
            vec!["SwiftUI", "UIKit", "AVFoundation", "AVKit"],
        ),
        Platform::Tvos => (
            "appletvos",
            "TVOS_DEPLOYMENT_TARGET",
            vec![
                (
                    "SUPPORTED_PLATFORMS",
                    "\"appletvos appletvsimulator\"".into(),
                ),
                ("TARGETED_DEVICE_FAMILY", "3".into()),
                (
                    "LD_RUNPATH_SEARCH_PATHS",
                    "(\"$(inherited)\", \"@executable_path/Frameworks\")".into(),
                ),
            ],
            vec!["SwiftUI", "UIKit", "AVFoundation", "AVKit"],
        ),
        Platform::Watchos => (
            "watchos",
            "WATCHOS_DEPLOYMENT_TARGET",
            vec![
                ("SUPPORTED_PLATFORMS", "\"watchos watchsimulator\"".into()),
                ("TARGETED_DEVICE_FAMILY", "4".into()),
                ("SKIP_INSTALL", "NO".into()),
                (
                    "LD_RUNPATH_SEARCH_PATHS",
                    "(\"$(inherited)\", \"@executable_path/Frameworks\")".into(),
                ),
            ],
            vec!["SwiftUI", "WatchKit", "AVFoundation", "AVKit"],
        ),
    };

    let mut ldflags: Vec<String> = vec![
        "\"$(inherited)\"".into(),
        "\"-Wl,-no_warn_duplicate_libraries\"".into(),
    ];
    match spec.engine {
        EjectEngine::Source => {
            ldflags.push("\"$(BUILT_PRODUCTS_DIR)/libbern_app.a\"".into());
            ldflags.push("\"@$(BUILT_PRODUCTS_DIR)/bern_native_libs.resp\"".into());
        }
        EjectEngine::Prebuilt => {
            ldflags.push("\"@$(SRCROOT)/Frameworks/bern_native_libs.resp\"".into());
        }
    }
    for fw in &platform_frameworks {
        ldflags.push("\"-framework\"".into());
        ldflags.push(format!("\"{fw}\""));
    }
    let ldflags_list = format!("(\n\t\t\t\t\t{},\n\t\t\t\t)", ldflags.join(",\n\t\t\t\t\t"));

    let target_settings = |release: bool| -> String {
        let mut s = String::new();
        if let Some(icon_name) = &spec.asset_app_icon_name {
            s.push_str(&format!(
                "\t\t\t\tASSETCATALOG_COMPILER_APPICON_NAME = {};\n",
                pbx_quote(icon_name)
            ));
        }
        s.push_str("\t\t\t\tCODE_SIGN_STYLE = Automatic;\n");
        s.push_str("\t\t\t\tCURRENT_PROJECT_VERSION = 1;\n");
        s.push_str("\t\t\t\tDEAD_CODE_STRIPPING = YES;\n");
        s.push_str("\t\t\t\tENABLE_USER_SCRIPT_SANDBOXING = NO;\n");
        s.push_str("\t\t\t\tGENERATE_INFOPLIST_FILE = NO;\n");
        s.push_str(&format!(
            "\t\t\t\tINFOPLIST_FILE = {};\n",
            pbx_quote(&spec.info_plist.to_string_lossy().replace('\\', "/"))
        ));
        s.push_str(&format!(
            "\t\t\t\tMARKETING_VERSION = {};\n",
            pbx_quote(&spec.version)
        ));
        s.push_str(&format!("\t\t\t\tOTHER_LDFLAGS = {ldflags_list};\n"));
        s.push_str(&format!(
            "\t\t\t\tPRODUCT_BUNDLE_IDENTIFIER = {};\n",
            pbx_quote(&spec.bundle_id)
        ));
        s.push_str("\t\t\t\tPRODUCT_NAME = \"$(TARGET_NAME)\";\n");
        s.push_str(&format!(
            "\t\t\t\tSWIFT_OBJC_BRIDGING_HEADER = {};\n",
            pbx_quote(&spec.bridging_header.to_string_lossy().replace('\\', "/"))
        ));
        if release {
            s.push_str("\t\t\t\tSWIFT_COMPILATION_MODE = wholemodule;\n");
            s.push_str("\t\t\t\tSWIFT_OPTIMIZATION_LEVEL = \"-O\";\n");
        } else {
            s.push_str("\t\t\t\tSWIFT_ACTIVE_COMPILATION_CONDITIONS = \"DEBUG $(inherited)\";\n");
            s.push_str("\t\t\t\tSWIFT_OPTIMIZATION_LEVEL = \"-Onone\";\n");
        }
        s.push_str("\t\t\t\tSWIFT_VERSION = 6.0;\n");
        for (k, v) in &extra_platform_settings {
            s.push_str(&format!("\t\t\t\t{k} = {v};\n"));
        }
        s
    };

    let mut out = String::new();
    out.push_str("// !$*UTF8*$!\n{\n\tarchiveVersion = 1;\n\tclasses = {\n\t};\n\tobjectVersion = 56;\n\tobjects = {\n\n");

    // PBXBuildFile
    out.push_str("/* Begin PBXBuildFile section */\n");
    for (bid, fid, comment) in &build_files {
        out.push_str(&format!(
            "\t\t{bid} /* {comment} */ = {{isa = PBXBuildFile; fileRef = {fid} /* {comment} */; }};\n"
        ));
    }
    out.push_str("/* End PBXBuildFile section */\n\n");

    // PBXFileReference
    out.push_str("/* Begin PBXFileReference section */\n");
    out.push_str(&format!(
        "\t\t{product_ref_id} /* {}.app */ = {{isa = PBXFileReference; explicitFileType = wrapper.application; includeInIndex = 0; path = {}; sourceTree = BUILT_PRODUCTS_DIR; }};\n",
        spec.product,
        pbx_quote(&format!("{}.app", spec.product))
    ));
    for (fid, name, path, ftype, tree) in &file_refs {
        out.push_str(&format!(
            "\t\t{fid} /* {name} */ = {{isa = PBXFileReference; lastKnownFileType = {ftype}; name = {}; path = {}; sourceTree = {}; }};\n",
            pbx_quote(name),
            pbx_quote(path),
            pbx_quote(tree)
        ));
    }
    out.push_str("/* End PBXFileReference section */\n\n");

    // PBXFrameworksBuildPhase
    out.push_str("/* Begin PBXFrameworksBuildPhase section */\n");
    out.push_str(&format!(
        "\t\t{frameworks_phase_id} /* Frameworks */ = {{\n\t\t\tisa = PBXFrameworksBuildPhase;\n\t\t\tbuildActionMask = 2147483647;\n\t\t\tfiles = (\n"
    ));
    for (bid, name) in &framework_build_ids {
        out.push_str(&format!("\t\t\t\t{bid} /* {name} in Frameworks */,\n"));
    }
    out.push_str("\t\t\t);\n\t\t\trunOnlyForDeploymentPostprocessing = 0;\n\t\t};\n");
    out.push_str("/* End PBXFrameworksBuildPhase section */\n\n");

    // PBXGroup
    out.push_str("/* Begin PBXGroup section */\n");
    out.push_str(&format!(
        "\t\t{main_group_id} = {{\n\t\t\tisa = PBXGroup;\n\t\t\tchildren = (\n"
    ));
    out.push_str(&format!("\t\t\t\t{sources_group_id} /* Sources */,\n"));
    out.push_str(&format!("\t\t\t\t{resources_group_id} /* Resources */,\n"));
    if !frameworks_group_children.is_empty() {
        out.push_str(&format!(
            "\t\t\t\t{frameworks_group_id} /* Frameworks */,\n"
        ));
    }
    out.push_str(&format!("\t\t\t\t{products_group_id} /* Products */,\n"));
    out.push_str("\t\t\t);\n\t\t\tsourceTree = \"<group>\";\n\t\t};\n");

    out.push_str(&format!(
        "\t\t{products_group_id} /* Products */ = {{\n\t\t\tisa = PBXGroup;\n\t\t\tchildren = (\n\t\t\t\t{product_ref_id} /* {}.app */,\n\t\t\t);\n\t\t\tname = Products;\n\t\t\tsourceTree = \"<group>\";\n\t\t}};\n",
        spec.product
    ));

    out.push_str(&format!(
        "\t\t{sources_group_id} /* Sources */ = {{\n\t\t\tisa = PBXGroup;\n\t\t\tchildren = (\n"
    ));
    for (cid, name) in &sources_group_children {
        out.push_str(&format!("\t\t\t\t{cid} /* {name} */,\n"));
    }
    out.push_str("\t\t\t);\n\t\t\tname = Sources;\n\t\t\tsourceTree = \"<group>\";\n\t\t};\n");
    out.push_str(&groups_section);

    out.push_str(&format!(
        "\t\t{resources_group_id} /* Resources */ = {{\n\t\t\tisa = PBXGroup;\n\t\t\tchildren = (\n"
    ));
    for (cid, name) in &resource_group_children {
        out.push_str(&format!("\t\t\t\t{cid} /* {name} */,\n"));
    }
    out.push_str("\t\t\t);\n\t\t\tname = Resources;\n\t\t\tsourceTree = \"<group>\";\n\t\t};\n");

    if !frameworks_group_children.is_empty() {
        out.push_str(&format!(
            "\t\t{frameworks_group_id} /* Frameworks */ = {{\n\t\t\tisa = PBXGroup;\n\t\t\tchildren = (\n"
        ));
        for (cid, name) in &frameworks_group_children {
            out.push_str(&format!("\t\t\t\t{cid} /* {name} */,\n"));
        }
        out.push_str(
            "\t\t\t);\n\t\t\tname = Frameworks;\n\t\t\tsourceTree = \"<group>\";\n\t\t};\n",
        );
    }
    out.push_str("/* End PBXGroup section */\n\n");

    // PBXNativeTarget
    out.push_str("/* Begin PBXNativeTarget section */\n");
    out.push_str(&format!(
        "\t\t{target_id} /* {} */ = {{\n\t\t\tisa = PBXNativeTarget;\n\t\t\tbuildConfigurationList = {target_cfg_list_id} /* Build configuration list for PBXNativeTarget \"{}\" */;\n\t\t\tbuildPhases = (\n",
        spec.product, spec.product
    ));
    if spec.engine == EjectEngine::Source {
        out.push_str(&format!(
            "\t\t\t\t{rust_script_phase_id} /* Build Rust Engine */,\n"
        ));
    }
    out.push_str(&format!("\t\t\t\t{sources_phase_id} /* Sources */,\n"));
    out.push_str(&format!(
        "\t\t\t\t{frameworks_phase_id} /* Frameworks */,\n"
    ));
    out.push_str(&format!("\t\t\t\t{resources_phase_id} /* Resources */,\n"));
    out.push_str(&format!(
        "\t\t\t);\n\t\t\tbuildRules = (\n\t\t\t);\n\t\t\tdependencies = (\n\t\t\t);\n\t\t\tname = {};\n\t\t\tproductName = {};\n\t\t\tproductReference = {product_ref_id} /* {}.app */;\n\t\t\tproductType = \"com.apple.product-type.application\";\n\t\t}};\n",
        pbx_quote(&spec.product),
        pbx_quote(&spec.product),
        spec.product
    ));
    out.push_str("/* End PBXNativeTarget section */\n\n");

    // PBXProject
    out.push_str("/* Begin PBXProject section */\n");
    out.push_str(&format!(
        "\t\t{project_id} /* Project object */ = {{\n\t\t\tisa = PBXProject;\n\t\t\tbuildConfigurationList = {project_cfg_list_id} /* Build configuration list for PBXProject \"{}\" */;\n\t\t\tcompatibilityVersion = \"Xcode 14.0\";\n\t\t\tdevelopmentRegion = {};\n\t\t\thasScannedForEncodings = 0;\n\t\t\tknownRegions = (\n",
        spec.product,
        pbx_quote(&spec.default_language)
    ));
    let mut regions: BTreeSet<&str> = spec.languages.iter().map(String::as_str).collect();
    regions.insert("Base");
    for region in regions {
        out.push_str(&format!("\t\t\t\t{},\n", pbx_quote(region)));
    }
    out.push_str(&format!(
        "\t\t\t);\n\t\t\tmainGroup = {main_group_id};\n\t\t\tproductRefGroup = {products_group_id} /* Products */;\n\t\t\tprojectDirPath = \"\";\n\t\t\tprojectRoot = \"\";\n\t\t\ttargets = (\n\t\t\t\t{target_id} /* {} */,\n\t\t\t);\n\t\t}};\n",
        spec.product
    ));
    out.push_str("/* End PBXProject section */\n\n");

    // PBXResourcesBuildPhase
    out.push_str("/* Begin PBXResourcesBuildPhase section */\n");
    out.push_str(&format!(
        "\t\t{resources_phase_id} /* Resources */ = {{\n\t\t\tisa = PBXResourcesBuildPhase;\n\t\t\tbuildActionMask = 2147483647;\n\t\t\tfiles = (\n"
    ));
    for (bid, name) in &resource_build_ids {
        out.push_str(&format!("\t\t\t\t{bid} /* {name} in Resources */,\n"));
    }
    out.push_str("\t\t\t);\n\t\t\trunOnlyForDeploymentPostprocessing = 0;\n\t\t};\n");
    out.push_str("/* End PBXResourcesBuildPhase section */\n\n");

    // PBXShellScriptBuildPhase (Source mode)
    if spec.engine == EjectEngine::Source {
        out.push_str("/* Begin PBXShellScriptBuildPhase section */\n");
        out.push_str(&format!(
            "\t\t{rust_script_phase_id} /* Build Rust Engine */ = {{\n\t\t\tisa = PBXShellScriptBuildPhase;\n\t\t\talwaysOutOfDate = 1;\n\t\t\tbuildActionMask = 2147483647;\n\t\t\tfiles = (\n\t\t\t);\n\t\t\tinputFileListPaths = (\n\t\t\t);\n\t\t\tinputPaths = (\n\t\t\t);\n\t\t\tname = \"Build Rust Engine\";\n\t\t\toutputFileListPaths = (\n\t\t\t);\n\t\t\toutputPaths = (\n\t\t\t\t\"$(BUILT_PRODUCTS_DIR)/libbern_app.a\",\n\t\t\t\t\"$(BUILT_PRODUCTS_DIR)/bern_native_libs.resp\",\n\t\t\t);\n\t\t\trunOnlyForDeploymentPostprocessing = 0;\n\t\t\tshellPath = /bin/sh;\n\t\t\tshellScript = \"\\\"$SRCROOT/scripts/build-rust.sh\\\"\\n\";\n\t\t}};\n"
        ));
        out.push_str("/* End PBXShellScriptBuildPhase section */\n\n");
    }

    // PBXSourcesBuildPhase
    out.push_str("/* Begin PBXSourcesBuildPhase section */\n");
    out.push_str(&format!(
        "\t\t{sources_phase_id} /* Sources */ = {{\n\t\t\tisa = PBXSourcesBuildPhase;\n\t\t\tbuildActionMask = 2147483647;\n\t\t\tfiles = (\n"
    ));
    for (bid, name) in &source_build_ids {
        out.push_str(&format!("\t\t\t\t{bid} /* {name} in Sources */,\n"));
    }
    out.push_str("\t\t\t);\n\t\t\trunOnlyForDeploymentPostprocessing = 0;\n\t\t};\n");
    out.push_str("/* End PBXSourcesBuildPhase section */\n\n");

    // PBXVariantGroup
    if !variant_groups.is_empty() {
        out.push_str("/* Begin PBXVariantGroup section */\n");
        for group in &variant_groups {
            out.push_str(&format!(
                "\t\t{} /* {} */ = {{\n\t\t\tisa = PBXVariantGroup;\n\t\t\tchildren = (\n",
                group.id, group.name
            ));
            for (cid, lang) in &group.children {
                out.push_str(&format!("\t\t\t\t{cid} /* {lang} */,\n"));
            }
            out.push_str(&format!(
                "\t\t\t);\n\t\t\tname = {};\n\t\t\tsourceTree = \"<group>\";\n\t\t}};\n",
                pbx_quote(&group.name)
            ));
        }
        out.push_str("/* End PBXVariantGroup section */\n\n");
    }

    // XCBuildConfiguration
    out.push_str("/* Begin XCBuildConfiguration section */\n");
    for (cfg_id, name, release) in [
        (&project_debug_id, "Debug", false),
        (&project_release_id, "Release", true),
    ] {
        out.push_str(&format!(
            "\t\t{cfg_id} /* {name} */ = {{\n\t\t\tisa = XCBuildConfiguration;\n\t\t\tbuildSettings = {{\n\t\t\t\tALWAYS_SEARCH_USER_PATHS = NO;\n\t\t\t\tCLANG_ENABLE_MODULES = YES;\n\t\t\t\tCLANG_ENABLE_OBJC_ARC = YES;\n\t\t\t\tCOPY_PHASE_STRIP = NO;\n\t\t\t\tDEBUG_INFORMATION_FORMAT = {};\n\t\t\t\tENABLE_STRICT_OBJC_MSGSEND = YES;\n\t\t\t\t{deploy_key} = {};\n\t\t\t\tONLY_ACTIVE_ARCH = {};\n\t\t\t\tSDKROOT = {sdk_root};\n\t\t\t}};\n\t\t\tname = {name};\n\t\t}};\n",
            if release { "\"dwarf-with-dsym\"" } else { "dwarf" },
            pbx_quote(&spec.deployment_target),
            if release { "NO" } else { "YES" },
        ));
    }
    for (cfg_id, name, release) in [
        (&target_debug_id, "Debug", false),
        (&target_release_id, "Release", true),
    ] {
        out.push_str(&format!(
            "\t\t{cfg_id} /* {name} */ = {{\n\t\t\tisa = XCBuildConfiguration;\n\t\t\tbuildSettings = {{\n{}\t\t\t}};\n\t\t\tname = {name};\n\t\t}};\n",
            target_settings(release)
        ));
    }
    out.push_str("/* End XCBuildConfiguration section */\n\n");

    // XCConfigurationList
    out.push_str("/* Begin XCConfigurationList section */\n");
    out.push_str(&format!(
        "\t\t{project_cfg_list_id} /* Build configuration list for PBXProject \"{}\" */ = {{\n\t\t\tisa = XCConfigurationList;\n\t\t\tbuildConfigurations = (\n\t\t\t\t{project_debug_id} /* Debug */,\n\t\t\t\t{project_release_id} /* Release */,\n\t\t\t);\n\t\t\tdefaultConfigurationIsVisible = 0;\n\t\t\tdefaultConfigurationName = Release;\n\t\t}};\n",
        spec.product
    ));
    out.push_str(&format!(
        "\t\t{target_cfg_list_id} /* Build configuration list for PBXNativeTarget \"{}\" */ = {{\n\t\t\tisa = XCConfigurationList;\n\t\t\tbuildConfigurations = (\n\t\t\t\t{target_debug_id} /* Debug */,\n\t\t\t\t{target_release_id} /* Release */,\n\t\t\t);\n\t\t\tdefaultConfigurationIsVisible = 0;\n\t\t\tdefaultConfigurationName = Release;\n\t\t}};\n",
        spec.product
    ));
    out.push_str("/* End XCConfigurationList section */\n");

    out.push_str(&format!(
        "\t}};\n\trootObject = {project_id} /* Project object */;\n}}\n"
    ));
    out
}

fn generate_xcscheme(product: &str) -> String {
    let target_id = pbx_id(&format!("target:{product}"));
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Scheme LastUpgradeVersion = "1600" version = "1.7">
   <BuildAction parallelizeBuildables = "YES" buildImplicitDependencies = "YES">
      <BuildActionEntries>
         <BuildActionEntry buildForTesting = "YES" buildForRunning = "YES" buildForProfiling = "YES" buildForArchiving = "YES" buildForAnalyzing = "YES">
            <BuildableReference BuildableIdentifier = "primary" BlueprintIdentifier = "{target_id}" BuildableName = "{product}.app" BlueprintName = "{product}" ReferencedContainer = "container:{product}.xcodeproj">
            </BuildableReference>
         </BuildActionEntry>
      </BuildActionEntries>
   </BuildAction>
   <LaunchAction buildConfiguration = "Debug" selectedDebuggerIdentifier = "Xcode.DebuggerFoundation.Debugger.LLDB" selectedLauncherIdentifier = "Xcode.DebuggerFoundation.Launcher.LLDB" launchStyle = "0" useCustomWorkingDirectory = "NO" ignoresPersistentStateOnLaunch = "NO" debugDocumentVersioning = "YES" debugServiceExtension = "internal" allowLocationSimulation = "YES">
      <BuildableProductRunnable runnableDebuggingMode = "0">
         <BuildableReference BuildableIdentifier = "primary" BlueprintIdentifier = "{target_id}" BuildableName = "{product}.app" BlueprintName = "{product}" ReferencedContainer = "container:{product}.xcodeproj">
         </BuildableReference>
      </BuildableProductRunnable>
   </LaunchAction>
   <ProfileAction buildConfiguration = "Release" shouldUseLaunchSchemeArgsEnv = "YES" savedToolIdentifier = "" useCustomWorkingDirectory = "NO" debugDocumentVersioning = "YES">
      <BuildableProductRunnable runnableDebuggingMode = "0">
         <BuildableReference BuildableIdentifier = "primary" BlueprintIdentifier = "{target_id}" BuildableName = "{product}.app" BlueprintName = "{product}" ReferencedContainer = "container:{product}.xcodeproj">
         </BuildableReference>
      </BuildableProductRunnable>
   </ProfileAction>
   <ArchiveAction buildConfiguration = "Release" revealArchiveInOrganizer = "YES">
   </ArchiveAction>
</Scheme>
"#
    )
}

/// Shell script executed by Xcode's "Build Rust Engine" Run Script phase.
fn build_rust_script(platform: Platform, deployment_target: &str) -> String {
    let (deploy_env, default_platform) = match platform {
        Platform::Macos => ("MACOSX_DEPLOYMENT_TARGET", "macosx"),
        Platform::Ios => ("IPHONEOS_DEPLOYMENT_TARGET", "iphonesimulator"),
        Platform::Tvos => ("TVOS_DEPLOYMENT_TARGET", "appletvsimulator"),
        Platform::Watchos => ("WATCHOS_DEPLOYMENT_TARGET", "watchsimulator"),
    };
    format!(
        r#"#!/bin/sh
# Build the vendored Rust engine static library for Xcode's active SDK,
# architecture(s), and configuration. Edit freely.
set -eu

export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:$PATH"

if ! command -v cargo >/dev/null 2>&1; then
    echo "error: cargo not found in PATH ($PATH); install Rust from https://rustup.rs" >&2
    exit 1
fi

PLATFORM="${{PLATFORM_NAME:-{default_platform}}}"
ARCHS_LIST="${{ARCHS:-$(uname -m)}}"
CONFIG="${{CONFIGURATION:-Debug}}"
MANIFEST="$SRCROOT/rust/glue/Cargo.toml"
TARGET_DIR="$SRCROOT/rust/target"
OUT_LIB="${{BUILT_PRODUCTS_DIR:?}}/libbern_app.a"
OUT_RESP="${{BUILT_PRODUCTS_DIR:?}}/bern_native_libs.resp"

export {deploy_env}="${{{deploy_env}:-{deployment_target}}}"

PROFILE="debug"
CARGO_PROFILE_ARGS=""
if [ "$CONFIG" = "Release" ]; then
    PROFILE="release"
    CARGO_PROFILE_ARGS="--release"
fi

rust_target_for() {{
    arch="$1"
    case "$PLATFORM:$arch" in
        macosx:arm64) echo "aarch64-apple-darwin" ;;
        macosx:x86_64) echo "x86_64-apple-darwin" ;;
        iphoneos:arm64) echo "aarch64-apple-ios" ;;
        iphonesimulator:arm64) echo "aarch64-apple-ios-sim" ;;
        iphonesimulator:x86_64) echo "x86_64-apple-ios" ;;
        appletvos:arm64) echo "aarch64-apple-tvos" ;;
        appletvsimulator:arm64) echo "aarch64-apple-tvos-sim" ;;
        appletvsimulator:x86_64) echo "x86_64-apple-tvos" ;;
        watchos:arm64|watchos:arm64_32) echo "aarch64-apple-watchos" ;;
        watchsimulator:arm64) echo "aarch64-apple-watchos-sim" ;;
        watchsimulator:x86_64) echo "x86_64-apple-watchos" ;;
        *)
            echo "error: unsupported Xcode platform/arch combination: $PLATFORM/$arch" >&2
            exit 1
            ;;
    esac
}}

LIBS=""
FIRST_RESP=""
cd "$SRCROOT/rust/glue"
for arch in $ARCHS_LIST; do
    RUST_TARGET="$(rust_target_for "$arch")"
    RESP_FILE="$TARGET_DIR/$RUST_TARGET/$PROFILE/libbern_app.native-libs"
    mkdir -p "$(dirname "$RESP_FILE")"
    # shellcheck disable=SC2086
    cargo rustc \
        --manifest-path "$MANIFEST" \
        --lib \
        --target "$RUST_TARGET" \
        --target-dir "$TARGET_DIR" \
        $CARGO_PROFILE_ARGS \
        -- --print "native-static-libs=$RESP_FILE"
    LIBS="$LIBS $TARGET_DIR/$RUST_TARGET/$PROFILE/libbern_app.a"
    if [ -z "$FIRST_RESP" ]; then
        FIRST_RESP="$RESP_FILE"
    fi
done

set -- $LIBS
if [ "$#" -eq 1 ]; then
    cp "$1" "$OUT_LIB"
else
    lipo -create "$@" -output "$OUT_LIB"
fi

if [ -n "$FIRST_RESP" ] && [ -f "$FIRST_RESP" ]; then
    cp "$FIRST_RESP" "$OUT_RESP"
else
    : > "$OUT_RESP"
fi
"#
    )
}

/// Module map bundled inside `BernEngine.xcframework` headers.
const ENGINE_MODULEMAP: &str = r#"module BernEngine {
    header "bern.h"
    export *
}
"#;

/// Eject any Apple SwiftUI target (`macos`, `ios`, `tvos`, `watchos`) into a
/// standalone `.xcodeproj` project.
pub(crate) fn eject_apple(
    app: &Application,
    context: &BuildContext,
    options: &EjectOptions,
    platform: Platform,
) -> Result<EjectedProject, Vec<Diagnostic>> {
    let context = &sanitized_context(context, options);
    let prepared = bern_target_api::svg::prepare(app, &context.target);
    let app = if platform == Platform::Tvos {
        prepared.on_television()
    } else {
        prepared
    };
    let app = &app;
    bern_target_api::svg::validate(app, context)?;

    let product = product_name(app);
    let mut files: Vec<GeneratedFile> = Vec::new();
    let mut swift_files: Vec<PathBuf> = Vec::new();
    let mut resource_files: Vec<PathBuf> = Vec::new();
    let mut resource_folders: Vec<PathBuf> = Vec::new();
    let mut localization_groups: BTreeMap<String, Vec<(String, PathBuf)>> = BTreeMap::new();
    let mut asset_catalog: Option<PathBuf> = None;
    let mut icon_documents: Vec<PathBuf> = Vec::new();
    let mut asset_app_icon_name: Option<String> = None;

    let ios_frontends = (platform == Platform::Ios).then(|| ios::frontends(&context.target));
    let use_ios16 = ios_frontends.is_some_and(|f| f == ios::IosFrontendKind::Ios16);

    let deployment_target = match platform {
        Platform::Macos => MINIMUM_MACOS.to_string(),
        Platform::Ios => ios_frontends
            .map(|f| f.minimum().to_string())
            .unwrap_or_else(|| ios::MINIMUM_IOS.to_string()),
        Platform::Tvos => tvos::MINIMUM_TVOS.to_string(),
        Platform::Watchos => watchos::MINIMUM_WATCHOS.to_string(),
    };

    let rewrite = |src: String| -> String { if use_ios16 { ios16::rewrite(&src) } else { src } };

    // 1. Runtime Swift sources under `Sources/Runtime/`.
    let platform_kit: (&str, &str) = match platform {
        Platform::Macos => ("BernAppKit.swift", APPKIT),
        Platform::Ios => ("BernUIKit.swift", ios::UIKIT_TEMPLATE),
        Platform::Tvos => ("BernUIKit.swift", tvos::UIKIT_TEMPLATE),
        Platform::Watchos => ("BernWatchKit.swift", watchos::WATCHKIT_TEMPLATE),
    };
    for (name, content) in [
        ("BernRuntime.swift", runtime_with_svg(app, RUNTIME)),
        ("BernMedia.swift", MEDIA.to_string()),
        ("BernStyle.swift", STYLE.to_string()),
        (platform_kit.0, platform_kit.1.to_string()),
    ] {
        let path = PathBuf::from("Sources/Runtime").join(name);
        swift_files.push(path.clone());
        files.push(GeneratedFile::text(path, rewrite(content)));
    }
    let bridging_header = PathBuf::from("Sources/Runtime/bern.h");
    files.push(GeneratedFile::text(bridging_header.clone(), HEADER));

    // 2. Split generated Swift sources (`fingerprint = 0` relaxes protocol check).
    let app_icons = crate::app_icon_contours(app, context)?;
    for (rel, content) in Generator::new(app, platform)
        .with_app_icons(app_icons)
        .generate_split(0)
    {
        let path = PathBuf::from("Sources").join(rel);
        swift_files.push(path.clone());
        files.push(GeneratedFile::text(path, rewrite(content)));
    }

    // 3. Copy co-located `NativeView` Swift files into `Sources/Native/`
    //    ensuring unique basenames across the entire Xcode target.
    let mut used_swift_basenames: BTreeSet<String> = swift_files
        .iter()
        .filter_map(|p| {
            p.file_stem()
                .map(|s| s.to_string_lossy().to_ascii_lowercase())
        })
        .collect();
    for native in app.native_files.iter().filter(|f| f.backend == "swiftui") {
        let base = bern_target_api::eject::pascal_filename(
            Path::new(&native.path)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("NativeView"),
        );
        let mut stem = base.clone();
        let mut suffix = 2usize;
        while !used_swift_basenames.insert(stem.to_ascii_lowercase()) {
            stem = format!("{base}Native{suffix}");
            suffix += 1;
        }
        let disk = context.project_dir.join(&native.path);
        let content = std::fs::read_to_string(&disk).unwrap_or_else(|_| native.source.clone());
        let path = PathBuf::from(format!("Sources/Native/{stem}.swift"));
        swift_files.push(path.clone());
        files.push(GeneratedFile::text(path, rewrite(content)));
    }

    // 4. Info.plist and localizations under `Resources/`.
    let info_plist = PathBuf::from("Resources/Info.plist");
    let plist_text = match platform {
        Platform::Macos => resources::info_plist(
            app,
            &product,
            &deployment_target,
            icon_document(app, "macos", &context.project_dir).is_some(),
        ),
        Platform::Ios => resources::info_plist_ios(app, &product, &deployment_target),
        Platform::Tvos => resources::info_plist_tvos(app, &product, &deployment_target),
        Platform::Watchos => resources::info_plist_watchos(app, &product, &deployment_target),
    };
    files.push(GeneratedFile::text(info_plist.clone(), plist_text));

    let mut add_localization = |rel: PathBuf, table: String| {
        let rel_str = rel.to_string_lossy().replace('\\', "/");
        let (lang, table_name) = match rel_str.split_once(".lproj/") {
            Some((l, name)) => (l.to_string(), name.to_string()),
            None => ("en".into(), "Localizable.strings".into()),
        };
        let path = PathBuf::from("Resources").join(&rel);
        localization_groups
            .entry(table_name)
            .or_default()
            .push((lang, path.clone()));
        files.push(GeneratedFile::text(path, table));
    };
    for (rel, table) in resources::localization_files(app) {
        add_localization(rel, table);
    }
    if platform == Platform::Macos {
        for (rel, table) in resources::services_menu_files(app) {
            add_localization(rel, table);
        }
    }

    // 5. Platform icons & Asset Catalogs.
    let failed = |message: String| vec![Diagnostic::error(message)];
    match platform {
        Platform::Macos => {
            if let Some(document) = icon_document(app, "macos", &context.project_dir) {
                let dest = PathBuf::from("Resources").join(ICON_DOCUMENT);
                files.extend(icon_document_files(&document, &dest)?);
                icon_documents.push(dest);
            } else {
                let contents = match icon::source(app, "macos", &context.project_dir) {
                    icon::Source::Native(path) => std::fs::read(&path).map_err(|error| {
                        failed(format!("cannot read the icon {}: {error}", path.display()))
                    })?,
                    _ => icon::icns(
                        &icon::images(app, "macos", &context.project_dir, icon::Shape::Masked)
                            .map_err(failed)?,
                    ),
                };
                let path = PathBuf::from("Resources").join(ICON_FILE);
                resource_files.push(path.clone());
                files.push(GeneratedFile { path, contents });
            }
            if let Some(name) = icon::status_image(app) {
                let src = bern_target_api::svg::resource_path(context, name);
                for (side, file) in [(18, STATUS_IMAGE), (36, STATUS_IMAGE_2X)] {
                    let path = PathBuf::from("Resources").join(file);
                    resource_files.push(path.clone());
                    files.push(GeneratedFile {
                        path,
                        contents: icon::status_picture(&src, side, icon::ON_LIGHT)
                            .map_err(|m| failed(format!("StatusItem image: {m}")))?,
                    });
                }
            }
            for (name, source) in alternate_icons(app, "macos", &context.project_dir)? {
                match source {
                    icon::Source::Native(path) if icon::is_icon_document(&path) => {
                        let dest = PathBuf::from("Resources")
                            .join(format!("{}.icon", alternate_icon(&name)));
                        files.extend(icon_document_files(&path, &dest)?);
                        icon_documents.push(dest);
                    }
                    icon::Source::Native(path) | icon::Source::Png(path) => {
                        let contents = std::fs::read(&path).map_err(|error| {
                            failed(format!("cannot read the icon {}: {error}", path.display()))
                        })?;
                        let ext = if icon::is_png_name(&path.display().to_string()) {
                            "png"
                        } else {
                            "icns"
                        };
                        let dest = PathBuf::from("Resources")
                            .join(format!("{}.{ext}", alternate_icon(&name)));
                        resource_files.push(dest.clone());
                        files.push(GeneratedFile {
                            path: dest,
                            contents,
                        });
                    }
                    icon::Source::Logo => {}
                }
            }
        }
        Platform::Ios | Platform::Watchos => {
            let platform_name = if platform == Platform::Ios {
                "ios"
            } else {
                "watchos"
            };
            let assets = PathBuf::from("Resources/Assets.xcassets");
            let set = assets.join("AppIcon.appiconset");
            files.push(GeneratedFile::text(
                assets.join("Contents.json"),
                "{\n  \"info\" : {\n    \"author\" : \"bern\",\n    \"version\" : 1\n  }\n}\n",
            ));
            if let Some(document) = icon_document(app, platform_name, &context.project_dir) {
                let dest = PathBuf::from("Resources/AppIcon.icon");
                files.extend(icon_document_files(&document, &dest)?);
                icon_documents.push(dest);
            } else if let icon::Source::Native(dir) =
                icon::source(app, platform_name, &context.project_dir)
            {
                for file in icon::app_icon_set(&dir).map_err(|m| {
                    failed(format!("the {platform_name} icon {} {m}", dir.display()))
                })? {
                    let contents = std::fs::read(dir.join(&file)).map_err(|error| {
                        failed(format!(
                            "cannot read {}: {error}",
                            dir.join(&file).display()
                        ))
                    })?;
                    files.push(GeneratedFile {
                        path: set.join(file),
                        contents,
                    });
                }
            } else {
                let images = icon::images(
                    app,
                    platform_name,
                    &context.project_dir,
                    icon::Shape::Masked,
                )
                .map_err(failed)?;
                files.push(GeneratedFile::text(
                    set.join("Contents.json"),
                    format!(
                        "{{\n  \"images\" : [\n    {{\n      \"filename\" : \"AppIcon.png\",\n      \"idiom\" : \"universal\",\n      \"platform\" : \"{platform_name}\",\n      \"size\" : \"1024x1024\"\n    }}\n  ],\n  \"info\" : {{\n    \"author\" : \"bern\",\n    \"version\" : 1\n  }}\n}}\n"
                    ),
                ));
                files.push(GeneratedFile {
                    path: set.join("AppIcon.png"),
                    contents: icon::largest(&images).png.to_vec(),
                });
            }
            if platform == Platform::Ios {
                for (name, source) in alternate_icons(app, "ios", &context.project_dir)? {
                    let asset = alternate_icon(&name);
                    let alt_set = assets.join(format!("{asset}.appiconset"));
                    match source {
                        icon::Source::Native(path) if icon::is_icon_document(&path) => {
                            let dest = PathBuf::from("Resources").join(format!("{asset}.icon"));
                            files.extend(icon_document_files(&path, &dest)?);
                            icon_documents.push(dest);
                        }
                        icon::Source::Native(dir) => {
                            for file in icon::app_icon_set(&dir).map_err(|m| {
                                failed(format!("the iOS icon {} {m}", dir.display()))
                            })? {
                                let contents = std::fs::read(dir.join(&file)).map_err(|error| {
                                    failed(format!(
                                        "cannot read {}: {error}",
                                        dir.join(&file).display()
                                    ))
                                })?;
                                files.push(GeneratedFile {
                                    path: alt_set.join(file),
                                    contents,
                                });
                            }
                        }
                        icon::Source::Png(path) => {
                            let contents = std::fs::read(&path).map_err(|error| {
                                failed(format!("cannot read {}: {error}", path.display()))
                            })?;
                            files.push(GeneratedFile::text(
                                alt_set.join("Contents.json"),
                                format!(
                                    "{{\n  \"images\" : [\n    {{\n      \"filename\" : \"{asset}.png\",\n      \"idiom\" : \"universal\",\n      \"platform\" : \"ios\",\n      \"size\" : \"1024x1024\"\n    }}\n  ],\n  \"info\" : {{\n    \"author\" : \"bern\",\n    \"version\" : 1\n  }}\n}}\n"
                                ),
                            ));
                            files.push(GeneratedFile {
                                path: alt_set.join(format!("{asset}.png")),
                                contents,
                            });
                        }
                        icon::Source::Logo => {}
                    }
                }
            }
            asset_catalog = Some(assets);
            asset_app_icon_name = Some("AppIcon".into());
        }
        Platform::Tvos => {
            let assets = PathBuf::from("Resources/Assets.xcassets");
            let set = assets.join("App Icon & Top Shelf Image.brandassets");
            files.push(GeneratedFile::text(
                assets.join("Contents.json"),
                "{\n  \"info\" : {\n    \"author\" : \"bern\",\n    \"version\" : 1\n  }\n}\n",
            ));
            let drawn = match icon::source(app, "tvos", &context.project_dir) {
                icon::Source::Native(dir) => {
                    let listed = brand_assets::check(&dir)
                        .map_err(|m| failed(format!("the tvOS icon {} {m}", dir.display())))?;
                    let mut read = Vec::new();
                    for file in listed {
                        let contents = std::fs::read(dir.join(&file)).map_err(|error| {
                            failed(format!(
                                "cannot read {}: {error}",
                                dir.join(&file).display()
                            ))
                        })?;
                        read.push((file, contents));
                    }
                    read
                }
                _ => {
                    let images =
                        icon::images(app, "tvos", &context.project_dir, icon::Shape::Masked)
                            .map_err(failed)?;
                    brand_assets::generate(&icon::largest(&images).png)
                        .map_err(|m| failed(format!("the tvOS icon {m}")))?
                }
            };
            for (path, contents) in drawn {
                files.push(GeneratedFile {
                    path: set.join(path),
                    contents,
                });
            }
            asset_catalog = Some(assets);
            asset_app_icon_name = Some("App Icon & Top Shelf Image".into());
        }
    }

    // 6. Declared resources and SVG static files under `Resources/`.
    // Top-level directories become Xcode folder references so nested paths
    // (`media/aurora.mp4`, `__bern_svg/...`) stay intact inside the bundle.
    let mut top_resource_dirs: BTreeSet<String> = BTreeSet::new();
    let declared_res = copy_declared_resources(app, context, Path::new("Resources"))?;
    for file in declared_res {
        if let Ok(rel) = file.path.strip_prefix("Resources") {
            let mut comps = rel.components();
            if let (Some(first), Some(_second)) = (comps.next(), comps.next()) {
                top_resource_dirs.insert(first.as_os_str().to_string_lossy().into_owned());
            } else {
                resource_files.push(file.path.clone());
            }
        }
        files.push(file);
    }
    for svg_file in bern_target_api::svg::static_files(app, context)? {
        let dest = PathBuf::from("Resources").join(&svg_file.path);
        if let Some(first) = svg_file.path.components().next() {
            if svg_file.path.components().count() > 1 {
                top_resource_dirs.insert(first.as_os_str().to_string_lossy().into_owned());
            } else {
                resource_files.push(dest.clone());
            }
        }
        files.push(GeneratedFile {
            path: dest,
            contents: svg_file.contents,
        });
    }
    for dir in top_resource_dirs {
        resource_folders.push(PathBuf::from("Resources").join(dir));
    }

    // 7. Xcode project & shared scheme.
    let pbx = generate_pbxproj(&PbxSpec {
        product: product.clone(),
        bundle_id: app.app.bundle_id.clone(),
        version: app.app.version.clone(),
        platform,
        deployment_target: deployment_target.clone(),
        default_language: app.languages.default.clone(),
        languages: app.languages.supported.clone(),
        swift_files,
        bridging_header,
        info_plist,
        resource_files,
        resource_folders,
        localization_groups,
        asset_catalog,
        icon_documents,
        asset_app_icon_name,
        engine: options.engine,
    });
    let xcodeproj = PathBuf::from(format!("{product}.xcodeproj"));
    files.push(GeneratedFile::text(xcodeproj.join("project.pbxproj"), pbx));
    files.push(GeneratedFile::text(
        xcodeproj
            .join("xcshareddata/xcschemes")
            .join(format!("{product}.xcscheme")),
        generate_xcscheme(&product),
    ));

    // 8. Rust engine wiring: either vendored source workspace or prebuilt XCFramework.
    let mut executables = Vec::new();
    let mut rust_manifests = Vec::new();
    let mut prebuilt_staging = Vec::new();
    let mut prebuilt_steps = Vec::new();

    let glue_cargo = glue::cargo_toml_for(context, app, options.with_cli);
    let glue_lib = glue::lib_rs(context, app, platform);

    match options.engine {
        EjectEngine::Source => {
            files.push(GeneratedFile::text("rust/glue/Cargo.toml", glue_cargo));
            files.push(GeneratedFile::text("rust/glue/src/lib.rs", glue_lib));
            rust_manifests.push(PathBuf::from("rust/glue/Cargo.toml"));
            if options.with_cli {
                files.push(GeneratedFile::text(
                    "rust/glue/src/cli.rs",
                    glue::cli_rs(context, app),
                ));
                if platform != Platform::Macos {
                    files.push(GeneratedFile::text(
                        "rust/tools/Cargo.toml",
                        glue::tools_cargo_toml(context, app),
                    ));
                    files.push(GeneratedFile::text(
                        "rust/tools/src/cli.rs",
                        glue::cli_rs(context, app),
                    ));
                    rust_manifests.push(PathBuf::from("rust/tools/Cargo.toml"));
                }
            }
            let script_path = PathBuf::from("scripts/build-rust.sh");
            files.push(GeneratedFile::text(
                script_path.clone(),
                build_rust_script(platform, &deployment_target),
            ));
            executables.push(script_path);
        }
        EjectEngine::Prebuilt => {
            prebuilt_staging.push(GeneratedFile::text("glue/Cargo.toml", glue_cargo));
            prebuilt_staging.push(GeneratedFile::text("glue/src/lib.rs", glue_lib));
            prebuilt_staging.push(GeneratedFile::text("headers/bern.h", HEADER));
            prebuilt_staging.push(GeneratedFile::text(
                "headers/module.modulemap",
                ENGINE_MODULEMAP,
            ));
            if options.with_cli {
                prebuilt_staging.push(GeneratedFile::text(
                    "glue/src/cli.rs",
                    glue::cli_rs(context, app),
                ));
            }
            if let Ok(lock) = std::fs::read(context.workspace_dir.join("Cargo.lock")) {
                prebuilt_staging.push(GeneratedFile {
                    path: "glue/Cargo.lock".into(),
                    contents: lock,
                });
            }
            bern_target_api::script::configure(&mut prebuilt_staging, context);

            let staging = &options.staging_dir;
            let profile = if options.release { "release" } else { "debug" };
            let glue_target = context.workspace_dir.join("target/bern-glue");
            let (rust_target, deploy_env, deploy_value): (&str, &str, String) = match platform {
                Platform::Macos => (
                    crate::architecture().1,
                    "MACOSX_DEPLOYMENT_TARGET",
                    MINIMUM_MACOS.into(),
                ),
                Platform::Ios => (
                    ios::simulator_rust_target(),
                    "IPHONEOS_DEPLOYMENT_TARGET",
                    deployment_target.clone(),
                ),
                Platform::Tvos => (
                    tvos::simulator_rust_target(),
                    "TVOS_DEPLOYMENT_TARGET",
                    tvos::MINIMUM_TVOS.into(),
                ),
                Platform::Watchos => (
                    watchos::simulator_rust_target(),
                    "WATCHOS_DEPLOYMENT_TARGET",
                    watchos::MINIMUM_WATCHOS.into(),
                ),
            };
            let mut cargo_args = vec![
                "build".to_string(),
                "--lib".into(),
                "--manifest-path".into(),
                staging.join("glue/Cargo.toml").display().to_string(),
                "--target".into(),
                rust_target.into(),
                "--target-dir".into(),
                glue_target.display().to_string(),
            ];
            if options.release {
                cargo_args.push("--release".into());
            }
            let rust_out = glue_target.join(rust_target).join(profile);
            let native_libs = native_libs_file(&rust_out);
            let frameworks_dir = context.output_dir.join("Frameworks");
            let xcframework = frameworks_dir.join("BernEngine.xcframework");

            prebuilt_steps.push(BuildStep::Run {
                description: format!("Build prebuilt Rust engine for {rust_target}"),
                program: "cargo".into(),
                args: listing_native_libs(&cargo_args, &native_libs),
                cwd: staging.join("glue"),
                env: vec![(deploy_env.to_string(), deploy_value.clone())],
            });
            prebuilt_steps.push(BuildStep::CreateDir {
                path: frameworks_dir.clone(),
            });
            prebuilt_steps.push(BuildStep::Copy {
                from: native_libs,
                to: frameworks_dir.join("bern_native_libs.resp"),
            });
            prebuilt_steps.push(BuildStep::Run {
                description: "Package BernEngine.xcframework".into(),
                program: "xcodebuild".into(),
                args: vec![
                    "-create-xcframework".into(),
                    "-library".into(),
                    rust_out.join("libbern_app.a").display().to_string(),
                    "-headers".into(),
                    staging.join("headers").display().to_string(),
                    "-output".into(),
                    xcframework.display().to_string(),
                ],
                cwd: context.output_dir.clone(),
                env: vec![],
            });
        }
    }

    if options.with_cli && skill::listens(&context.target.channels) {
        let cli_name = app.app.id.replace('_', "-");
        files.push(GeneratedFile::text(
            skill::path(app),
            skill::markdown(
                app,
                &skill::Place {
                    cli: cli_name,
                    installed: None,
                    connect: None,
                },
            ),
        ));
    }

    bern_target_api::script::configure(&mut files, context);

    let app_title = app.app.name.get(&app.languages.default).unwrap_or(&product);
    let platform_label = match platform {
        Platform::Macos => "macOS · SwiftUI",
        Platform::Ios => "iOS · SwiftUI",
        Platform::Tvos => "tvOS · SwiftUI",
        Platform::Watchos => "watchOS · SwiftUI",
    };
    let build_help = match options.engine {
        EjectEngine::Source => format!(
            "Open `{product}.xcodeproj` in Xcode and press **Cmd+R**.\n\nThe **Build Rust Engine** build phase runs `scripts/build-rust.sh`, compiling the self-contained Rust workspace in `rust/` automatically for the active SDK, architecture, and build configuration."
        ),
        EjectEngine::Prebuilt => format!(
            "Open `{product}.xcodeproj` in Xcode and press **Cmd+R**.\n\nNo Rust toolchain is required: the pre-compiled engine is packaged in `Frameworks/BernEngine.xcframework`."
        ),
    };
    let layout_help = "- `Sources/App/` — Application entry point and scenes\n- `Sources/Models/` — Protocol value types, engine state, and session state\n- `Sources/Styles/` — Styles, accents, patterns, and breakpoints\n- `Sources/Views/` — One SwiftUI file per UI component\n- `Sources/Windows/` — One SwiftUI file per window\n- `Sources/Runtime/` — Self-contained Bern SwiftUI bridge & C header (`bern.h`)\n- `Resources/` — `Info.plist`, app icons, localizations, and bundled assets";
    files.push(GeneratedFile::text(
        "README.md",
        readme(
            app_title,
            platform_label,
            options.engine,
            &[
                ("Opening & Building", &build_help),
                ("Project Layout", layout_help),
            ],
        ),
    ));

    let files = files.into_iter().map(clean_generated_file).collect();
    let notes = vec![format!(
        "Open `{product}.xcodeproj` in Xcode to build and edit."
    )];

    Ok(EjectedProject {
        files,
        executables,
        rust_manifests,
        rust_root: PathBuf::from("rust"),
        prebuilt_staging,
        prebuilt_steps,
        notes,
    })
}
