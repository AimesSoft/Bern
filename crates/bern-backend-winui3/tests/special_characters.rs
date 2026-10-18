//! Texts, labels, ids and literals holding characters that C++ string
//! literals and resource scripts treat specially: the generated sources
//! spell each one so it reads back as written. Nothing here needs the
//! Visual Studio tools.

use bern_backend_winui3::WinUi3Windows;
use bern_target_api::{BuildContext, Framework, Target};
use std::path::{Path, PathBuf};

/// What the project declares, as Rust spells it; each is written into the
/// declarations below where its `@NAME@` stands.
const TEXT: &str = "Say \"hi\" \\ back\nline $total \\(x) `tick` 100% </script> <b> & Ünïcødé";
const FORMATTED: &str = "Cost: ${price} (100%) \"q\" \\(y) `t`";
const PROMPT: &str = "e.g. \"Ann\" & 'Bo'";
const TITLE: &str = "Window \"title\" </script> & <i>";
const LABEL: &str = "Mode \"<fast>\" & \\slow\\ $mode";
const JAPANESE: &str = "日本語「引用」\"";
const LITERAL: &str = "raw \"q\" \\ \n $x ${x} \\(y) ` 50% </script> <&> é";
const COMPARED: &str = "${x}\"</script>\\";
// No `\`: the resource script does not escape it in the name yet.
const NAME: &str = "Bern's \"Q\" <&> </script> $x 100% é";

const PROTOCOL: &str = r#"
#[bern::protocol(namespace = "special", version = 1)]
pub mod special {
    use bern::prelude::*;

    #[value]
    pub enum Mode {
        #[label(tr! { "en" => @LABEL@, "ja" => "モード" })]
        Fast,
        #[label(tr! { "en" => "Slow", "ja" => "遅い" })]
        Slow,
    }

    #[state]
    pub struct State {
        pub title: String,
        pub mode: Mode,
    }

    #[commands]
    pub trait Commands {
        fn rename(&mut self, title: String) -> Result<(), CommandError>;
    }
}
"#;

const UI: &str = r#"
use bern::prelude::*;

#[bern::session]
pub struct Draft {
    pub name: String,
}

#[component]
pub fn app(app: special::Client, windows: AppWindows) -> impl View {
    ui! {
        WindowSet(handle: AppWindows, primary: main) {
            Window(id: main, title: tr! { "en" => @TITLE@, "ja" => "ウインドウ" }, initial: open) {
                page(app: app)
            }
        }
    }
}

#[component]
pub fn page(app: special::Client, #[session] draft: Draft) -> impl View {
    ui! {
        Column {
            Text(content: tr! { "en" => @TEXT@, "ja" => @JAPANESE@ })
            Text(content: tr! { "en" => @FORMATTED@, "ja" => "価格 ${price} 100%"; price: draft.name })
            TextInput(id: "name-field", label: tr! { "en" => "Name", "ja" => "名前" }, prompt: tr! { "en" => @PROMPT@, "ja" => "例" }, value: draft.name)
            Button(id: "save-as", label: tr! { "en" => "Save", "ja" => "保存" }) => app.rename(@LITERAL@)
            if draft.name == @COMPARED@ {
                Text(content: tr! { "en" => "Matched", "ja" => "一致" })
            }
            Text(content: app.mode)
        }
    }
}
"#;

const CONFIG: &str = r#"
[app]
id = "special"
bundle_id = "dev.bern.special"
version = "0.1.0"
name = { "en" = @NAME@, "ja" = "ベルン" }

[languages]
supported = ["en", "ja"]
default = "en"

[protocol]
package = "special-protocol"
source = "special-protocol/src/lib.rs"

[engine]
package = "special-core"
type = "special_core::Engine"

[ui]
package = "special-ui"
sources = ["special-ui/src/lib.rs"]
root = "app"

[targets.windows]
platform = "windows"
"#;

fn workspace() -> PathBuf {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    // Without the verbatim prefix Windows gives canonical paths.
    match root.to_string_lossy().strip_prefix(r"\\?\") {
        Some(plain) => PathBuf::from(plain),
        None => root,
    }
}

/// `source` with every `@NAME@` replaced by that constant as a quoted
/// literal, which Rust and TOML read alike for these characters.
fn fill(source: &str) -> String {
    let mut out = source.to_string();
    for (name, value) in [
        ("TEXT", TEXT),
        ("FORMATTED", FORMATTED),
        ("PROMPT", PROMPT),
        ("TITLE", TITLE),
        ("LABEL", LABEL),
        ("JAPANESE", JAPANESE),
        ("LITERAL", LITERAL),
        ("COMPARED", COMPARED),
        ("NAME", NAME),
    ] {
        out = out.replace(&format!("@{name}@"), &format!("{value:?}"));
    }
    assert!(!out.contains('@'), "an unknown `@NAME@` in {out}");
    out
}

