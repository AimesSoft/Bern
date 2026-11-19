//! Shared representation and helpers for `NativeView` elements across
//! compiler checks and every target backend.

use crate::{Action, Application, Element, Expr, FieldRef, Node, Prop, Text};
use bern_protocol::Type;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The backend property names accepted on `NativeView`, matching
/// `TargetDescriptor::ui`.
pub const NATIVE_BACKENDS: &[&str] = &["swiftui", "winui3", "compose", "gtk", "gpui", "web"];

/// The CLI-discoverable functional entry elements a `NativeView` declares.
pub const NATIVE_ENTRIES: &[&str] = &["NativeAction", "NativeNumber", "NativeBool", "NativeInput"];

/// All structural children belonging to a `NativeView`.
pub const NATIVE_CHILDREN: &[&str] = &[
    "NativeValue",
    "NativeText",
    "NativeDraft",
    "NativeAction",
    "NativeNumber",
    "NativeBool",
    "NativeInput",
    "Fallback",
];

/// Expected file extensions for each native UI backend.
pub fn backend_extensions(backend: &str) -> &'static [&'static str] {
    match backend {
        "swiftui" => &[".swift"],
        "winui3" => &[".cpp", ".inl"],
        "compose" => &[".kt"],
        "gtk" | "gpui" => &[".rs"],
        "web" => &[".js"],
        _ => &[],
    }
}

/// Supported scalar types for `NativeValue(value:)` bindings.
pub fn is_native_scalar_type(ty: &Type) -> bool {
    match ty {
        Type::Bool | Type::Int | Type::Float | Type::Text => true,
        Type::Optional { inner } => matches!(**inner, Type::Int | Type::Float | Type::Text),
        _ => false,
    }
}

/// Supported session field types for `NativeDraft(value:)` bindings.
pub fn is_native_draft_type(ty: &Type) -> bool {
    match ty {
        Type::Bool | Type::Int | Type::Float | Type::Text => true,
        Type::Optional { inner } => matches!(**inner, Type::Float),
        _ => false,
    }
}

/// A co-located native UI implementation file referenced by a `NativeView`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeFileDef {
    /// Normalized `/`-separated path relative to the project directory
    /// (or library-prefixed when inside a component library).
    pub path: String,
    /// Backend id from [`NATIVE_BACKENDS`].
    pub backend: String,
    /// Source contents loaded at compile time. Native desktop targets prefer
    /// referencing the real file under `project_dir.join(&path)` directly when
    /// it exists on disk, falling back to `source` for generated/in-memory
    /// builds and targets that rewrite or copy files.
    pub source: String,
}

/// A read-only data binding declared via `NativeValue(name:, value:)`.
#[derive(Clone, Copy, Debug)]
pub struct NativeValueInfo<'a> {
    pub name: &'a str,
    pub expr: &'a Expr,
}

/// A localized display text declared via `NativeText(name:, content:)`.
#[derive(Clone, Copy, Debug)]
pub struct NativeTextInfo<'a> {
    pub name: &'a str,
    pub text: &'a Text,
}

/// A writable session draft field declared via `NativeDraft(name:, value:)`.
#[derive(Clone, Copy, Debug)]
pub struct NativeDraftInfo<'a> {
    pub name: &'a str,
    pub field: &'a FieldRef,
}

/// One CLI-discoverable functional entry declared inside a `NativeView`.
#[derive(Clone, Copy, Debug)]
pub enum NativeEntryKind<'a> {
    Action,
    Number {
        current: &'a Expr,
        min: f64,
        max: f64,
        step: Option<f64>,
    },
    Bool {
        checked: &'a Expr,
    },
    Input {
        current: &'a Expr,
    },
}

/// Parsed view of one `NativeAction` / `NativeNumber` / `NativeBool` / `NativeInput`.
#[derive(Clone, Copy, Debug)]
pub struct NativeEntryInfo<'a> {
    pub id: &'a str,
    pub label: &'a Text,
    pub enabled: Option<&'a Expr>,
    pub action: &'a Action,
    pub kind: NativeEntryKind<'a>,
    pub element: &'a Element,
}

