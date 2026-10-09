//! Compile-time validation for Bern's restricted UI declarations.
//!
//! The first version supports `Column`, `Row`, `Text`, `Button`, `TextInput`
//! and `Checkbox`. Properties contain literals or paths, never Rust expressions.
//! Commands and editable fields must be paths to typed values. Rust checks their
//! types; `bern_core::App::validate` checks registration and field scope.
//!
//! ```
//! use bern_core::Node;
//! use bern_macros::component;
//!
//! #[component]
//! fn greeting() -> Node {
//!     ui! {
//!         Column(id: "root", gap: 16, padding: 24) {
//!             Text(id: "greeting", content: "你好，Bern")
//!         }
//!     }
//! }
//! let _: Node = greeting();
//! ```
//!
//! Buttons cannot omit their command:
//!
//! ```compile_fail
//! use bern_macros::ui;
//! let _ = ui! { Button(id: "save", label: "保存") };
//! ```
//!
//! Calls cannot hide side effects inside a property:
//!
//! ```compile_fail
//! use bern_macros::ui;
//! fn read_and_mutate() -> &'static str { "changed" }
//! let _ = ui! { Text(id: "status", content: read_and_mutate()) };
//! ```
//!
//! Arbitrary callbacks are not an alternative to commands:
//!
//! ```compile_fail
//! use bern_macros::ui;
//! let _ = ui! { Button(id: "save", label: "保存", command: || {}) };
//! ```
//!
//! Components cannot execute arbitrary Rust statements before rendering:
//!
//! ```compile_fail
//! use bern_core::Node;
//! use bern_macros::component;
//! #[component]
//! fn bypass() -> Node {
//!     println!("side effect");
//!     ui! { Text(id: "status", content: "done") }
//! }
//! ```
//!
//! The generated constructor also checks that a field has the correct type:
//!
//! ```compile_fail
//! use bern_core::Field;
//! use bern_macros::ui;
//! const COUNT: Field<i32> = Field::session("count");
//! let _ = ui! { TextInput(id: "name", label: "名字", value: COUNT) };
//! ```

use std::collections::BTreeMap;

use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{ToTokens, quote};
use syn::parse::{Parse, ParseStream};
use syn::spanned::Spanned;
use syn::{
    Error, Expr, Ident, ItemFn, Lit, LitInt, LitStr, Macro, Path, PathArguments, Result,
    ReturnType, Stmt, Token, Type, braced, parenthesized, parse_macro_input, parse_quote,
};

/// Build one UI tree from a restricted declaration.
#[proc_macro]
pub fn ui(input: TokenStream) -> TokenStream {
    let node = parse_macro_input!(input as UiNode);
    node.expand().into()
}

/// Validate a synchronous, parameterless component containing exactly one `ui!`.
///
/// The return type must be `Node` or `bern_core::Node`. This macro parses the UI
/// itself, so the `ui` macro does not need to be imported separately.
#[proc_macro_attribute]
pub fn component(attribute: TokenStream, item: TokenStream) -> TokenStream {
    if !attribute.is_empty() {
        return Error::new(Span::call_site(), "#[component] does not accept arguments")
            .into_compile_error()
            .into();
    }
    let function = parse_macro_input!(item as ItemFn);
    expand_component(function)
        .unwrap_or_else(Error::into_compile_error)
        .into()
}

enum Value {
    Literal(Lit),
    Path(Path),
}

impl Value {
    fn span(&self) -> Span {
        match self {
            Self::Literal(literal) => literal.span(),
            Self::Path(path) => path.span(),
        }
    }
}

impl Parse for Value {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        if input.fork().parse::<Lit>().is_ok() {
            return Ok(Self::Literal(input.parse()?));
        }
        input
            .call(Path::parse_mod_style)
            .map(Self::Path)
            .map_err(|_| input.error("UI properties accept only literals or Rust paths; expressions and callbacks are not allowed"))
    }
}

impl ToTokens for Value {
    fn to_tokens(&self, tokens: &mut TokenStream2) {
        match self {
            Self::Literal(literal) => literal.to_tokens(tokens),
            Self::Path(path) => path.to_tokens(tokens),
        }
    }
}

struct Properties {
    values: BTreeMap<String, Value>,
    kind: Ident,
}

impl Properties {
    fn required(&mut self, name: &str) -> Result<Value> {
        self.values
            .remove(name)
            .ok_or_else(|| Error::new(self.kind.span(), format!("{} requires `{name}`", self.kind)))
    }

    fn id(&mut self) -> Result<LitStr> {
        match self.required("id")? {
            Value::Literal(Lit::Str(value)) if !value.value().is_empty() => Ok(value),
            value => Err(Error::new(
                value.span(),
                "`id` must be a non-empty string literal",
            )),
        }
    }

