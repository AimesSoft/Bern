//! Web pages opt into a native browsing environment without changing
//! their custom user agent or the default of existing declarations.

use bern_backend_swiftui::{SwiftUiIos, SwiftUiMacos};
use bern_target_api::{BuildContext, Framework, Target, feature};
use std::path::{Path, PathBuf};

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn project(name: &str, pages: &str) -> bern_compiler::Config {
    let dir = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| workspace().join("target"))
        .join("bern-test")
        .join(name);
    let protocol = r#"
#[bern::protocol(namespace = "pages", version = 1)]
pub mod pages {
    use bern::prelude::*;
    #[state]
    pub struct State {
        #[content]
        pub url: String,
        #[content]
        pub agent: String,
    }
    #[commands]
    pub trait Commands {
        fn close(&mut self) -> Result<(), CommandError>;
    }
}
"#;
    let ui = r#"
use bern::prelude::*;
#[component]
pub fn app(app: pages::Client, windows: AppWindows) -> impl View {
    ui! {
        WindowSet(handle: AppWindows, primary: main) {
            Window(id: main, title: tr! { "en" => "Pages" }, initial: open) {
                Column {
                    $PAGES
                }
            }
        }
    }
}
"#
    .replace("$PAGES", pages);
    for (package, source) in [
        ("pages-protocol", protocol),
        ("pages-ui", ui.as_str()),
        ("pages-core", ""),
    ] {
        std::fs::create_dir_all(dir.join(package).join("src")).unwrap();
        std::fs::write(
            dir.join(package).join("Cargo.toml"),
            format!("[package]\nname = \"{package}\"\n"),
        )
        .unwrap();
        std::fs::write(dir.join(package).join("src/lib.rs"), source).unwrap();
    }
    std::fs::write(
        dir.join("Bern.toml"),
        r#"
[app]
id = "pages"
bundle_id = "dev.bern.pages"
version = "0.1.0"
name = { "en" = "Pages" }
[languages]
supported = ["en"]
default = "en"
[protocol]
package = "pages-protocol"
source = "pages-protocol/src/lib.rs"
[engine]
package = "pages-core"
type = "pages_core::Engine"
[ui]
package = "pages-ui"
sources = ["pages-ui/src/lib.rs"]
root = "app"
[targets.ios]
platform = "ios"
frontends = ["swiftui", "swiftui-16"]
[targets.macos]
platform = "macos"
"#,
    )
    .unwrap();
    bern_compiler::config::load(&dir.join("Bern.toml")).unwrap()
}

#[test]
fn both_ios_front_ends_generate_native_modes_and_preserve_the_user_agent() {
    let config = project(
        "web-content-modes",
        r#"
WebPage(url: app.url, user_agent: app.agent) => app.close()
WebPage(url: app.url, user_agent: app.agent, content_mode: auto) => app.close()
WebPage(url: app.url, user_agent: app.agent, content_mode: mobile) => app.close()
WebPage(url: app.url, user_agent: app.agent, content_mode: desktop) => app.close()
"#,
    );
    let compiled = bern_compiler::compile(&config).unwrap_or_else(|errors| panic!("{errors:?}"));
    let target = config
        .targets
        .iter()
        .find(|target| target.name == "ios")
        .unwrap();
    let app = compiled.app.on_platform("ios");
    let diagnostics = SwiftUiIos.check(&app, target);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert!(
        SwiftUiIos
            .descriptor()
            .features
            .contains(&feature::WEB_CONTENT_MODE)
    );
    let context = BuildContext {
        project_dir: config.dir.clone(),
        workspace_dir: workspace(),
        output_dir: config.dir.join("generated"),
        target: target.clone(),
        protocol_package: config.protocol_package.name.clone(),
        protocol_path: config.protocol_package.dir.clone(),
        protocol_module: compiled.protocol_module.clone(),
        engine_package: config.engine_package.name.clone(),
        engine_path: config.engine_package.dir.clone(),
        engine_type: config.engine_type.clone(),
        modules: Vec::new(),
        framework: Framework::Path(workspace().join("crates")),
        release: false,
    };
    let files = SwiftUiIos.generate(&app, &context).unwrap();
    for sources in ["Sources", "Sources17"] {
        let file = files
            .iter()
            .find(|file| file.path == Path::new(sources).join("BernGenerated.swift"))
            .unwrap();
        let swift = std::str::from_utf8(&file.contents).unwrap();
        assert_eq!(swift.matches("contentMode: .auto").count(), 2, "{swift}");
        for mode in ["mobile", "desktop"] {
            assert_eq!(
                swift.matches(&format!("contentMode: .{mode}")).count(),
                1,
                "{swift}"
            );
        }
        assert_eq!(
            swift
                .matches("userAgent: BernText.text(bernEngine.agent)")
                .count(),
            4,
            "{swift}"
        );
    }

    // A custom UA is not a substitute on an engine without this feature.
    let target = config
        .targets
        .iter()
        .find(|target| target.name == "macos")
        .unwrap();
    let diagnostics = SwiftUiMacos.check(&compiled.app.on_platform("macos"), target);
    let modes: Vec<_> = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.message.contains("content_mode"))
        .collect();
    assert_eq!(modes.len(), 1, "{diagnostics:?}");
    assert!(!modes[0].is_error());
    assert!(modes[0].message.contains("default browsing environment"));
}

#[test]
fn omitted_and_auto_modes_do_not_require_a_backend_feature() {
    let config = project(
        "web-content-default",
        "WebPage(url: app.url) WebPage(url: app.url, content_mode: auto)",
    );
    let compiled = bern_compiler::compile(&config).unwrap_or_else(|errors| panic!("{errors:?}"));
    let target = config
        .targets
        .iter()
        .find(|target| target.name == "macos")
        .unwrap();
    let diagnostics = SwiftUiMacos.check(&compiled.app.on_platform("macos"), target);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn unknown_content_modes_are_rejected() {
    let config = project(
        "web-content-invalid",
        "WebPage(url: app.url, content_mode: tablet)",
    );
    let errors = bern_compiler::compile(&config)
        .err()
        .expect("unknown mode compiled");
    assert!(
        errors.iter().any(|error| {
            let message = error.to_string();
            message.contains("expected one of: auto, mobile, desktop")
        }),
        "{errors:?}"
    );
}

#[test]
fn native_content_mode_is_configured_before_the_web_view_is_created() {
    let template = include_str!("../templates/BernSvg.swift");
    let page = &template[template.find("private struct BernWebPageView").unwrap()..];
    let configured = page.find("switch contentMode").unwrap();
    let created = page.find("let view = WKWebView").unwrap();
    assert!(configured < created);
    assert!(page.contains("case .auto: break"));
    assert!(page.contains(
        "case .mobile: configuration.defaultWebpagePreferences.preferredContentMode = .mobile"
    ));
    assert!(page.contains(
        "case .desktop: configuration.defaultWebpagePreferences.preferredContentMode = .desktop"
    ));
    assert!(page.contains("if !userAgent.isEmpty { view.customUserAgent = userAgent }"));
    assert!(!page.contains("navigator.platform"));
}
