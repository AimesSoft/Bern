//! A toolbar extracted into a component still attaches to its owning page.
use bern_backend_swiftui::{Generator, Platform};
use std::{fs, path::Path};

const UI: &str = r#"
use bern::prelude::*;
#[bern::value]
pub enum Mode {
    #[label(tr! { "en" => "All" })]
    All,
    #[label(tr! { "en" => "Recent" })]
    Recent,
}
#[bern::session]
pub struct Draft { pub query: String, pub detail: bool, pub mode: Mode }

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
                Stack {
                    icon_palette(app: app)
                    default_picker()
                }
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

#[component]
pub fn icon_palette(app: probe::Client, #[session] draft: Draft) -> impl View {
    ui! {
        Toolbar {
            Spacer
            Search(id: search_icon, value: draft.query, style: palette, display: icon, prompt: tr! { "en" => "Search" })
            IconButton(id: open, icon: "folder", label: tr! { "en" => "Open" }) => app.add()
        }
    }
}

#[component]
pub fn default_picker(#[session] draft: Draft) -> impl View {
    ui! {
        Column {
            Picker(id: kind, label: tr! { "en" => "Kind" }, value: draft.mode)
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
        assert!(palette.contains("if !bernInert {"), "{palette}");
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
        let icon_palette = body("icon_palette");
        assert!(
            icon_palette.contains("iconOnly: true"),
            "icon palette should request icon-only trigger: {icon_palette}"
        );
        let default_picker = body("default_picker");
        if platform == Platform::Macos {
            assert!(
                default_picker.contains(".pickerStyle(.menu).tint(nil as SwiftUI.Color?)"),
                "default picker must drop accent tint on macOS: {default_picker}"
            );
        } else {
            assert!(
                default_picker.contains("SwiftUI.Menu {")
                    && default_picker.contains(".pickerStyle(.inline)"),
                "default picker on UIKit uses a neutral inline menu row: {default_picker}"
            );
        }
    }
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn inert_split_view_collapses_sidebar_and_backdrop_hides_scroll_edge() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = root.join(format!(
        "target/bern-test/covered-split-{}",
        std::process::id()
    ));
    let protocol = r#"
#[bern::protocol(namespace = "probe", version = 1)]
pub mod probe {
    use bern::prelude::*;
    #[value] pub enum Tab { Home, Archive }
    #[state] pub struct State { pub covering: bool }
    #[commands] pub trait Commands { fn close(&mut self) -> Result<(), CommandError>; }
}
"#;
    let ui = r#"
use bern::prelude::*;
use probe_protocol::probe::{self, Tab};
#[bern::session]
pub struct Draft { pub tab: Tab }

#[component]
pub fn app(app: probe::Client, #[session] draft: Draft) -> impl View {
    ui! {
        WindowSet(handle: AppWindows, primary: main) {
            Window(id: main, title: tr! { "en" => "Probe" }, chrome: unified, initial: open) {
                Stack {
                    Stack(enabled: !app.covering) {
                        SplitView {
                            Sidebar(id: nav, selection: draft.tab) {
                                SidebarItem(value: Tab::Home, label: tr! { "en" => "Home" }, icon: "house")
                                SidebarItem(value: Tab::Archive, label: tr! { "en" => "Archive" }, icon: "archivebox")
                            }
                            Detail(titled: false) {
                                Toolbar {
                                    Button(id: open_item, label: tr! { "en" => "Open" }) => app.close()
                                }
                                Text(content: tr! { "en" => "Body" })
                            }
                        }
                    }
                    if app.covering {
                        Stack(align: top_leading) {
                            Toolbar {
                                Button(id: back, label: tr! { "en" => "Back" }) => app.close()
                            }
                            cover_body()
                        }
                    }
                }
            }
        }
    }
}

#[component]
pub fn cover_body() -> impl View {
    ui! {
        Stack(align: top_leading) {
            Stack(under_bars: true, align: top_leading) {
                Canvas { Shape(kind: rect, fill: surface.canvas) }
            }
            ScrollView {
                Text(content: tr! { "en" => "Detail" })
            }
        }
    }
}
"#;
    for (package, source) in [
        ("probe-protocol", protocol),
        ("probe-ui", ui),
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
id = "covered_split_probe"
bundle_id = "dev.bern.covered-split-probe"
version = "0.1.0"
name = { en = "Covered Split Probe" }
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
    let swift = Generator::new(&compiled.app, Platform::Macos).generate(0);
    assert!(swift.contains("BernSplitView {"), "{swift}");
    let cover = swift
        .split_once("/// Generated from the `cover_body` component.")
        .unwrap()
        .1;
    assert_eq!(cover.matches(".bernScrollEdgeEffectHidden()").count(), 2);
    let _ = fs::remove_dir_all(&dir);
}