    fn text(&mut self, name: &str) -> Result<Value> {
        let value = self.required(name)?;
        match value {
            Value::Literal(Lit::Str(_)) | Value::Path(_) => Ok(value),
            _ => Err(Error::new(
                value.span(),
                format!("`{name}` must be a string literal or Rust path"),
            )),
        }
    }

    fn path(&mut self, name: &str) -> Result<Path> {
        match self.required(name)? {
            Value::Path(path) => Ok(path),
            value => Err(Error::new(
                value.span(),
                format!("`{name}` must be a Rust path to a typed command or field"),
            )),
        }
    }

    fn spacing(&mut self, name: &str, default: u16) -> Result<LitInt> {
        let Some(value) = self.values.remove(name) else {
            return Ok(LitInt::new(&default.to_string(), self.kind.span()));
        };
        if let Value::Literal(Lit::Int(integer)) = &value
            && let Ok(number @ 0..=128) = integer.base10_parse::<u16>()
            && (integer.suffix().is_empty() || integer.suffix() == "u16")
        {
            return Ok(LitInt::new(&number.to_string(), integer.span()));
        }
        Err(Error::new(
            value.span(),
            format!("`{name}` must be an integer literal in 0..=128 (unsuffixed or u16)"),
        ))
    }
}

struct UiNode {
    id: LitStr,
    body: UiBody,
}

enum UiBody {
    Container {
        row: bool,
        gap: LitInt,
        padding: LitInt,
        children: Vec<UiNode>,
    },
    Text {
        content: Value,
    },
    Button {
        label: Value,
        command: Path,
    },
    TextInput {
        label: Value,
        value: Path,
    },
    Checkbox {
        label: Value,
        value: Path,
    },
}

impl Parse for UiNode {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let kind: Ident = input.parse()?;
        let name = kind.to_string();
        let allowed: &[&str] = match name.as_str() {
            "Column" | "Row" => &["id", "gap", "padding"],
            "Text" => &["id", "content"],
            "Button" => &["id", "label", "command"],
            "TextInput" | "Checkbox" => &["id", "label", "value"],
            _ => {
                return Err(Error::new(
                    kind.span(),
                    format!("unknown Bern control `{kind}`"),
                ));
            }
        };
        let arguments;
        parenthesized!(arguments in input);
        let mut properties = Properties {
            values: BTreeMap::new(),
            kind,
        };
        while !arguments.is_empty() {
            let key: Ident = arguments.parse()?;
            let key_name = key.to_string();
            if !allowed.contains(&key_name.as_str()) {
                return Err(Error::new(
                    key.span(),
                    format!("unknown property `{key}` on {name}"),
                ));
            }
            if properties.values.contains_key(&key_name) {
                return Err(Error::new(
                    key.span(),
                    format!("duplicate property `{key}`"),
                ));
            }
            arguments.parse::<Token![:]>()?;
            properties.values.insert(key_name, arguments.parse()?);
            if !arguments.is_empty() {
                if !arguments.peek(Token![,]) {
                    return Err(arguments.error("UI properties accept only literals or Rust paths; calls, operators and other expressions are not allowed"));
                }
                arguments.parse::<Token![,]>()?;
            }
        }
        let id = properties.id()?;
        let body = match name.as_str() {
            "Column" | "Row" => {
                let gap = properties.spacing("gap", 12)?;
                let padding = properties.spacing("padding", 0)?;
                if !input.peek(syn::token::Brace) {
                    return Err(
                        input.error(format!("{name} requires a children block `{{ ... }}`"))
                    );
                }
                let content;
                braced!(content in input);
                let mut children = Vec::new();
                while !content.is_empty() {
                    children.push(content.parse()?);
                }
                UiBody::Container {
                    row: name == "Row",
                    gap,
                    padding,
                    children,
                }
            }
            "Text" => UiBody::Text {
                content: properties.text("content")?,
            },
            "Button" => UiBody::Button {
                label: properties.text("label")?,
                command: properties.path("command")?,
            },
            "TextInput" => UiBody::TextInput {
                label: properties.text("label")?,
                value: properties.path("value")?,
            },
            "Checkbox" => UiBody::Checkbox {
                label: properties.text("label")?,
                value: properties.path("value")?,
            },
            _ => unreachable!("control kind was validated before properties"),
        };
        if !matches!(body, UiBody::Container { .. }) && input.peek(syn::token::Brace) {
            return Err(input.error(format!("{name} is a leaf control and cannot have children")));
        }
        Ok(Self { id, body })
    }
}

