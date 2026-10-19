//! Texts, labels, ids and literals holding characters that Kotlin string
//! templates, JSON and Android string resources treat specially: the
//! generated sources spell each one so it reads back as written. Nothing
//! here needs Gradle.

use bern_backend_compose::ComposeAndroid;
use bern_target_api::{BuildContext, Framework, Target};
use std::collections::BTreeMap;
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
const NAME: &str = "Bern's \"Q\" <&> </script> $x 100% \\(y) é";

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

[targets.android]
platform = "android"
channels = ["ipc"]
"#;

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
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

/// Write the project, compile it and generate it for Android into
/// `target/bern-test/compose-special-characters`.
fn generate() -> PathBuf {
    let root = workspace();
    let dir = root.join("target/bern-test/compose-special-characters-project");
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
    let problems = ComposeAndroid.check(&compiled.app, &target);
    assert!(problems.is_empty(), "{problems:?}");
    let output = root.join("target/bern-test/compose-special-characters");
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
    for file in ComposeAndroid.generate(&compiled.app, &context).unwrap() {
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
fn special_characters_are_escaped_in_kotlin_and_the_resources() {
    let output = generate();
    let main = output.join("project/app/src/main");
    let kotlin = read(main.join("kotlin/dev/bern/special/BernGenerated.kt"));
    for expected in [
        // Texts come from the string tables.
        r#"BernText(BernText.string("page.0.0.content"), bernTextStyle("#,
        r#"BernText(BernText.format("page.0.1.content", mapOf("price" to BernText.arg(bernDraft.name))),"#,
        // Literals in actions and conditions: `$` starts no template.
        r#"BernEngine.rename("raw \"q\" \\ \n \$x \${x} \\(y) ` 50% </script> <&> é", bernWindow)"#,
        r#"if ((bernDraft.name) == ("\${x}\"</script>\\")) {"#,
        // Ids name the controls.
        r#"BernControl("page.name-field", "text_input","#,
        r#"Modifier.testTag("page.save-as")"#,
    ] {
        assert!(
            kotlin.contains(expected),
            "BernGenerated.kt lacks `{expected}`:\n{kotlin}"
        );
    }
    // Nothing is written unescaped: each of these would end its literal.
    for raw in [
        TEXT, FORMATTED, PROMPT, TITLE, LABEL, JAPANESE, LITERAL, COMPARED, NAME,
    ] {
        assert!(
            !kotlin.contains(raw),
            "BernGenerated.kt holds `{raw}` unescaped"
        );
    }

    // The string tables are JSON; a text with arguments keeps Bern's
    // format, which the runtime fills by name.
    let english = read(main.join("assets/bern/texts/en.json"));
    for expected in [
        r#""page.0.0.content": "Say \"hi\" \\ back\nline $total \\(x) `tick` 100% </script> <b> & Ünïcødé""#,
        r#""page.0.1.content": "Cost: ${price} (100%) \"q\" \\(y) `t`""#,
        r#""page.0.2.prompt": "e.g. \"Ann\" & 'Bo'""#,
        r#""app.0.0.title": "Window \"title\" </script> & <i>""#,
        r#""Mode.Fast": "Mode \"<fast>\" & \\slow\\ $mode""#,
        r#""app.name": "Bern's \"Q\" <&> </script> $x 100% \\(y) é""#,
    ] {
        assert!(
            english.contains(expected),
            "en.json lacks `{expected}`:\n{english}"
        );
    }
    let japanese = read(main.join("assets/bern/texts/ja.json"));
    // Each entry reads back as declared.
    let english: BTreeMap<String, String> = serde_json::from_str(&english).unwrap();
    let japanese: BTreeMap<String, String> = serde_json::from_str(&japanese).unwrap();
    for (key, value) in [
        ("page.0.0.content", TEXT),
        ("page.0.1.content", FORMATTED),
        ("page.0.2.prompt", PROMPT),
        ("app.0.0.title", TITLE),
        ("Mode.Fast", LABEL),
        ("app.name", NAME),
    ] {
        assert_eq!(english[key], value, "en.json `{key}`");
    }
    assert_eq!(japanese["page.0.0.content"], JAPANESE);
    assert_eq!(japanese["page.0.1.content"], "価格 ${price} 100%");

    // The name the launcher shows, in the syntax of string resources.
    let names = read(main.join("res/values/strings.xml"));
    assert!(
        names.contains(
            r#"<string name="app_name">Bern\'s \"Q\" &lt;&amp;&gt; &lt;/script&gt; $x 100% \\(y) é</string>"#
        ),
        "{names}"
    );
    let names = read(main.join("res/values-b+ja/strings.xml"));
    assert!(
        names.contains(r#"<string name="app_name">ベルン</string>"#),
        "{names}"
    );
}