impl<'a> NativeEntryInfo<'a> {
    /// The Bern CLI control role for this entry.
    pub fn role(&self) -> &'static str {
        match self.kind {
            NativeEntryKind::Action => "button",
            NativeEntryKind::Number { .. } => "slider",
            NativeEntryKind::Bool { .. } => "switch",
            NativeEntryKind::Input { .. } => "text_input",
        }
    }

    /// Control target suffix relative to the enclosing component (`<view>.<entry>`).
    pub fn local_target(&self, view_id: &str) -> String {
        format!("{view_id}.{}", self.id)
    }
}

/// Structured view of a `NativeView` element used identically by every backend.
#[derive(Clone, Debug)]
pub struct NativeViewInfo<'a> {
    pub id: &'a str,
    pub label: &'a Text,
    pub files: BTreeMap<&'a str, &'a str>,
    pub values: Vec<NativeValueInfo<'a>>,
    pub texts: Vec<NativeTextInfo<'a>>,
    pub drafts: Vec<NativeDraftInfo<'a>>,
    pub entries: Vec<NativeEntryInfo<'a>>,
    pub fallback: Option<&'a [Node]>,
    pub element: &'a Element,
}

impl<'a> NativeViewInfo<'a> {
    pub fn file_for(&self, backend: &str) -> Option<&'a str> {
        self.files.get(backend).copied()
    }
}

/// One `NativeView` discovered in an [`Application`], paired with its owning
/// component/window scope name and unique generated symbol base.
#[derive(Clone, Debug)]
pub struct CollectedNativeView<'a> {
    pub scope: String,
    pub symbol: String,
    pub info: NativeViewInfo<'a>,
}

/// Parse a `NativeView` element into its structured contract.
pub fn inspect_native_view(element: &Element) -> Option<NativeViewInfo<'_>> {
    if element.kind != "NativeView" {
        return None;
    }
    let id = element.id.as_deref()?;
    let label = element.text("label")?;
    let mut files = BTreeMap::new();
    for &backend in NATIVE_BACKENDS {
        if let Some(Prop::NativeFile { path }) = element.prop(backend) {
            files.insert(backend, path.as_str());
        }
    }
    let mut values = Vec::new();
    let mut texts = Vec::new();
    let mut drafts = Vec::new();
    let mut entries = Vec::new();
    let mut fallback = None;

    for child in &element.children {
        let Node::Element(part) = child else {
            continue;
        };
        match part.kind.as_str() {
            "NativeValue" => {
                if let (Some(Prop::Id { value: name }), Some(expr)) =
                    (part.prop("name"), part.expr("value"))
                {
                    values.push(NativeValueInfo {
                        name: name.as_str(),
                        expr,
                    });
                }
            }
            "NativeText" => {
                if let (Some(Prop::Id { value: name }), Some(text)) =
                    (part.prop("name"), part.text("content"))
                {
                    texts.push(NativeTextInfo {
                        name: name.as_str(),
                        text,
                    });
                }
            }
            "NativeDraft" => {
                if let (Some(Prop::Id { value: name }), Some(field)) =
                    (part.prop("name"), part.field("value"))
                {
                    drafts.push(NativeDraftInfo {
                        name: name.as_str(),
                        field,
                    });
                }
            }
            "NativeAction" => {
                if let (Some(eid), Some(elabel), Some(action)) =
                    (part.id.as_deref(), part.text("label"), part.action.as_ref())
                {
                    entries.push(NativeEntryInfo {
                        id: eid,
                        label: elabel,
                        enabled: part.expr("enabled"),
                        action,
                        kind: NativeEntryKind::Action,
                        element: part,
                    });
                }
            }
            "NativeNumber" => {
                if let (
                    Some(eid),
                    Some(elabel),
                    Some(current),
                    Some(min),
                    Some(max),
                    Some(action),
                ) = (
                    part.id.as_deref(),
                    part.text("label"),
                    part.expr("current"),
                    part.number("min"),
                    part.number("max"),
                    part.action.as_ref(),
                ) {
                    entries.push(NativeEntryInfo {
                        id: eid,
                        label: elabel,
                        enabled: part.expr("enabled"),
                        action,
                        kind: NativeEntryKind::Number {
                            current,
                            min,
                            max,
                            step: part.number("step"),
                        },
                        element: part,
                    });
                }
            }
            "NativeBool" => {
                if let (Some(eid), Some(elabel), Some(checked), Some(action)) = (
                    part.id.as_deref(),
                    part.text("label"),
                    part.expr("checked"),
                    part.action.as_ref(),
                ) {
                    entries.push(NativeEntryInfo {
                        id: eid,
                        label: elabel,
                        enabled: part.expr("enabled"),
                        action,
                        kind: NativeEntryKind::Bool { checked },
                        element: part,
                    });
                }
            }
            "NativeInput" => {
                if let (Some(eid), Some(elabel), Some(current), Some(action)) = (
                    part.id.as_deref(),
                    part.text("label"),
                    part.expr("current"),
                    part.action.as_ref(),
                ) {
                    entries.push(NativeEntryInfo {
                        id: eid,
                        label: elabel,
                        enabled: part.expr("enabled"),
                        action,
                        kind: NativeEntryKind::Input { current },
                        element: part,
                    });
                }
            }
            "Fallback" => {
                fallback = Some(part.children.as_slice());
            }
            _ => {}
        }
    }

    Some(NativeViewInfo {
        id,
        label,
        files,
        values,
        texts,
        drafts,
        entries,
        fallback,
        element,
    })
}

