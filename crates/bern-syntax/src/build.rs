//! Compile-time module selection, shared by the metadata compiler and Rust macros.
use crate::{ComponentItem, ast::*};
use std::collections::BTreeMap;
use syn::{Attribute, Item, Meta, Token, punctuated::Punctuated, spanned::Spanned};

#[derive(Clone, Debug, Default)]
pub struct Modules(pub BTreeMap<String, bool>);

fn retain<T>(
    values: &mut Vec<T>,
    mut keep: impl FnMut(&mut T) -> syn::Result<bool>,
) -> syn::Result<()> {
    let mut result = Vec::new();
    for mut value in std::mem::take(values) {
        if keep(&mut value)? {
            result.push(value);
        }
    }
    *values = result;
    Ok(())
}

impl Modules {
    fn cfg(&self, meta: &Meta) -> syn::Result<Option<bool>> {
        match meta {
            Meta::NameValue(value) if value.path.is_ident("feature") => {
                if let syn::Expr::Lit(expr) = &value.value
                    && let syn::Lit::Str(name) = &expr.lit
                    && let Some(id) = name.value().strip_prefix("bern-module-")
                {
                    return self.0.get(id).copied().map(Some).ok_or_else(|| {
                        syn::Error::new(name.span(), format!("unknown build module `{id}`"))
                    });
                }
                Ok(None)
            }
            Meta::List(list)
                if list.path.is_ident("not")
                    || list.path.is_ident("all")
                    || list.path.is_ident("any") =>
            {
                let values =
                    list.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)?;
                let results = values
                    .iter()
                    .map(|m| self.cfg(m))
                    .collect::<syn::Result<Vec<_>>>()?;
                if list.path.is_ident("not") {
                    if results.len() != 1 {
                        return Err(syn::Error::new(list.span(), "not() needs one condition"));
                    }
                    Ok(results[0].map(|v| !v))
                } else if results.iter().any(Option::is_none) {
                    Ok(None)
                } else if list.path.is_ident("all") {
                    Ok(Some(results.iter().all(|v| v == &Some(true))))
                } else {
                    Ok(Some(results.iter().any(|v| v == &Some(true))))
                }
            }
            _ => Ok(None),
        }
    }

    /// Resolve module cfg/cfg_attr; leave unrelated Rust cfg to its existing checker.
    pub fn attrs(&self, attrs: &mut Vec<Attribute>) -> syn::Result<bool> {
        let mut output = Vec::new();
        for attr in std::mem::take(attrs) {
            if attr.path().is_ident("cfg") {
                match self.cfg(&attr.parse_args::<Meta>()?)? {
                    Some(false) => return Ok(false),
                    Some(true) => continue,
                    None => {}
                }
            } else if attr.path().is_ident("cfg_attr") {
                let args = attr.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)?;
                if let Some(condition) = args.first()
                    && let Some(enabled) = self.cfg(condition)?
                {
                    if enabled {
                        for meta in args.iter().skip(1) {
                            output.push(syn::parse_quote!(#[#meta]));
                        }
                    }
                    continue;
                }
            }
            output.push(attr);
        }
        *attrs = output;
        Ok(true)
    }

    pub fn function(&self, func: &mut syn::ItemFn) -> syn::Result<()> {
        let mut inputs = Vec::new();
        for mut arg in std::mem::take(&mut func.sig.inputs) {
            let attrs = match &mut arg {
                syn::FnArg::Typed(t) => &mut t.attrs,
                syn::FnArg::Receiver(r) => &mut r.attrs,
            };
            if self.attrs(attrs)? {
                inputs.push(arg);
            }
        }
        func.sig.inputs = inputs.into_iter().collect();
        Ok(())
    }

    fn fields(&self, fields: &mut syn::Fields) -> syn::Result<()> {
        let mut kept = Vec::new();
        let original = match fields {
            syn::Fields::Named(f) => std::mem::take(&mut f.named),
            syn::Fields::Unnamed(f) => std::mem::take(&mut f.unnamed),
            syn::Fields::Unit => return Ok(()),
        };
        for mut field in original {
            if self.attrs(&mut field.attrs)? {
                kept.push(field);
            }
        }
        match fields {
            syn::Fields::Named(f) => f.named = kept.into_iter().collect(),
            syn::Fields::Unnamed(f) => f.unnamed = kept.into_iter().collect(),
            _ => {}
        }
        Ok(())
    }

    pub fn items(&self, items: &mut Vec<Item>) -> syn::Result<()> {
        retain(items, |item| {
            let attrs = match item {
                Item::Mod(i) => &mut i.attrs,
                Item::Fn(i) => &mut i.attrs,
                Item::Struct(i) => &mut i.attrs,
                Item::Enum(i) => &mut i.attrs,
                Item::Trait(i) => &mut i.attrs,
                Item::Use(i) => &mut i.attrs,
                _ => return Ok(true),
            };
            if !self.attrs(attrs)? {
                return Ok(false);
            }
            match item {
                Item::Mod(i) => {
                    if let Some((_, children)) = &mut i.content {
                        self.items(children)?;
                    }
                }
                Item::Fn(i) => self.function(i)?,
                Item::Struct(i) => self.fields(&mut i.fields)?,
                Item::Enum(i) => {
                    let mut kept = Vec::new();
                    for mut v in std::mem::take(&mut i.variants) {
                        if self.attrs(&mut v.attrs)? {
                            self.fields(&mut v.fields)?;
                            kept.push(v);
                        }
                    }
                    i.variants = kept.into_iter().collect();
                }
                Item::Trait(i) => retain(&mut i.items, |item| match item {
                    syn::TraitItem::Fn(f) => self.attrs(&mut f.attrs),
                    _ => Ok(true),
                })?,
                _ => {}
            }
            Ok(true)
        })
    }

    pub fn component(&self, component: &mut ComponentItem) -> syn::Result<()> {
        self.nodes(&mut component.body.nodes)
    }

    fn boolean(expr: &Expr) -> Option<bool> {
        match expr {
            Expr::Literal {
                value: Literal::Bool(v),
                ..
            } => Some(*v),
            _ => None,
        }
    }

    fn expr(&self, expr: &mut Expr) -> syn::Result<()> {
        let span = expr.span();
        match expr {
            Expr::Path(path) if path.segments.first().is_some_and(|s| s == "build") => {
                if path.segments.len() != 3 || path.segments[1] != "modules" {
                    return Err(syn::Error::new(span, "use build.modules.<module_id>"));
                }
                let id = path.segments[2].to_string();
                let value =
                    self.0.get(&id).copied().ok_or_else(|| {
                        syn::Error::new(span, format!("unknown build module `{id}`"))
                    })?;
                *expr = Expr::Literal {
                    value: Literal::Bool(value),
                    span,
                };
            }
            Expr::Unary {
                op, expr: inner, ..
            } => {
                self.expr(inner)?;
                if *op == UnaryOp::Not
                    && let Some(value) = Self::boolean(inner)
                {
                    *expr = Expr::Literal {
                        value: Literal::Bool(!value),
                        span,
                    };
                }
            }
            Expr::Binary { op, lhs, rhs, .. } => {
                self.expr(lhs)?;
                self.expr(rhs)?;
                let (a, b) = (Self::boolean(lhs), Self::boolean(rhs));
                let result = match op {
                    BinaryOp::And if a == Some(false) || b == Some(false) => Some(false),
                    BinaryOp::Or if a == Some(true) || b == Some(true) => Some(true),
                    BinaryOp::Eq if a.is_some() && b.is_some() => Some(a == b),
                    BinaryOp::Ne if a.is_some() && b.is_some() => Some(a != b),
                    _ => None,
                };
                if let Some(value) = result {
                    *expr = Expr::Literal {
                        value: Literal::Bool(value),
                        span,
                    };
                } else if (*op == BinaryOp::And && a == Some(true))
                    || (*op == BinaryOp::Or && a == Some(false))
                {
                    *expr = *rhs.clone();
                } else if (*op == BinaryOp::And && b == Some(true))
                    || (*op == BinaryOp::Or && b == Some(false))
                {
                    *expr = *lhs.clone();
                }
            }
            Expr::If {
                cond,
                then,
                otherwise,
                ..
            } => {
                self.expr(cond)?;
                if let Some(value) = Self::boolean(cond) {
                    *expr = if value {
                        *then.clone()
                    } else {
                        *otherwise.clone()
                    };
                    self.expr(expr)?;
                } else {
                    self.expr(then)?;
                    self.expr(otherwise)?;
                }
            }
            Expr::Tr(tr) => {
                for arg in &mut tr.args {
                    self.expr(&mut arg.value)?;
                }
            }
            Expr::Variant(v) => {
                for (_, value) in &mut v.fields {
                    self.expr(value)?;
                }
            }
            Expr::Query(a) => {
                for arg in &mut a.args {
                    self.expr(arg)?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn props(&self, props: &mut Vec<Prop>) -> syn::Result<()> {
        retain(props, |prop| {
            if !self.attrs(&mut prop.attrs)? {
                return Ok(false);
            }
            if !prop.attrs.is_empty() {
                return Err(syn::Error::new(
                    prop.attrs[0].span(),
                    "UI properties accept only module cfg attributes",
                ));
            }
            self.expr(&mut prop.value)?;
            Ok(true)
        })
    }

    fn nodes(&self, nodes: &mut Vec<Node>) -> syn::Result<()> {
        let mut out = Vec::new();
        for mut node in std::mem::take(nodes) {
            match &mut node {
                Node::Element(e) => {
                    self.props(&mut e.props)?;
                    if let Some(children) = &mut e.children {
                        self.nodes(children)?;
                    }
                    if let Some(action) = &mut e.action {
                        for arg in &mut action.args {
                            self.expr(arg)?;
                        }
                    }
                }
                Node::Component(c) => self.props(&mut c.args)?,
                Node::If(i) => {
                    let mut branches = Vec::new();
                    let mut fallback = i.otherwise.take();
                    for (mut condition, mut body) in std::mem::take(&mut i.branches) {
                        self.expr(&mut condition)?;
                        match Self::boolean(&condition) {
                            Some(false) => continue,
                            Some(true) => {
                                fallback = Some(body);
                                break;
                            }
                            None => {
                                self.nodes(&mut body)?;
                                branches.push((condition, body));
                            }
                        }
                    }
                    if let Some(body) = &mut fallback {
                        self.nodes(body)?;
                    }
                    if branches.is_empty() {
                        out.extend(fallback.unwrap_or_default());
                        continue;
                    }
                    i.branches = branches;
                    i.otherwise = fallback;
                }
                Node::IfLet(i) => {
                    self.expr(&mut i.value)?;
                    self.nodes(&mut i.then)?;
                    if let Some(body) = &mut i.otherwise {
                        self.nodes(body)?;
                    }
                }
                Node::Match(m) => {
                    self.expr(&mut m.value)?;
                    retain(&mut m.arms, |arm| {
                        if !self.attrs(&mut arm.attrs)? {
                            return Ok(false);
                        }
                        if !arm.attrs.is_empty() {
                            return Err(syn::Error::new(
                                arm.span,
                                "match arms accept only module cfg attributes",
                            ));
                        }
                        self.nodes(&mut arm.body)?;
                        Ok(true)
                    })?;
                }
            }
            out.push(node);
        }
        *nodes = out;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn modules() -> Modules {
        Modules(BTreeMap::from([
            ("notes".into(), false),
            ("tags".into(), true),
        ]))
    }

    #[test]
    fn prunes_branches_expressions_parameters_and_props_before_checks() {
        let source = r#"
            #[component]
            pub fn page(#[cfg(feature = "bern-module-notes")] notes: notes::Client, show: bool) -> impl View {
                ui! { Column {
                    if build.modules.notes && notes.open { missing(notes: notes) }
                    else if show { child(#[cfg(feature = "bern-module-notes")] notes: notes) }
                    else if build.modules.tags { Text(content: if build.modules.notes { notes.title } else { "present" }) }
                    else { unknown() }
                    match show {
                        #[cfg(feature = "bern-module-notes")] Missing::Variant => { missing() }
                        _ => { Spacer }
                    }
                } }
            }
        "#;
        let items = crate::items::parse_source_file_with_modules(source, &modules()).unwrap();
        let component = &items.components[0].1;
        assert_eq!(component.params.len(), 1);
        let Node::Element(root) = &component.body.nodes[0] else {
            panic!()
        };
        let children = root.children.as_ref().unwrap();
        let Node::If(branches) = &children[0] else {
            panic!()
        };
        assert_eq!(branches.branches.len(), 1);
        let Node::Component(call) = &branches.branches[0].1[0] else {
            panic!()
        };
        assert!(call.args.is_empty());
        let Node::Element(text) = &branches.otherwise.as_ref().unwrap()[0] else {
            panic!()
        };
        assert!(
            matches!(&text.props[0].value, Expr::Literal { value: Literal::Text(v), .. } if v == "present")
        );
        let Node::Match(m) = &children[1] else {
            panic!()
        };
        assert_eq!(m.arms.len(), 1);
    }

    #[test]
    fn cfg_and_cfg_attr_share_protocol_selection() {
        let mut items = syn::parse_file(
            r#"
            #[cfg(feature = "bern-module-notes")] fn removed() {}
            #[cfg(not(feature = "bern-module-notes"))] fn kept() {}
            trait Commands {
                #[cfg_attr(feature = "bern-module-notes", on(notes::changed))]
                fn plain();
                #[cfg(feature = "bern-module-notes")] fn removed();
            }
        "#,
        )
        .unwrap()
        .items;
        modules().items(&mut items).unwrap();
        assert_eq!(items.len(), 2);
        let Item::Trait(t) = &items[1] else { panic!() };
        assert_eq!(t.items.len(), 1);
        let syn::TraitItem::Fn(f) = &t.items[0] else {
            panic!()
        };
        assert!(f.attrs.is_empty());
    }

    #[test]
    fn misspelled_build_module_is_an_error() {
        let source = "#[component] fn page() -> impl View { ui! { if build.modules.note { Spacer } else { Spacer } } }";
        let error = crate::items::parse_source_file_with_modules(source, &modules()).unwrap_err();
        assert!(error.to_string().contains("unknown build module `note`"));
    }
}