impl UiNode {
    fn expand(&self) -> TokenStream2 {
        let id = &self.id;
        match &self.body {
            UiBody::Container {
                row,
                gap,
                padding,
                children,
            } => {
                let constructor = Ident::new(if *row { "row" } else { "column" }, id.span());
                let children = children.iter().map(Self::expand);
                quote! { ::bern_core::Node::#constructor(#id, #gap, #padding, ::std::vec![#(#children),*]) }
            }
            UiBody::Text { content } => quote! { ::bern_core::Node::text(#id, #content) },
            UiBody::Button { label, command } => {
                quote! { ::bern_core::Node::button(#id, #label, #command) }
            }
            UiBody::TextInput { label, value } => {
                quote! { ::bern_core::Node::text_input(#id, #label, #value) }
            }
            UiBody::Checkbox { label, value } => {
                quote! { ::bern_core::Node::checkbox(#id, #label, #value) }
            }
        }
    }
}

fn is_named_path(path: &Path, names: &[&str]) -> bool {
    path.segments.len() == names.len()
        && path.segments.iter().zip(names).all(|(segment, name)| {
            segment.ident == name && matches!(segment.arguments, PathArguments::None)
        })
}

fn component_ui(function: &ItemFn) -> Result<&Macro> {
    let signature = &function.sig;
    if signature.asyncness.is_some()
        || signature.constness.is_some()
        || signature.unsafety.is_some()
        || signature.abi.is_some()
        || signature.variadic.is_some()
        || !signature.inputs.is_empty()
        || !signature.generics.params.is_empty()
        || signature.generics.where_clause.is_some()
    {
        return Err(Error::new_spanned(
            signature,
            "#[component] requires a safe synchronous function without parameters, generics, const, extern or a where clause",
        ));
    }
    let valid_return = match &signature.output {
        ReturnType::Type(_, ty) => match ty.as_ref() {
            Type::Path(ty) if ty.qself.is_none() => {
                is_named_path(&ty.path, &["Node"])
                    || is_named_path(&ty.path, &["bern_core", "Node"])
            }
            _ => false,
        },
        _ => false,
    };
    if !valid_return {
        return Err(Error::new_spanned(
            &signature.output,
            "#[component] must return Node or bern_core::Node",
        ));
    }
    let mac = match function.block.stmts.as_slice() {
        [Stmt::Expr(Expr::Macro(expression), None)] if expression.attrs.is_empty() => {
            &expression.mac
        }
        [Stmt::Macro(statement)]
            if statement.semi_token.is_none() && statement.attrs.is_empty() =>
        {
            &statement.mac
        }
        _ => {
            return Err(Error::new_spanned(
                &function.block,
                "#[component] body must contain exactly one ui! declaration, without Rust statements or a trailing semicolon",
            ));
        }
    };
    if !is_named_path(&mac.path, &["ui"]) && !is_named_path(&mac.path, &["bern_macros", "ui"]) {
        return Err(Error::new_spanned(
            &mac.path,
            "#[component] body must be ui!",
        ));
    }
    Ok(mac)
}

fn expand_component(mut function: ItemFn) -> Result<TokenStream2> {
    let mac = component_ui(&function)?;
    let node = syn::parse2::<UiNode>(mac.tokens.clone())?;
    let body = node.expand();
    *function.block = parse_quote!({ #body });
    Ok(quote!(#function))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reject_ui(source: &str, expected: &str) {
        let error = syn::parse_str::<UiNode>(source)
            .err()
            .expect("must reject UI");
        assert!(error.to_string().contains(expected), "{source}: {error}");
    }

    #[test]
    fn parses_all_controls_and_typed_paths() {
        let node = syn::parse_str::<UiNode>(
            r#"
            Column(id: "root", gap: 16, padding: 24) {
                Text(id: "title", content: "标题")
                Text(id: "count", content: fields::COUNT)
                Row(id: "actions", gap: 12) {
                    Button(id: "inc", label: "增加", command: commands::INCREMENT)
                }
                TextInput(id: "name", label: "名字", value: fields::NAME)
                Checkbox(id: "enabled", label: "启用", value: fields::ENABLED)
            }
        "#,
        )
        .unwrap();
        let UiBody::Container {
            children,
            gap,
            padding,
            ..
        } = node.body
        else {
            panic!("container")
        };
        assert_eq!(children.len(), 5);
        assert_eq!(gap.base10_parse::<u16>().unwrap(), 16);
        assert_eq!(padding.base10_parse::<u16>().unwrap(), 24);
    }

    #[test]
    fn container_defaults_and_spacing_limits() {
        let node = syn::parse_str::<UiNode>(r#"Row(id: "root") {}"#).unwrap();
        let UiBody::Container {
            gap,
            padding,
            children,
            ..
        } = node.body
        else {
            panic!("container")
        };
        assert!(children.is_empty());
        assert_eq!(gap.base10_parse::<u16>().unwrap(), 12);
        assert_eq!(padding.base10_parse::<u16>().unwrap(), 0);
        assert!(
            syn::parse_str::<UiNode>(r#"Column(id: "root", gap: 0, padding: 128,) {}"#).is_ok()
        );
        reject_ui(r#"Column(id: "root", gap: 129) {}"#, "0..=128");
        reject_ui(r#"Column(id: "root", padding: 65536) {}"#, "0..=128");
        reject_ui(r#"Column(id: "root", gap: 2.5) {}"#, "0..=128");
        reject_ui(r#"Column(id: "root", gap: SPACING) {}"#, "0..=128");
    }

    #[test]
    fn rejects_unknown_missing_and_duplicate_properties() {
        reject_ui(r#"Unknown(id: "x")"#, "unknown Bern control");
        reject_ui(
            r#"Text(id: "x", content: "x", color: "red")"#,
            "unknown property",
        );
        reject_ui(
            r#"Text(id: "x", id: "y", content: "x")"#,
            "duplicate property",
        );
        reject_ui(r#"Text(content: "x")"#, "requires `id`");
        reject_ui(r#"Text(id: "x")"#, "requires `content`");
        reject_ui(r#"Button(id: "x", label: "save")"#, "requires `command`");
        reject_ui(
            r#"Button(id: "x", label: "save", command: "save")"#,
            "Rust path",
        );
        reject_ui(
            r#"TextInput(id: "x", label: "name", value: "draft")"#,
            "Rust path",
        );
        reject_ui(r#"Text(id: ID, content: "x")"#, "string literal");
        reject_ui(r#"Text(id: "", content: "x")"#, "non-empty");
    }

    #[test]
    fn rejects_expressions_and_callback_escape_hatches() {
        for source in [
            r#"Text(id: "x", content: compute())"#,
            r#"Text(id: "x", content: fields::VALUE + 1)"#,
            r#"Text(id: "x", content: object.value)"#,
            r#"Text(id: "x", content: format!("text"))"#,
            r#"Text(id: "x", content: { mutate(); "x" })"#,
            r#"Text(id: "x", content: if FLAG { "a" } else { "b" })"#,
            r#"Button(id: "x", label: "save", command: || {})"#,
            r#"Button(id: "x", label: "save", command: state = value)"#,
            r#"Text(id: "x", content: fields::value::<u32>)"#,
        ] {
            assert!(
                syn::parse_str::<UiNode>(source).is_err(),
                "accepted {source}"
            );
        }
        reject_ui(
            r#"Button(id: "x", label: "save", on_click: handler)"#,
            "unknown property",
        );
    }

    #[test]
    fn enforces_tree_shape_and_one_root() {
        reject_ui(r#"Column(id: "x")"#, "requires a children block");
        reject_ui(r#"Text(id: "x", content: "x") {}"#, "cannot have children");
        reject_ui(
            r#"Text(id: "a", content: "a") Text(id: "b", content: "b")"#,
            "unexpected token",
        );
        assert!(
            syn::parse_str::<UiNode>(
                r#"Column(id: "x") { if READY { Text(id: "x", content: "x") } }"#
            )
            .is_err()
        );
    }

    #[test]
    fn accepts_only_restricted_component_signatures_and_body() {
        for source in [
            r#"fn screen() -> Node { ui! { Text(id: "x", content: "x") } }"#,
            r#"pub fn screen() -> ::bern_core::Node { bern_macros::ui!(Text(id: "x", content: "x")) }"#,
        ] {
            let function = syn::parse_str::<ItemFn>(source).unwrap();
            assert!(expand_component(function).is_ok(), "rejected {source}");
        }
        for source in [
            r#"async fn screen() -> Node { ui! { Text(id: "x", content: "x") } }"#,
            r#"const fn screen() -> Node { ui! { Text(id: "x", content: "x") } }"#,
            r#"unsafe fn screen() -> Node { ui! { Text(id: "x", content: "x") } }"#,
            r#"extern "C" fn screen() -> Node { ui! { Text(id: "x", content: "x") } }"#,
            r#"fn screen(value: u32) -> Node { ui! { Text(id: "x", content: "x") } }"#,
            r#"fn screen<T>() -> Node { ui! { Text(id: "x", content: "x") } }"#,
            r#"fn screen() -> Node where Node: Clone { ui! { Text(id: "x", content: "x") } }"#,
            r#"fn screen() -> String { ui! { Text(id: "x", content: "x") } }"#,
            r#"fn screen() -> Node { let x = mutate(); ui! { Text(id: "x", content: "x") } }"#,
            r#"fn screen() -> Node { arbitrary_rust() }"#,
            r#"fn screen() -> Node { other_macro!() }"#,
            r#"fn screen() -> Node { ui! { Text(id: "x", content: "x") }; }"#,
            r#"fn screen() -> Node { ui! { Button(id: "x", label: "save") } }"#,
        ] {
            let function = syn::parse_str::<ItemFn>(source).unwrap();
            assert!(expand_component(function).is_err(), "accepted {source}");
        }
    }
}