/// Collect every `NativeView` in an application, in deterministic order.
pub fn collect_native_views(app: &Application) -> Vec<CollectedNativeView<'_>> {
    let mut found = Vec::new();
    for component in &app.components {
        collect_in_node(&component.name, &component.body, &mut found);
    }
    for window in &app.windows.windows {
        collect_in_node(&window.id, &window.content, &mut found);
    }
    found
}

fn collect_in_node<'a>(scope: &str, node: &'a Node, out: &mut Vec<CollectedNativeView<'a>>) {
    node.walk(&mut |n| {
        if let Node::Element(element) = n
            && let Some(info) = inspect_native_view(element)
        {
            out.push(CollectedNativeView {
                scope: scope.to_string(),
                symbol: view_symbol(scope, info.id),
                info,
            });
        }
    });
}

/// Unique snake_case symbol for a `NativeView` in a component/window scope.
pub fn view_symbol(scope: &str, id: &str) -> String {
    format!("{}_{}", snake_ident(scope), snake_ident(id))
}

/// Convert `foo-bar_baz` into `foo_bar_baz`.
pub fn snake_ident(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut prev_sep = false;
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() {
            if ch.is_ascii_uppercase() && !out.is_empty() && !prev_sep {
                out.push('_');
            }
            out.push(ch.to_ascii_lowercase());
            prev_sep = false;
        } else if !out.is_empty() && !prev_sep {
            out.push('_');
            prev_sep = true;
        }
    }
    while out.ends_with('_') {
        out.pop();
    }
    if out.is_empty() {
        "view".into()
    } else if out.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        format!("n_{out}")
    } else {
        out
    }
}

/// Convert `foo_bar` or `foo-bar` into `FooBar`.
pub fn pascal_ident(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut upper = true;
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() {
            if upper {
                out.push(ch.to_ascii_uppercase());
                upper = false;
            } else {
                out.push(ch);
            }
        } else {
            upper = true;
        }
    }
    if out.is_empty() {
        "View".into()
    } else if out.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        format!("N{out}")
    } else {
        out
    }
}

/// Convert `foo_bar` or `foo-bar` into `fooBar`.
pub fn camel_ident(raw: &str) -> String {
    let pascal = pascal_ident(raw);
    let mut chars = pascal.chars();
    let Some(first) = chars.next() else {
        return "view".into();
    };
    let mut out = String::with_capacity(pascal.len());
    out.push(first.to_ascii_lowercase());
    out.extend(chars);
    out
}

/// Look up the embedded source of a native file by its normalized IR path.
pub fn find_native_source<'a>(app: &'a Application, path: &str) -> Option<&'a NativeFileDef> {
    app.native_files.iter().find(|file| file.path == path)
}
