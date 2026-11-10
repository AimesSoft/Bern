//! A toolbar extracted into a component still attaches to its owning page.
use bern_backend_swiftui::{Generator, Platform};
use std::{fs, path::Path};

const UI: &str = r#"
use bern::prelude::*;
#[bern::session]
pub struct Draft { pub query: String, pub detail: bool }

#[component]
pub fn app(app: probe::Client, windows: AppWindows) -> impl View {
    ui! {
        WindowSet(handle: AppWindows, primary: main) {
            Window(id: main, title: tr! { "en" => "Probe" }, initial: open) {
                page(app: app)
            }
        }
    }
}

#[component]
pub fn page(app: probe::Client, #[session] draft: Draft) -> impl View {
    ui! {
        if draft.detail {
            Stack {
                alias(app: app, title: app.title)
                Text(content: tr! { "en" => "Detail content" })
            }
        } else {
            Column {
                palette(app: app, title: app.title)
                Text(content: tr! { "en" => "List content" })
                nested(app: app)
            }
        }
    }
}

#[component]
pub fn alias(app: probe::Client, #[content] title: String) -> impl View {
    ui! { palette(app: app, title: title) }
}

#[component]
pub fn palette(app: probe::Client, #[content] title: String, #[session] draft: Draft) -> impl View {
    ui! {
        Toolbar {
            if draft.detail {
                Button(id: back, label: title, shortcut: "cmd+[") => draft.detail.set(false)
            }
            Spacer
            Search(id: search, value: draft.query, style: palette, shortcut: "cmd+k") {
                Text(content: title)
            }
            Button(id: add, label: title) => app.add()
        }
    }
}

#[component]
pub fn nested(app: probe::Client) -> impl View {
    ui! {
        Column {
            field(app: app)
            Text(content: tr! { "en" => "Nested content" })
        }
    }
}

#[component]
pub fn field(app: probe::Client, #[session] draft: Draft) -> impl View {
    ui! {
        Toolbar {
            Search(id: field, value: draft.query, prompt: tr! { "en" => "Find" })
            Button(id: add, label: tr! { "en" => "Add" }) => app.add()
        }
    }
}
"#;

#[test]
fn toolbar_components_attach_search_and_actions_to_the_container() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = root.join(format!(
        "target/bern-test/toolbar-components-{}",
        std::process::id()
    ));
    for (package, source) in [
        (
            "probe-protocol",
            r#"
#[bern::protocol(namespace = "probe", version = 1)]
pub mod probe {
    use bern::prelude::*;
    #[state] pub struct State { pub count: i64, #[content] pub title: String }
    #[commands] pub trait Commands { fn add(&mut self) -> Result<(), CommandError>; }
}
"#,
        ),
        ("probe-ui", UI),
        ("probe-core", ""),
    ] {
        fs::create_dir_all(dir.join(package).join("src")).unwrap();
        fs::write(
            dir.join(package).join("Cargo.toml"),
            format!("[package]\nname = \"{package}\"\n"),
        )
        .unwrap();
        fs::write(dir.join(package).join("src/lib.rs"), source).unwrap();
    }
    fs::write(
        dir.join("Bern.toml"),
        r#"
[app]
id = "toolbar_probe"
bundle_id = "dev.bern.toolbar-probe"
version = "0.1.0"
name = { en = "Toolbar Probe" }
[languages]
supported = ["en"]
default = "en"
[protocol]
package = "probe-protocol"
source = "probe-protocol/src/lib.rs"
[engine]
package = "probe-core"
type = "probe_core::Engine"
[ui]
package = "probe-ui"
sources = ["probe-ui/src/lib.rs"]
root = "app"
[targets.macos]
platform = "macos"
"#,
    )
    .unwrap();
    let config = bern_compiler::config::load(&dir.join("Bern.toml")).unwrap();
    let compiled = bern_compiler::compile(&config).unwrap_or_else(|errors| panic!("{errors:?}"));
    for platform in [Platform::Macos, Platform::Ios, Platform::Tvos] {
        let swift = Generator::new(&compiled.app, platform).generate(0);
        let body = |name: &str| {
            let marker = format!("/// Generated from the `{name}` component.");
            let rest = swift.split_once(&marker).unwrap().1;
            rest.split("/// Generated from")
                .next()
                .unwrap()
                .split("// MARK: - Windows")
                .next()
                .unwrap()
        };
        let palette = body("palette");
        assert!(palette.contains(": ViewModifier {"), "{palette}");
        assert!(
            palette.contains("func body(content bernContent: Content) -> some View {"),
            "{palette}"
        );
        assert!(
            palette.contains(".modifier(BernPaletteDetail())"),
            "{palette}"
        );
        assert!(palette.contains(".toolbar {"), "{palette}");
        for target in ["palette.back", "palette.search", "palette.add"] {
            assert!(palette.contains(target), "missing {target}: {palette}");
        }
        assert!(palette.contains("BernSearchPalette(text:"), "{palette}");
        assert!(palette.contains(".navigation"), "{palette}");
        assert!(palette.contains("bernEngine.add("), "{palette}");
        assert!(
            palette.contains("@Environment(BernEngine.self)"),
            "{palette}"
        );
        assert!(
            palette.contains("@Environment(App_Draft.self)"),
            "{palette}"
        );
        let alias = body("alias");
        assert!(alias.contains(": ViewModifier {"), "{alias}");
        assert!(
            alias.contains(".modifier(AppView_palette(title: title))"),
            "{alias}"
        );
        let page = body("page");
        assert!(
            page.contains(".modifier(AppView_alias(title: bernEngine.title))"),
            "{page}"
        );
        assert!(
            page.contains(".modifier(AppView_palette(title: bernEngine.title))"),
            "{page}"
        );
        assert!(page.contains("AppView_nested()"), "{page}");
        assert!(!page.contains(".modifier(AppView_nested("), "{page}");
        assert!(
            !page.contains(".modifier(AppView_field("),
            "nested toolbar leaked: {page}"
        );
        let field = body("field");
        assert!(field.contains(": ViewModifier {"), "{field}");
        assert!(field.contains(".modifier(BernSearchable(text:"), "{field}");
        assert!(field.contains("field.field"), "{field}");
        let nested = body("nested");
        assert!(nested.contains(": View {"), "{nested}");
        assert!(nested.contains(".modifier(AppView_field())"), "{nested}");
    }
}