/// Write the project, compile it and generate it for Windows into
/// `target/bern-test/winui3-special-characters`.
fn generate() -> PathBuf {
    let root = workspace();
    let dir = root.join("target/bern-test/winui3-special-characters-project");
    let _ = std::fs::remove_dir_all(&dir);
    for (package, source) in [
        ("special-protocol", fill(PROTOCOL)),
        ("special-ui", fill(UI)),
        ("special-core", String::new()),
    ] {
        std::fs::create_dir_all(dir.join(package).join("src")).unwrap();
        std::fs::write(
            dir.join(package).join("Cargo.toml"),
            format!("[package]\nname = \"{package}\"\n"),
        )
        .unwrap();
        std::fs::write(dir.join(package).join("src/lib.rs"), source).unwrap();
    }
    std::fs::write(dir.join("Bern.toml"), fill(CONFIG)).unwrap();
    let config = bern_compiler::config::load(&dir.join("Bern.toml")).unwrap();
    let compiled = bern_compiler::compile(&config).unwrap_or_else(|errors| {
        panic!(
            "{}",
            errors
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n")
        )
    });
    let target = config.targets[0].clone();
    let problems = WinUi3Windows.check(&compiled.app, &target);
    assert!(problems.is_empty(), "{problems:?}");
    let output = root.join("target/bern-test/winui3-special-characters");
    let _ = std::fs::remove_dir_all(&output);
    let context = BuildContext {
        project_dir: config.dir.clone(),
        workspace_dir: root.clone(),
        output_dir: output.clone(),
        target,
        protocol_package: config.protocol_package.name.clone(),
        protocol_path: config.protocol_package.dir.clone(),
        protocol_module: compiled.protocol_module.clone(),
        engine_package: config.engine_package.name.clone(),
        engine_path: config.engine_package.dir.clone(),
        engine_type: config.engine_type.clone(),
        modules: Vec::new(),
        framework: Framework::Path(root.join("crates")),
        release: false,
    };
    for file in WinUi3Windows.generate(&compiled.app, &context).unwrap() {
        let path = output.join(&file.path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, &file.contents).unwrap();
    }
    output
}

fn read(path: PathBuf) -> String {
    std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

#[test]
fn special_characters_are_escaped_in_cpp_and_the_resources() {
    let output = generate();
    let ui = read(output.join("app/app_ui.inl"));
    for expected in [
        // Texts come from the string tables.
        r#"return bern::Text::Tr("page.0.0.content");"#,
        r#"return bern::Text::Fmt("page.0.1.content", {{"price", R::Plain(c.Session(0, 0))}});"#,
        // Literals in actions and conditions.
        r#"Dispatch(c, 0, Values{Value::Text("raw \"q\" \\ \n $x ${x} \\(y) ` 50% </script> <&> é")});"#,
        r#"R::Eq(c.Session(0, 0), Value::Text("${x}\"</script>\\"))"#,
        // Ids name the controls.
        r#"B::TextInput(p, c, {.id = "name-field","#,
        r#"B::Button(p, c, {.id = "save-as","#,
    ] {
        assert!(
            ui.contains(expected),
            "app_ui.inl lacks `{expected}`:\n{ui}"
        );
    }
    let catalog = read(output.join("app/app_catalog.cpp"));
    for expected in [
        // The string tables, in both languages; placeholders stay named.
        r#"{"page.0.0.content", L"Say \"hi\" \\ back\nline $total \\(x) `tick` 100% </script> <b> & Ünïcødé"},"#,
        r#"{"page.0.1.content", L"Cost: ${price} (100%) \"q\" \\(y) `t`"},"#,
        r#"{"page.0.2.prompt", L"e.g. \"Ann\" & 'Bo'"},"#,
        r#"{"app.0.0.title", L"Window \"title\" </script> & <i>"},"#,
        r#"{"Mode.Fast", L"Mode \"<fast>\" & \\slow\\ $mode"},"#,
        r#"{"app.name", L"Bern's \"Q\" <&> </script> $x 100% é"},"#,
        r#"{"page.0.0.content", L"日本語「引用」\""},"#,
    ] {
        assert!(
            catalog.contains(expected),
            "app_catalog.cpp lacks `{expected}`:\n{catalog}"
        );
    }
    // Nothing is written unescaped: each of these would end its literal.
    for raw in [
        TEXT, FORMATTED, PROMPT, TITLE, LABEL, JAPANESE, LITERAL, COMPARED, NAME,
    ] {
        assert!(!ui.contains(raw), "app_ui.inl holds `{raw}` unescaped");
        assert!(
            !catalog.contains(raw),
            "app_catalog.cpp holds `{raw}` unescaped"
        );
    }
    // Literals outside ASCII are UTF-8: the sources compile with `/utf-8`.
    let build = read(output.join("app/build.bat"));
    assert!(build.contains(" /utf-8 "), "{build}");

    // The version resource doubles quotes and reads UTF-8.
    let resources = read(output.join("app/app.rc"));
    assert!(
        resources.contains("#pragma code_page(65001)"),
        "{resources}"
    );
    assert!(
        resources.contains(r#"VALUE "ProductName", "Bern's ""Q"" <&> </script> $x 100% é""#),
        "{resources}"
    );
}
