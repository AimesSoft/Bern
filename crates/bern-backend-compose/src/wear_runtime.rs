//! Wear composes the platform-independent runtime with native watch controls.
//!
//! The shared Kotlin files explicitly mark phone-only implementations. Imports
//! are collected separately, so no signature or qualified-name rewriting is
//! involved. Protocol, JNI, state, media, pictures, gestures, drawing and style
//! implementations remain a single source shared by Android and Wear OS.

use bern_target_api::GeneratedFile;
use std::path::Path;

fn shared(source: &str) -> String {
    let mut imports = Vec::new();
    let mut body = String::new();
    let mut skipping = false;
    for line in source.lines() {
        match line.trim() {
            "// @bern:wear-skip-start" => {
                assert!(!skipping, "nested Wear runtime exclusion");
                skipping = true;
            }
            "// @bern:wear-skip-end" => {
                assert!(skipping, "unmatched Wear runtime exclusion");
                skipping = false;
            }
            _ if skipping => {}
            _ if line.starts_with("package ") => {}
            _ if line.starts_with("import ") => {
                // Phone Material imports belong to the excluded controls, or
                // to semantic primitives supplied by Wear Material below.
                if !line.starts_with("import androidx.compose.material3.")
                    && line != "import androidx.compose.ui.window.Dialog"
                {
                    imports.push(line);
                }
            }
            _ => {
                body.push_str(line);
                body.push('\n');
            }
        }
    }
    assert!(!skipping, "unclosed Wear runtime exclusion");
    format!(
        "package dev.bern.runtime\n\n{}\nimport androidx.wear.compose.material3.*\n\n{}",
        imports.join("\n"),
        body
    )
}

pub(crate) fn files(engine_kinds: &str, package: &str) -> Vec<GeneratedFile> {
    let runtime = Path::new("project/app/src/main/kotlin/dev/bern/runtime");
    let common = [
        ("BernRuntime.kt", super::RUNTIME.to_owned()),
        ("BernViews.kt", super::views_source()),
        ("BernStyle.kt", super::STYLE.to_owned()),
        ("BernFeatures.kt", super::features_source(engine_kinds)),
        ("BernScene.kt", super::SCENE.to_owned()),
        (
            "BernMedia.kt",
            super::MEDIA.replace("__APP_PACKAGE__", package),
        ),
    ];
    let mut files: Vec<_> = common
        .into_iter()
        .map(|(name, source)| GeneratedFile::text(runtime.join(name), shared(&source)))
        .collect();
    for (name, source) in [
        (
            "BernWearRoot.kt",
            include_str!("../templates/wear/BernWearRoot.kt"),
        ),
        (
            "BernWearViews.kt",
            include_str!("../templates/wear/BernWearViews.kt"),
        ),
        (
            "BernWearPanels.kt",
            include_str!("../templates/wear/BernWearPanels.kt"),
        ),
        (
            "BernWearMedia.kt",
            include_str!("../templates/wear/BernWearMedia.kt"),
        ),
        (
            "BernPictureTries.kt",
            include_str!("../templates/BernPictureTries.kt"),
        ),
        (
            "BernAppIcon.kt",
            include_str!("../templates/BernAppIcon.kt"),
        ),
    ] {
        files.push(GeneratedFile::text(runtime.join(name), source));
    }
    files
}

#[cfg(test)]
mod tests {
    #[test]
    fn shared_runtime_has_no_phone_material_or_rewritten_signatures() {
        let files = super::files("audio", "dev.bern.probe");
        assert!(files.len() >= 10);
        for file in files {
            let source = String::from_utf8(file.contents).unwrap();
            assert!(!source.contains("androidx.compose.material3"));
            assert!(!source.contains("TopAppBar("));
            assert!(!source.contains("NavigationBar("));
            assert!(!source.contains("/*bern:engine-opens*/"));
            assert!(!source.contains("/*bern:icon-branches*/"));
        }
    }
}
