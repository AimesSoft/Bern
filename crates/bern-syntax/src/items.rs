//! Item-level declarations: protocol modules, value types, session state
//! and components. The macros and the collector parse them identically.

use crate::ast::*;
use crate::expr::{FullExpr, TrBody};
use proc_macro2::{Span, TokenStream};
use syn::spanned::Spanned;
use syn::{Attribute, Ident, Item, ItemFn, ItemMod, Meta};

/// Arguments of `#[bern::protocol(namespace = "…", version = N)]`.
#[derive(Clone, Debug)]
pub struct ProtocolArgs {
    pub namespace: String,
    pub version: u32,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct FieldItem {
    pub name: Ident,
    pub ty: TypeRef,
    pub syn_ty: syn::Type,
    pub doc: String,
    pub initial: Option<Expr>,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct VariantItem {
    pub name: Ident,
    pub fields: Vec<FieldItem>,
    pub label: Option<Tr>,
    pub doc: String,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum ValueKind {
    Enum(Vec<VariantItem>),
    Record(Vec<FieldItem>),
}

/// An enum or record that may cross a Bern boundary.
#[derive(Clone, Debug)]
pub struct ValueItem {
    pub name: Ident,
    pub kind: ValueKind,
    pub doc: String,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct StateItem {
    pub name: Ident,
    pub fields: Vec<FieldItem>,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct CommandParam {
    pub name: Ident,
    pub ty: TypeRef,
    pub syn_ty: syn::Type,
}

#[derive(Clone, Debug)]
pub struct CommandItem {
    pub name: Ident,
    pub params: Vec<CommandParam>,
    pub result: TypeRef,
    pub result_ty: syn::Type,
    pub doc: String,
    pub errors: Vec<String>,
    pub idempotent: bool,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct CommandsItem {
    pub name: Ident,
    pub methods: Vec<CommandItem>,
    pub span: Span,
}

/// A `#[bern::protocol]` module: value types, one state struct and one
/// command trait.
#[derive(Clone, Debug)]
pub struct ProtocolItem {
    pub args: ProtocolArgs,
    pub module: ItemMod,
    pub doc: String,
    pub values: Vec<ValueItem>,
    pub state: StateItem,
    pub commands: CommandsItem,
    pub uses: Vec<syn::ItemUse>,
}

/// A `#[bern::session]` struct: typed drafts owned by a UI session.
#[derive(Clone, Debug)]
pub struct SessionItem {
    pub name: Ident,
    pub fields: Vec<FieldItem>,
    pub doc: String,
    pub span: Span,
}

/// One application player in a `#[bern::players]` struct.
#[derive(Clone, Debug)]
pub struct PlayerField {
    pub name: Ident,
    pub doc: String,
    pub span: Span,
}

/// A `#[bern::players]` struct: media players owned by the application,
/// shared by every window and addressed by their field names.
#[derive(Clone, Debug)]
pub struct PlayersItem {
    pub name: Ident,
    pub players: Vec<PlayerField>,
    pub doc: String,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParamKind {
    /// Passed by the caller.
    Prop,
    /// Owned session state, scoped to the hosting window.
    Session,
}

#[derive(Clone, Debug)]
pub struct Param {
    pub name: Ident,
    pub ty: TypeRef,
    pub syn_ty: syn::Type,
    pub kind: ParamKind,
    pub span: Span,
}

/// A `#[component]` function whose body is a single `ui!` declaration.
#[derive(Clone, Debug)]
pub struct ComponentItem {
    pub func: ItemFn,
    pub name: Ident,
    pub params: Vec<Param>,
    pub body: UiBody,
    pub doc: String,
    pub span: Span,
}

/// Every Bern declaration found in one source file, with module paths
/// relative to the file's own module.
#[derive(Clone, Debug, Default)]
pub struct SourceItems {
    pub protocols: Vec<(Vec<String>, ProtocolItem)>,
    pub values: Vec<(Vec<String>, ValueItem)>,
    pub sessions: Vec<(Vec<String>, SessionItem)>,
    pub players: Vec<(Vec<String>, PlayersItem)>,
    pub components: Vec<(Vec<String>, ComponentItem)>,
}

fn error(span: Span, message: impl std::fmt::Display) -> syn::Error {
    syn::Error::new(span, message)
}

/// True for `#[name]` and `#[bern::name]`.
pub fn is_bern_attr(attr: &Attribute, name: &str) -> bool {
    let path = attr.path();
    let segments: Vec<_> = path.segments.iter().map(|s| s.ident.to_string()).collect();
    match segments.as_slice() {
        [only] => only == name,
        [first, second] => first == "bern" && second == name,
        _ => false,
    }
}

fn find_attr<'a>(attrs: &'a [Attribute], name: &str) -> Option<&'a Attribute> {
    attrs.iter().find(|attr| is_bern_attr(attr, name))
}

pub fn doc_comment(attrs: &[Attribute]) -> String {
    let mut lines = Vec::new();
    for attr in attrs {
        if attr.path().is_ident("doc")
            && let Meta::NameValue(value) = &attr.meta
            && let syn::Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Str(text),
                ..
            }) = &value.value
        {
            lines.push(text.value().trim().to_string());
        }
    }
    lines.join("\n").trim().to_string()
}

fn reject_cfg(attrs: &[Attribute], what: &str) -> syn::Result<()> {
    for attr in attrs {
        if attr.path().is_ident("cfg") || attr.path().is_ident("cfg_attr") {
            return Err(error(
                attr.span(),
                format!(
                    "conditional compilation on {what} is not supported in this version; \
                     the collector cannot match `cfg` conditions reliably"
                ),
            ));
        }
    }
    Ok(())
}

/// Map a Rust type to Bern's type vocabulary.
pub fn type_ref(ty: &syn::Type) -> syn::Result<TypeRef> {
    match ty {
        syn::Type::Tuple(tuple) if tuple.elems.is_empty() => Ok(TypeRef::Unit),
        syn::Type::Paren(inner) => type_ref(&inner.elem),
        syn::Type::Path(path) if path.qself.is_none() => {
            let last = path.path.segments.last().unwrap();
            let name = last.ident.to_string();
            let single = path.path.segments.len() == 1;
            let generic = |index: usize| -> syn::Result<&syn::Type> {
                if let syn::PathArguments::AngleBracketed(args) = &last.arguments {
                    let types: Vec<_> = args
                        .args
                        .iter()
                        .filter_map(|arg| match arg {
                            syn::GenericArgument::Type(ty) => Some(ty),
                            _ => None,
                        })
                        .collect();
                    if let Some(ty) = types.get(index) {
                        return Ok(ty);
                    }
                }
                Err(error(
                    last.span(),
                    format!("`{name}` needs a type argument"),
                ))
            };
            match name.as_str() {
                "bool" if single => Ok(TypeRef::Bool),
                "i64" if single => Ok(TypeRef::Int),
                "f64" if single => Ok(TypeRef::Float),
                "String" => Ok(TypeRef::Text),
                "Option" => Ok(TypeRef::Optional(Box::new(type_ref(generic(0)?)?))),
                "Vec" => Ok(TypeRef::List(Box::new(type_ref(generic(0)?)?))),
                "i8" | "i16" | "i32" | "i128" | "isize" | "u8" | "u16" | "u32" | "u64" | "u128"
                | "usize" => Err(error(
                    last.span(),
                    format!("`{name}` cannot cross a Bern boundary; use `i64`"),
                )),
                "f32" => Err(error(
                    last.span(),
                    "`f32` cannot cross a Bern boundary; use `f64`",
                )),
                "char" | "str" => Err(error(last.span(), "use `String` for text")),
                "Box" | "Rc" | "Arc" | "HashMap" | "BTreeMap" | "HashSet" => Err(error(
                    last.span(),
                    format!(
                        "`{name}` cannot cross a Bern boundary; the first version supports \
                         bool, i64, f64, String, Option, Vec, enums and records"
                    ),
                )),
                _ => {
                    if !matches!(last.arguments, syn::PathArguments::None) {
                        return Err(error(
                            last.span(),
                            "generic types cannot cross a Bern boundary",
                        ));
                    }
                    Ok(TypeRef::Named(
                        path.path
                            .segments
                            .iter()
                            .map(|segment| segment.ident.to_string())
                            .collect(),
                    ))
                }
            }
        }
        syn::Type::Reference(_) => Err(error(
            ty.span(),
            "references cannot cross a Bern boundary; use owned values",
        )),
        _ => Err(error(
            ty.span(),
            "this type cannot cross a Bern boundary; the first version supports bool, i64, \
             f64, String, Option, Vec, enums and records",
        )),
    }
}

fn parse_initial(attrs: &[Attribute]) -> syn::Result<Option<Expr>> {
    match find_attr(attrs, "initial") {
        Some(attr) => {
            let tokens = attr.meta.require_list()?.tokens.clone();
            Ok(Some(syn::parse2::<FullExpr>(tokens)?.0))
        }
        None => Ok(None),
    }
}

fn parse_label(attrs: &[Attribute]) -> syn::Result<Option<Tr>> {
    match find_attr(attrs, "label") {
        Some(attr) => {
            let tokens = attr.meta.require_list()?.tokens.clone();
            let expr = syn::parse2::<FullExpr>(tokens.clone());
            match expr {
                Ok(FullExpr(Expr::Tr(tr))) => Ok(Some(tr)),
                _ => {
                    // Also accept the bare form `#[label("en" => "…")]`.
                    let tr = syn::parse2::<TrBody>(tokens).map_err(|_| {
                        error(attr.span(), "expected `#[label(tr! { \"en\" => \"…\" })]`")
                    })?;
                    Ok(Some(tr.0))
                }
            }
        }
        None => Ok(None),
    }
}

fn check_known_attrs(attrs: &[Attribute], allowed: &[&str], what: &str) -> syn::Result<()> {
    for attr in attrs {
        let path = attr.path();
        if path.is_ident("doc") || path.is_ident("allow") || path.is_ident("expect") {
            continue;
        }
        if allowed.iter().any(|name| is_bern_attr(attr, name)) {
            continue;
        }
        if path.is_ident("derive") {
            return Err(error(
                attr.span(),
                format!("{what} derive their traits from Bern; remove `#[derive]`"),
            ));
        }
        if path.is_ident("cfg") || path.is_ident("cfg_attr") {
            reject_cfg(std::slice::from_ref(attr), what)?;
        }
        return Err(error(
            attr.span(),
            format!("attribute is not supported on {what}"),
        ));
    }
    Ok(())
}

fn parse_named_fields(
    fields: &syn::Fields,
    owner: &Ident,
    allow_initial: bool,
    require_pub: bool,
) -> syn::Result<Vec<FieldItem>> {
    let named = match fields {
        syn::Fields::Named(named) => named,
        syn::Fields::Unit => return Ok(Vec::new()),
        syn::Fields::Unnamed(_) => {
            return Err(error(
                owner.span(),
                format!("`{owner}` must use named fields"),
            ));
        }
    };
    let mut result = Vec::new();
    for field in &named.named {
        let name = field.ident.clone().unwrap();
        let allowed: &[&str] = if allow_initial { &["initial"] } else { &[] };
        check_known_attrs(&field.attrs, allowed, "fields")?;
        if require_pub && !matches!(field.vis, syn::Visibility::Public(_)) {
            return Err(error(
                name.span(),
                format!("field `{name}` must be `pub` so every entry can read it"),
            ));
        }
        result.push(FieldItem {
            ty: type_ref(&field.ty)?,
            syn_ty: field.ty.clone(),
            doc: doc_comment(&field.attrs),
            initial: if allow_initial {
                parse_initial(&field.attrs)?
            } else {
                None
            },
            span: name.span(),
            name,
        });
    }
    Ok(result)
}

fn reject_generics(generics: &syn::Generics, name: &Ident) -> syn::Result<()> {
    if !generics.params.is_empty() || generics.where_clause.is_some() {
        return Err(error(
            name.span(),
            format!("`{name}` cannot be generic; Bern types have one schema"),
        ));
    }
    Ok(())
}

/// Parse an enum or struct marked as a Bern value type. The marker
/// attribute itself must already be removed by the caller.
pub fn parse_value(item: &Item) -> syn::Result<ValueItem> {
    match item {
        Item::Enum(item) => {
            reject_generics(&item.generics, &item.ident)?;
            check_known_attrs(&item.attrs, &["value"], "value types")?;
            let mut variants = Vec::new();
            for variant in &item.variants {
                check_known_attrs(&variant.attrs, &["label"], "enum variants")?;
                if variant.discriminant.is_some() {
                    return Err(error(
                        variant.ident.span(),
                        "explicit discriminants are not supported",
                    ));
                }
                if matches!(variant.fields, syn::Fields::Unnamed(_)) {
                    return Err(error(
                        variant.ident.span(),
                        "use named fields `Variant { field: Type }` instead of tuple variants",
                    ));
                }
                variants.push(VariantItem {
                    name: variant.ident.clone(),
                    fields: parse_named_fields(&variant.fields, &variant.ident, false, false)?,
                    label: parse_label(&variant.attrs)?,
                    doc: doc_comment(&variant.attrs),
                    span: variant.ident.span(),
                });
            }
            if variants.is_empty() {
                return Err(error(
                    item.ident.span(),
                    "an enum needs at least one variant",
                ));
            }
            Ok(ValueItem {
                name: item.ident.clone(),
                kind: ValueKind::Enum(variants),
                doc: doc_comment(&item.attrs),
                span: item.ident.span(),
            })
        }
        Item::Struct(item) => {
            reject_generics(&item.generics, &item.ident)?;
            check_known_attrs(&item.attrs, &["value"], "value types")?;
            Ok(ValueItem {
                name: item.ident.clone(),
                kind: ValueKind::Record(parse_named_fields(
                    &item.fields,
                    &item.ident,
                    false,
                    true,
                )?),
                doc: doc_comment(&item.attrs),
                span: item.ident.span(),
            })
        }
        other => Err(error(
            other.span(),
            "`#[bern::value]` applies to enums and structs",
        )),
    }
}

/// Parse a `#[bern::session]` struct (marker attribute already removed).
pub fn parse_session(item: &Item) -> syn::Result<SessionItem> {
    let Item::Struct(item) = item else {
        return Err(error(item.span(), "`#[bern::session]` applies to structs"));
    };
    reject_generics(&item.generics, &item.ident)?;
    check_known_attrs(&item.attrs, &["session"], "session state")?;
    let fields = parse_named_fields(&item.fields, &item.ident, true, true)?;
    if fields.is_empty() {
        return Err(error(
            item.ident.span(),
            "session state needs at least one field",
        ));
    }
    Ok(SessionItem {
        name: item.ident.clone(),
        fields,
        doc: doc_comment(&item.attrs),
        span: item.ident.span(),
    })
}

/// Parse a `#[bern::players]` struct (marker attribute already removed).
/// Every field is a `Player`; its name is the stable `player_id`.
pub fn parse_players(item: &Item) -> syn::Result<PlayersItem> {
    let Item::Struct(item) = item else {
        return Err(error(item.span(), "`#[bern::players]` applies to structs"));
    };
    reject_generics(&item.generics, &item.ident)?;
    check_known_attrs(&item.attrs, &["players"], "player catalogs")?;
    let syn::Fields::Named(fields) = &item.fields else {
        return Err(error(
            item.ident.span(),
            "declare players as named fields: `pub preview: Player`",
        ));
    };
    let mut players = Vec::new();
    for field in &fields.named {
        let name = field.ident.clone().unwrap();
        check_known_attrs(&field.attrs, &[], "players")?;
        if !matches!(field.vis, syn::Visibility::Public(_)) {
            return Err(error(name.span(), format!("player `{name}` must be `pub`")));
        }
        let is_player = matches!(&field.ty, syn::Type::Path(path)
        if path.qself.is_none()
            && path.path.segments.last().is_some_and(|segment| {
                segment.ident == "Player" && segment.arguments.is_none()
            }));
        if !is_player {
            return Err(error(
                field.ty.span(),
                "every field of a player catalog has the type `Player`",
            ));
        }
        if !crate::valid_id(&name.to_string()) {
            return Err(error(
                name.span(),
                "player names are lower snake case; they become stable player ids",
            ));
        }
        players.push(PlayerField {
            doc: doc_comment(&field.attrs),
            span: name.span(),
            name,
        });
    }
    if players.is_empty() {
        return Err(error(item.ident.span(), "declare at least one player"));
    }
    Ok(PlayersItem {
        name: item.ident.clone(),
        players,
        doc: doc_comment(&item.attrs),
        span: item.ident.span(),
    })
}

/// Parse protocol attribute arguments.
pub fn parse_protocol_args(args: TokenStream, span: Span) -> syn::Result<ProtocolArgs> {
    let mut namespace = None;
    let mut version = None;
    let parser = syn::meta::parser(|meta| {
        if meta.path.is_ident("namespace") {
            let value: syn::LitStr = meta.value()?.parse()?;
            namespace = Some(value.value());
            Ok(())
        } else if meta.path.is_ident("version") {
            let value: syn::LitInt = meta.value()?.parse()?;
            version = Some(value.base10_parse()?);
            Ok(())
        } else {
            Err(meta.error("expected `namespace = \"…\"` or `version = N`"))
        }
    });
    syn::parse::Parser::parse2(parser, args)?;
    Ok(ProtocolArgs {
        namespace: namespace
            .ok_or_else(|| error(span, "the protocol needs `namespace = \"…\"`"))?,
        version: version.ok_or_else(|| error(span, "the protocol needs `version = N`"))?,
        span,
    })
}

fn parse_command(method: &syn::TraitItemFn) -> syn::Result<CommandItem> {
    let sig = &method.sig;
    let name = sig.ident.clone();
    check_known_attrs(&method.attrs, &["errors", "idempotent"], "commands")?;
    if method.default.is_some() {
        return Err(error(
            name.span(),
            "commands are implemented by the engine, not in the protocol",
        ));
    }
    if sig.asyncness.is_some() || !sig.generics.params.is_empty() {
        return Err(error(
            name.span(),
            "commands cannot be async or generic in this version",
        ));
    }
    let mut inputs = sig.inputs.iter();
    match inputs.next() {
        Some(syn::FnArg::Receiver(receiver))
            if receiver.reference.is_some() && receiver.mutability.is_some() => {}
        _ => {
            return Err(error(
                name.span(),
                "a command takes `&mut self` as its first parameter",
            ));
        }
    }
    let mut params = Vec::new();
    for input in inputs {
        let syn::FnArg::Typed(typed) = input else {
            unreachable!()
        };
        let syn::Pat::Ident(pat) = &*typed.pat else {
            return Err(error(typed.pat.span(), "parameters must be simple names"));
        };
        params.push(CommandParam {
            name: pat.ident.clone(),
            ty: type_ref(&typed.ty)?,
            syn_ty: (*typed.ty).clone(),
        });
    }
    let result_ty = match &sig.output {
        syn::ReturnType::Type(_, ty) => match &**ty {
            syn::Type::Path(path)
                if path.path.segments.last().map(|s| s.ident == "Result") == Some(true) =>
            {
                let last = path.path.segments.last().unwrap();
                let syn::PathArguments::AngleBracketed(args) = &last.arguments else {
                    return Err(error(ty.span(), "expected `Result<T, CommandError>`"));
                };
                let types: Vec<_> = args
                    .args
                    .iter()
                    .filter_map(|arg| match arg {
                        syn::GenericArgument::Type(ty) => Some(ty.clone()),
                        _ => None,
                    })
                    .collect();
                match types.as_slice() {
                    [ok, syn::Type::Path(err)]
                        if err.path.segments.last().map(|s| s.ident == "CommandError")
                            == Some(true) =>
                    {
                        ok.clone()
                    }
                    _ => return Err(error(ty.span(), "expected `Result<T, CommandError>`")),
                }
            }
            _ => return Err(error(ty.span(), "expected `Result<T, CommandError>`")),
        },
        syn::ReturnType::Default => {
            return Err(error(
                name.span(),
                "commands return `Result<T, CommandError>` so failures reach every entry",
            ));
        }
    };
    let mut errors = Vec::new();
    if let Some(attr) = find_attr(&method.attrs, "errors") {
        let list = attr.parse_args_with(
            syn::punctuated::Punctuated::<syn::LitStr, syn::Token![,]>::parse_terminated,
        )?;
        errors = list.iter().map(syn::LitStr::value).collect();
    }
    Ok(CommandItem {
        result: type_ref(&result_ty)?,
        result_ty,
        params,
        doc: doc_comment(&method.attrs),
        errors,
        idempotent: find_attr(&method.attrs, "idempotent").is_some(),
        span: name.span(),
        name,
    })
}

fn strip_marker(item: &Item, marker: &str) -> Item {
    let mut item = item.clone();
    let attrs = match &mut item {
        Item::Enum(item) => &mut item.attrs,
        Item::Struct(item) => &mut item.attrs,
        Item::Trait(item) => &mut item.attrs,
        _ => return item,
    };
    attrs.retain(|attr| !is_bern_attr(attr, marker));
    item
}

/// Parse a protocol module (protocol attribute already removed).
pub fn parse_protocol(args: ProtocolArgs, module: ItemMod) -> syn::Result<ProtocolItem> {
    let Some((_, items)) = &module.content else {
        return Err(error(
            module.ident.span(),
            "the protocol module must be declared inline: `mod name { … }`",
        ));
    };
    reject_cfg(&module.attrs, "protocol modules")?;
    let mut values = Vec::new();
    let mut state = None;
    let mut commands = None;
    let mut uses = Vec::new();
    for item in items {
        match item {
            Item::Use(item_use) => uses.push(item_use.clone()),
            Item::Enum(syn::ItemEnum { attrs, .. })
            | Item::Struct(syn::ItemStruct { attrs, .. })
                if find_attr(attrs, "value").is_some() =>
            {
                values.push(parse_value(&strip_marker(item, "value"))?);
            }
            Item::Struct(item_struct) if find_attr(&item_struct.attrs, "state").is_some() => {
                if state.is_some() {
                    return Err(error(
                        item_struct.ident.span(),
                        "a protocol has exactly one `#[state]` struct",
                    ));
                }
                let stripped = strip_marker(item, "state");
                let Item::Struct(stripped) = stripped else {
                    unreachable!()
                };
                reject_generics(&stripped.generics, &stripped.ident)?;
                check_known_attrs(&stripped.attrs, &[], "state")?;
                state = Some(StateItem {
                    name: stripped.ident.clone(),
                    fields: parse_named_fields(&stripped.fields, &stripped.ident, false, true)?,
                    span: stripped.ident.span(),
                });
            }
            Item::Trait(item_trait) if find_attr(&item_trait.attrs, "commands").is_some() => {
                if commands.is_some() {
                    return Err(error(
                        item_trait.ident.span(),
                        "a protocol has exactly one `#[commands]` trait",
                    ));
                }
                reject_generics(&item_trait.generics, &item_trait.ident)?;
                let mut methods = Vec::new();
                for trait_item in &item_trait.items {
                    match trait_item {
                        syn::TraitItem::Fn(method) => methods.push(parse_command(method)?),
                        other => {
                            return Err(error(
                                other.span(),
                                "the command trait only contains command methods",
                            ));
                        }
                    }
                }
                commands = Some(CommandsItem {
                    name: item_trait.ident.clone(),
                    methods,
                    span: item_trait.ident.span(),
                });
            }
            other => {
                return Err(error(
                    other.span(),
                    "a protocol module contains `use` items, `#[value]` types, one `#[state]` \
                     struct and one `#[commands]` trait",
                ));
            }
        }
    }
    let state = state.ok_or_else(|| {
        error(
            module.ident.span(),
            "the protocol needs a `#[state] pub struct State { … }`",
        )
    })?;
    let commands = commands.ok_or_else(|| {
        error(
            module.ident.span(),
            "the protocol needs a `#[commands] pub trait Commands { … }`",
        )
    })?;
    Ok(ProtocolItem {
        args,
        doc: doc_comment(&module.attrs),
        module,
        values,
        state,
        commands,
        uses,
    })
}

/// Parse a `#[component]` function (marker attribute already removed).
pub fn parse_component(func: ItemFn) -> syn::Result<ComponentItem> {
    let name = func.sig.ident.clone();
    reject_cfg(&func.attrs, "components")?;
    check_known_attrs(&func.attrs, &["component"], "components")?;
    if !func.sig.generics.params.is_empty() || func.sig.asyncness.is_some() {
        return Err(error(name.span(), "components cannot be generic or async"));
    }
    let returns_view = match &func.sig.output {
        syn::ReturnType::Type(_, ty) => match &**ty {
            syn::Type::ImplTrait(bounds) => bounds.bounds.iter().any(|bound| {
                matches!(bound, syn::TypeParamBound::Trait(t)
                    if t.path.segments.last().map(|s| s.ident == "View") == Some(true))
            }),
            syn::Type::Path(path) => {
                path.path.segments.last().map(|s| s.ident == "View") == Some(true)
            }
            _ => false,
        },
        syn::ReturnType::Default => false,
    };
    if !returns_view {
        return Err(error(name.span(), "a component returns `impl View`"));
    }
    let mut params = Vec::new();
    for input in &func.sig.inputs {
        let syn::FnArg::Typed(typed) = input else {
            return Err(error(input.span(), "components do not take `self`"));
        };
        let syn::Pat::Ident(pat) = &*typed.pat else {
            return Err(error(typed.pat.span(), "parameters must be simple names"));
        };
        let mut kind = ParamKind::Prop;
        for attr in &typed.attrs {
            if is_bern_attr(attr, "session") {
                kind = ParamKind::Session;
            } else {
                return Err(error(
                    attr.span(),
                    "only `#[session]` is allowed on component parameters",
                ));
            }
        }
        params.push(Param {
            name: pat.ident.clone(),
            ty: type_ref(&typed.ty)?,
            syn_ty: (*typed.ty).clone(),
            kind,
            span: pat.ident.span(),
        });
    }
    let stmts = &func.block.stmts;
    let tokens = match stmts.as_slice() {
        [syn::Stmt::Macro(stmt)] if stmt.mac.path.is_ident("ui") => stmt.mac.tokens.clone(),
        [syn::Stmt::Expr(syn::Expr::Macro(expr), None)] if expr.mac.path.is_ident("ui") => {
            expr.mac.tokens.clone()
        }
        _ => {
            return Err(error(
                func.block.span(),
                "a component body is exactly one `ui! { … }` declaration",
            ));
        }
    };
    let body: UiBody = syn::parse2(tokens)?;
    Ok(ComponentItem {
        doc: doc_comment(&func.attrs),
        span: name.span(),
        name,
        params,
        body,
        func,
    })
}

fn strip_fn_marker(func: &ItemFn) -> ItemFn {
    let mut func = func.clone();
    func.attrs.retain(|attr| !is_bern_attr(attr, "component"));
    func
}

fn collect(items: &[Item], path: &mut Vec<String>, out: &mut SourceItems) -> syn::Result<()> {
    for item in items {
        match item {
            Item::Mod(module) => {
                if let Some(attr) = find_attr(&module.attrs, "protocol") {
                    let args = match &attr.meta {
                        Meta::List(list) => list.tokens.clone(),
                        _ => TokenStream::new(),
                    };
                    let args = parse_protocol_args(args, attr.span())?;
                    let mut stripped = module.clone();
                    stripped
                        .attrs
                        .retain(|attr| !is_bern_attr(attr, "protocol"));
                    out.protocols
                        .push((path.clone(), parse_protocol(args, stripped)?));
                } else if let Some((_, content)) = &module.content {
                    path.push(module.ident.to_string());
                    collect(content, path, out)?;
                    path.pop();
                }
            }
            Item::Enum(syn::ItemEnum { attrs, .. })
            | Item::Struct(syn::ItemStruct { attrs, .. })
                if find_attr(attrs, "value").is_some() =>
            {
                reject_cfg(attrs, "value types")?;
                out.values
                    .push((path.clone(), parse_value(&strip_marker(item, "value"))?));
            }
            Item::Struct(syn::ItemStruct { attrs, .. })
                if find_attr(attrs, "session").is_some() =>
            {
                reject_cfg(attrs, "session state")?;
                out.sessions
                    .push((path.clone(), parse_session(&strip_marker(item, "session"))?));
            }
            Item::Struct(syn::ItemStruct { attrs, .. })
                if find_attr(attrs, "players").is_some() =>
            {
                reject_cfg(attrs, "player catalogs")?;
                out.players
                    .push((path.clone(), parse_players(&strip_marker(item, "players"))?));
            }
            Item::Fn(func) if find_attr(&func.attrs, "component").is_some() => {
                out.components
                    .push((path.clone(), parse_component(strip_fn_marker(func))?));
            }
            _ => {}
        }
    }
    Ok(())
}

/// Collect Bern declarations from the text of one Rust source file.
pub fn parse_source_file(source: &str) -> syn::Result<SourceItems> {
    let file = syn::parse_file(source)?;
    let mut out = SourceItems::default();
    collect(&file.items, &mut Vec::new(), &mut out)?;
    Ok(out)
}
