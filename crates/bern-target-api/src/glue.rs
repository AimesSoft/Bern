//! Rust source for generated glue crates.
//!
//! Targets that link the engine and the session runtime into a product
//! generate a small crate around `bern-ffi`. These helpers write the IR's
//! schema and session description as Rust expressions, so every target
//! describes the same session layer the same way.

use bern_ir::Application;
use bern_protocol::{EnumDef, FieldDef, RecordDef, Type, TypeTable, Value};
use std::path::Path;

const P: &str = "bern_ffi::protocol";

pub fn type_expr(ty: &Type) -> String {
    match ty {
        Type::Unit => format!("{P}::Type::Unit"),
        Type::Bool => format!("{P}::Type::Bool"),
        Type::Int => format!("{P}::Type::Int"),
        Type::Float => format!("{P}::Type::Float"),
        Type::Text => format!("{P}::Type::Text"),
        Type::Enum { name } => format!("{P}::Type::Enum {{ name: {name:?}.into() }}"),
        Type::Record { name } => format!("{P}::Type::Record {{ name: {name:?}.into() }}"),
        Type::Optional { inner } => format!("{P}::Type::optional({})", type_expr(inner)),
        Type::List { inner } => format!("{P}::Type::list({})", type_expr(inner)),
    }
}

pub fn value_expr(value: &Value) -> String {
    let list = |values: &[Value]| values.iter().map(value_expr).collect::<Vec<_>>().join(", ");
    match value {
        Value::Unit => format!("{P}::Value::Unit"),
        Value::Bool(value) => format!("{P}::Value::Bool({value})"),
        Value::Int(value) => format!("{P}::Value::Int({value})"),
        Value::Float(value) => format!("{P}::Value::Float({value:?})"),
        Value::Text(value) => format!("{P}::Value::Text({value:?}.into())"),
        Value::Enum { variant, fields } => format!(
            "{P}::Value::Enum {{ variant: {variant}, fields: vec![{}] }}",
            list(fields)
        ),
        Value::Optional(None) => format!("{P}::Value::Optional(None)"),
        Value::Optional(Some(value)) => format!("{P}::Value::some({})", value_expr(value)),
        Value::Record(values) => format!("{P}::Value::Record(vec![{}])", list(values)),
        Value::List(values) => format!("{P}::Value::List(vec![{}])", list(values)),
    }
}

fn field_expr(field: &FieldDef) -> String {
    format!(
        "{P}::FieldDef {{ name: {:?}.into(), ty: {}, doc: String::new(), content: {} }}",
        field.name,
        type_expr(&field.ty),
        field.content
    )
}

fn fields_expr(fields: &[FieldDef]) -> String {
    fields.iter().map(field_expr).collect::<Vec<_>>().join(", ")
}

fn enum_expr(def: &EnumDef) -> String {
    let variants: Vec<_> = def
        .variants
        .iter()
        .map(|variant| {
            // A label keeps its key and the fields it shows; its words
            // come from the string tables.
            let label = match &variant.label {
                Some(label) => format!(
                    "Some({P}::LocalizedText {{ key: {:?}.into(), translations: ::std::default::Default::default(), params: vec![{}] }})",
                    label.key,
                    label
                        .params
                        .iter()
                        .map(|param| format!("{param:?}.into()"))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                None => "None".into(),
            };
            format!(
                "{P}::VariantDef {{ name: {:?}.into(), fields: vec![{}], label: {label} }}",
                variant.name,
                fields_expr(&variant.fields)
            )
        })
        .collect();
    format!(
        "{P}::EnumDef {{ name: {:?}.into(), variants: vec![{}] }}",
        def.name,
        variants.join(", ")
    )
}

fn record_expr(def: &RecordDef) -> String {
    format!(
        "{P}::RecordDef {{ name: {:?}.into(), fields: vec![{}] }}",
        def.name,
        fields_expr(&def.fields)
    )
}

pub fn types_expr(types: &TypeTable) -> String {
    format!(
        "{P}::TypeTable {{ enums: vec![{}], records: vec![{}] }}",
        types
            .enums
            .iter()
            .map(enum_expr)
            .collect::<Vec<_>>()
            .join(", "),
        types
            .records
            .iter()
            .map(record_expr)
            .collect::<Vec<_>>()
            .join(", ")
    )
}

/// How the target presents every window kind, as a `Presentation` variant.
#[derive(Clone, Copy)]
pub enum WindowPresentation {
    Native,
    Virtual,
}

/// The `bern_ffi::runtime::SessionSpec` of an application.
pub fn session_spec(app: &Application, presentation: WindowPresentation) -> String {
    let presentation = match presentation {
        WindowPresentation::Native => "bern_ffi::runtime::Presentation::Native",
        WindowPresentation::Virtual => "bern_ffi::runtime::Presentation::Virtual",
    };
    session_spec_with(app, &|_| presentation.to_string())
}

/// The `bern_ffi::runtime::SessionSpec` of an application whose window
/// kinds present as the Rust expression `presentation` returns for each,
/// for targets that choose at run time.
pub fn session_spec_with(
    app: &Application,
    presentation: &dyn Fn(&bern_ir::WindowDef) -> String,
) -> String {
    let sessions: Vec<String> = app
        .sessions
        .iter()
        .map(|session| {
            let fields: Vec<String> = session
                .fields
                .iter()
                .map(|field| {
                    format!(
                        "({:?}.into(), {}, {})",
                        field.name,
                        type_expr(&field.ty),
                        value_expr(&field.initial)
                    )
                })
                .collect();
            format!(
                "bern_ffi::SessionType {{ name: {:?}.into(), fields: vec![{}] }}",
                session.name,
                fields.join(", ")
            )
        })
        .collect();
    let windows: Vec<String> = app
        .windows
        .windows
        .iter()
        .map(|window| {
            let indices: Vec<String> = window
                .sessions
                .iter()
                .filter_map(|name| {
                    app.sessions
                        .iter()
                        .position(|session| session.name == *name)
                })
                .map(|index| index.to_string())
                .collect();
            format!(
                "bern_ffi::WindowKind {{ id: {:?}.into(), sessions: vec![{}], presentation: {} }}",
                window.id,
                indices.join(", "),
                presentation(window)
            )
        })
        .collect();
    format!(
        "bern_ffi::runtime::SessionSpec {{\n            types: {},\n            sessions: vec![\n                {}\n            ],\n            windows: vec![\n                {}\n            ],\n        }}",
        types_expr(&app.types),
        sessions.join(",\n                "),
        windows.join(",\n                ")
    )
}

/// `vec![…]` items for the application's players and declared resources.
pub fn players_expr(app: &Application) -> String {
    app.players
        .iter()
        .map(|player| format!("{:?}.into()", player.id))
        .collect::<Vec<_>>()
        .join(", ")
}

pub fn resources_expr(app: &Application) -> String {
    app.resources
        .iter()
        .map(|resource| format!("{:?}.into()", resource.name))
        .collect::<Vec<_>>()
        .join(", ")
}

/// The expression creating the application engine: the protocol adapter
/// around the engine, or with modules the composite of the shell and every
/// module engine.
pub fn engine_expr(context: &crate::BuildContext) -> String {
    let adapter = |protocol: &str, engine: &str| {
        format!("{protocol}::Adapter(<{engine} as ::std::default::Default>::default())")
    };
    let shell = adapter(&context.protocol_module, &context.engine_type);
    if context.modules.is_empty() {
        return shell;
    }
    let modules: Vec<String> = context
        .modules
        .iter()
        .map(|module| {
            let depends: Vec<String> = module
                .depends
                .iter()
                .map(|id| format!("{id:?}.to_string()"))
                .collect();
            format!(
                "::bern::__private::command::ModuleEngine {{ descriptor: {}::descriptor(), start: ::std::boxed::Box::new(|| ::std::boxed::Box::new({})), optional: {}, enabled: {}, depends: ::std::vec![{}] }}",
                module.protocol_module,
                adapter(&module.protocol_module, &module.engine_type),
                module.optional,
                module.enabled,
                depends.join(", ")
            )
        })
        .collect();
    format!(
        "::bern::__private::command::Composite::new(::std::boxed::Box::new({shell}), ::std::vec![{}]).unwrap_or_else(|error| panic!(\"{{error}}\"))",
        modules.join(", ")
    )
}

/// The expression giving the application catalog as a
/// `&'static ProtocolDescriptor`.
pub fn descriptor_expr(context: &crate::BuildContext) -> String {
    let shell = format!("{}::descriptor()", context.protocol_module);
    if context.modules.is_empty() {
        return shell;
    }
    let modules: Vec<String> = context
        .modules
        .iter()
        .map(|module| {
            format!(
                "::bern::__private::protocol::ModulePart {{ descriptor: {}::descriptor(), optional: {} }}",
                module.protocol_module, module.optional
            )
        })
        .collect();
    format!(
        "::std::boxed::Box::leak(::std::boxed::Box::new(::bern::__private::protocol::compose({shell}, &[{}]).unwrap_or_else(|error| panic!(\"{{error}}\"))))",
        modules.join(", ")
    )
}

/// Cargo dependency lines for the modules' protocol crates, their engine
/// crates when `engines` is set, and `bern` itself, which composes them.
pub fn module_dependencies(context: &crate::BuildContext, engines: bool) -> String {
    if context.modules.is_empty() {
        return String::new();
    }
    let path = |path: &std::path::Path| path.display().to_string().replace('\\', "/");
    let mut lines = vec![format!(
        "bern = {{ {} }}",
        context.framework.dependency("bern")
    )];
    for module in &context.modules {
        lines.push(format!(
            "{} = {{ path = \"{}\" }}",
            module.protocol_package,
            path(&module.protocol_path)
        ));
        if engines {
            lines.push(format!(
                "{} = {{ path = \"{}\" }}",
                module.engine_package,
                path(&module.engine_path)
            ));
        }
    }
    lines.join("\n") + "\n"
}

/// A path as generated manifests and sources write it, with `/` between
/// components on every system.
pub fn path(path: &Path) -> String {
    path.display().to_string().replace('\\', "/")
}

/// What the `[profile.release]` of every generated crate sets: link-time
/// optimization across all crates in one code generation unit, and
/// programs without a symbol table. Packages ship release builds, and
/// this keeps the command line and the MCP server beside an app small;
/// debug builds keep Cargo's defaults.
pub const RELEASE_PROFILE: &str = "codegen-units = 1\nlto = true\nstrip = true\n";

/// The `[profile]` tables of a generated crate. A generated crate is a
/// workspace of its own, so the application's profiles would not reach the
/// programs a target builds: they are carried over from its workspace
/// manifest. Release builds start from `RELEASE_PROFILE`, take the
/// application's own release settings over it, and keep `fixed`, the lines
/// the target cannot do without (unwinding across the FFI, say); the other
/// profiles, such as optimized libraries in debug builds, are the
/// application's as they are.
pub fn profiles(context: &crate::BuildContext, fixed: &str) -> String {
    let table = |text: &str| text.parse::<toml::Table>().unwrap_or_default();
    let mut release = table(RELEASE_PROFILE);
    let mut tables = toml::Table::new();
    let own = std::fs::read_to_string(context.workspace_dir.join("Cargo.toml"))
        .ok()
        .and_then(|text| text.parse::<toml::Table>().ok())
        .and_then(|manifest| manifest.get("profile")?.as_table().cloned())
        .unwrap_or_default();
    for (name, value) in own {
        match (name.as_str(), value) {
            ("release", toml::Value::Table(settings)) => release.extend(settings),
            (_, value) => {
                tables.insert(name, value);
            }
        }
    }
    release.extend(table(fixed));
    tables.insert("release".into(), toml::Value::Table(release));
    let mut root = toml::Table::new();
    root.insert("profile".into(), toml::Value::Table(tables));
    toml::to_string(&root).unwrap_or_default()
}

/// The application's command line and MCP server for the developer's
/// computer: they reach the app on the device through `adb forward`.
pub fn tools_cargo_toml(context: &crate::BuildContext, app: &Application) -> String {
    format!(
        r#"# Generated by Bern. Do not edit.
[package]
name = "bern-tools-{id}"
version = "{version}"
edition = "2024"
publish = false

[[bin]]
name = "{cli}"
path = "src/cli.rs"

[[bin]]
name = "{cli}-mcp"
path = "src/mcp.rs"

[dependencies]
bern-adapter-cli = {{ {cli_adapter} }}
bern-adapter-mcp = {{ {mcp_adapter} }}
{protocol} = {{ path = "{protocol_path}" }}
{modules}
[workspace]

{profiles}"#,
        id = app.app.id.replace('_', "-"),
        version = app.app.version,
        cli = app.app.id.replace('_', "-"),
        cli_adapter = context.framework.dependency("bern-adapter-cli"),
        mcp_adapter = context.framework.dependency("bern-adapter-mcp"),
        protocol = context.protocol_package,
        protocol_path = path(&context.protocol_path),
        modules = module_dependencies(context, false),
        profiles = profiles(context, ""),
    )
}

/// Name stem of the generated programs, for example `showcase`.
pub fn program(app: &Application) -> String {
    app.app.id.replace('_', "-")
}

/// `main.rs` of the application CLI, derived from the command catalog.
pub fn cli_rs(context: &crate::BuildContext, app: &Application) -> String {
    format!(
        r#"//! Generated by Bern. Application CLI derived from the command catalog.

fn main() {{
    std::process::exit(bern_adapter_cli::run(
        {descriptor},
        {bundle:?},
        {name:?},
    ));
}}
"#,
        descriptor = descriptor_expr(context),
        bundle = app.app.bundle_id,
        name = program(app),
    )
}

/// `main.rs` of the MCP server, derived from the command catalog.
pub fn mcp_rs(context: &crate::BuildContext, app: &Application) -> String {
    format!(
        r#"//! Generated by Bern. MCP server derived from the command catalog.

fn main() {{
    std::process::exit(bern_adapter_mcp::run(
        {descriptor},
        {bundle:?},
        {name:?},
        {version:?},
    ));
}}
"#,
        descriptor = descriptor_expr(context),
        bundle = app.app.bundle_id,
        name = program(app),
        version = app.app.version,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BuildContext, Framework, ModuleBuild, TargetConfig};

    #[test]
    fn tools_manifest_normalizes_windows_dependency_paths() {
        let app = crate::texts::tests::app(Vec::new());
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        for (protocol_path, expected) in [
            (
                r"D:\Devs\tmp\hello\hello-protocol",
                "D:/Devs/tmp/hello/hello-protocol",
            ),
            // These backslashes would parse as escapes rather than fail.
            (r"C:\tmp\new\test", "C:/tmp/new/test"),
            (
                r"D:\My Projects\示例\hello-protocol",
                "D:/My Projects/示例/hello-protocol",
            ),
            (
                r"\\server\share\hello\hello-protocol",
                "//server/share/hello/hello-protocol",
            ),
            (
                r"\\?\D:\Devs\hello\hello-protocol",
                "//?/D:/Devs/hello/hello-protocol",
            ),
            ("/home/dev/hello-protocol", "/home/dev/hello-protocol"),
        ] {
            let context = BuildContext {
                project_dir: root.clone(),
                workspace_dir: root.clone(),
                output_dir: root.join("target/bern-test/tools-manifest"),
                target: TargetConfig::default(),
                protocol_package: "hello-protocol".into(),
                protocol_path: protocol_path.into(),
                protocol_module: "hello_protocol::hello".into(),
                engine_package: "hello-core".into(),
                engine_path: root.join("hello-core"),
                engine_type: "hello_core::Engine".into(),
                modules: vec![ModuleBuild {
                    id: "extra".into(),
                    protocol_package: "extra-protocol".into(),
                    protocol_path: r"D:\My Projects\extra\extra-protocol".into(),
                    protocol_module: "extra_protocol::extra".into(),
                    engine_package: "extra-core".into(),
                    engine_path: r"D:\My Projects\extra\extra-core".into(),
                    engine_type: "extra_core::Engine".into(),
                    optional: false,
                    enabled: true,
                    depends: Vec::new(),
                }],
                framework: Framework::Path(r"C:\Dev Kits\Bern\crates".into()),
                release: false,
            };
            let source = tools_cargo_toml(&context, &app);
            let manifest = source.parse::<toml::Table>().unwrap_or_else(|error| {
                panic!("invalid tools manifest for {protocol_path:?}: {error}\n{source}")
            });
            let dependencies = manifest["dependencies"].as_table().unwrap();
            for (package, expected_path) in [
                ("hello-protocol", expected),
                ("extra-protocol", "D:/My Projects/extra/extra-protocol"),
                ("bern", "C:/Dev Kits/Bern/crates/bern"),
                (
                    "bern-adapter-cli",
                    "C:/Dev Kits/Bern/crates/bern-adapter-cli",
                ),
                (
                    "bern-adapter-mcp",
                    "C:/Dev Kits/Bern/crates/bern-adapter-mcp",
                ),
            ] {
                let actual = dependencies[package]["path"].as_str().unwrap();
                assert_eq!(actual, expected_path, "{protocol_path:?}: {package}");
                assert!(!actual.contains('\\'), "{protocol_path:?}: {package}");
            }
            assert!(!dependencies.contains_key("hello-core"));
            assert!(!dependencies.contains_key("extra-core"));
        }
    }
}
