//! `style!`: the application's style overrides and motion tokens.
//!
//! ```text
//! style! {
//!     motion {
//!         jelly: spring(period: 0.42, damping: 0.42),
//!     }
//!     SidebarItem {
//!         corner: 8,
//!         hover: scale(1.03, snappy),
//!         selected: fill(accent),
//!     }
//!     Button.hero { size: large }
//!     #[target(macos)]
//!     SidebarItem { selected: system }
//! }
//! ```
//!
//! This module only reads the declaration; what each property means and
//! where it applies is checked by the build tool.

use proc_macro2::{Span, TokenStream};
use syn::ext::IdentExt;
use syn::parse::{Parse, ParseStream};
use syn::spanned::Spanned;
use syn::{Ident, LitFloat, LitInt, LitStr, Token, braced, parenthesized};

#[derive(Clone, Debug)]
pub struct StyleItem {
    pub motion: Vec<MotionDecl>,
    /// `transition { name: pop(from: 0.4, jelly) }`.
    pub transitions: Vec<MotionDecl>,
    /// `pattern { name: check(band(accent, 1, 0.28), gap(6), …) }`.
    pub patterns: Vec<MotionDecl>,
    pub rules: Vec<StyleRuleItem>,
    pub span: Span,
}

/// `name: spring(period: T, damping: ζ)`.
#[derive(Clone, Debug)]
pub struct MotionDecl {
    pub name: Ident,
    pub value: StyleValue,
}

/// `Component` or `Component.variant`, with its properties.
#[derive(Clone, Debug)]
pub struct StyleRuleItem {
    pub component: Ident,
    pub variant: Option<Ident>,
    /// `#[target(macos, ios)]`: the platforms the rule applies to; empty
    /// for all.
    pub targets: Vec<Ident>,
    /// `#[exempt(gtk, "reason")]`: platforms that keep their default for
    /// what they cannot show.
    pub exempt: Vec<(Ident, String)>,
    pub props: Vec<StyleProp>,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct StyleProp {
    pub name: Ident,
    pub value: StyleValue,
}

/// A property value: a number, a name such as `accent` or `fill.subtle`,
/// text, a call such as `scale(1.03, snappy)`, or effects joined by `+`.
#[derive(Clone, Debug)]
pub enum StyleValue {
    Number(f64, Span),
    Name(Vec<Ident>),
    Text(String, Span),
    Call { name: Ident, args: Vec<StyleArg> },
    Sum(Vec<StyleValue>),
}

#[derive(Clone, Debug)]
pub struct StyleArg {
    pub name: Option<Ident>,
    pub value: StyleValue,
}

impl StyleValue {
    pub fn span(&self) -> Span {
        match self {
            StyleValue::Number(_, span) | StyleValue::Text(_, span) => *span,
            StyleValue::Name(path) => path[0].span(),
            StyleValue::Call { name, .. } => name.span(),
            StyleValue::Sum(parts) => parts[0].span(),
        }
    }

    /// `a.b` for a name.
    pub fn dotted(&self) -> Option<String> {
        match self {
            StyleValue::Name(path) => Some(
                path.iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("."),
            ),
            _ => None,
        }
    }

    /// The parts of a sum, or the value alone.
    pub fn terms(&self) -> Vec<&StyleValue> {
        match self {
            StyleValue::Sum(parts) => parts.iter().collect(),
            other => vec![other],
        }
    }
}

impl Parse for StyleItem {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let span = input.span();
        let mut motion = Vec::new();
        let mut transitions = Vec::new();
        let mut patterns = Vec::new();
        let mut rules = Vec::new();
        while !input.is_empty() {
            let attrs = input.call(syn::Attribute::parse_outer)?;
            let component: Ident = input.parse()?;
            if (component == "motion" || component == "transition" || component == "pattern")
                && attrs.is_empty()
                && input.peek(syn::token::Brace)
            {
                let content;
                braced!(content in input);
                let list = if component == "motion" {
                    &mut motion
                } else if component == "transition" {
                    &mut transitions
                } else {
                    &mut patterns
                };
                while !content.is_empty() {
                    let name: Ident = content.parse()?;
                    content.parse::<Token![:]>()?;
                    let value: StyleValue = content.parse()?;
                    list.push(MotionDecl { name, value });
                    if !content.is_empty() {
                        content.parse::<Token![,]>()?;
                    }
                }
                continue;
            }
            let variant = if input.peek(Token![.]) {
                input.parse::<Token![.]>()?;
                Some(input.parse()?)
            } else {
                None
            };
            let mut targets = Vec::new();
            let mut exempt = Vec::new();
            for attr in &attrs {
                if attr.path().is_ident("target") {
                    let list = attr.parse_args_with(
                        syn::punctuated::Punctuated::<Ident, Token![,]>::parse_terminated,
                    )?;
                    targets.extend(list);
                } else if attr.path().is_ident("exempt") {
                    attr.parse_args_with(|input: ParseStream| {
                        let target: Ident = input.parse()?;
                        input.parse::<Token![,]>()?;
                        let reason: LitStr = input.parse()?;
                        exempt.push((target, reason.value()));
                        Ok(())
                    })?;
                } else {
                    return Err(syn::Error::new(
                        attr.span(),
                        "style rules take `#[target(…)]` and `#[exempt(target, \"reason\")]`",
                    ));
                }
            }
            let content;
            braced!(content in input);
            let mut props = Vec::new();
            while !content.is_empty() {
                let name: Ident = content.parse()?;
                content.parse::<Token![:]>()?;
                let value: StyleValue = content.parse()?;
                props.push(StyleProp { name, value });
                if !content.is_empty() {
                    content.parse::<Token![,]>()?;
                }
            }
            rules.push(StyleRuleItem {
                span: component.span(),
                component,
                variant,
                targets,
                exempt,
                props,
            });
        }
        Ok(StyleItem {
            motion,
            transitions,
            patterns,
            rules,
            span,
        })
    }
}

impl Parse for StyleValue {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut parts = vec![term(input)?];
        while input.peek(Token![+]) {
            input.parse::<Token![+]>()?;
            parts.push(term(input)?);
        }
        Ok(if parts.len() == 1 {
            parts.pop().unwrap()
        } else {
            StyleValue::Sum(parts)
        })
    }
}

fn number(input: ParseStream, negative: bool) -> syn::Result<StyleValue> {
    let sign = if negative { -1.0 } else { 1.0 };
    if input.peek(LitFloat) {
        let lit: LitFloat = input.parse()?;
        Ok(StyleValue::Number(
            sign * lit.base10_parse::<f64>()?,
            lit.span(),
        ))
    } else {
        let lit: LitInt = input.parse()?;
        Ok(StyleValue::Number(
            sign * lit.base10_parse::<f64>()?,
            lit.span(),
        ))
    }
}

fn term(input: ParseStream) -> syn::Result<StyleValue> {
    if input.peek(Token![-]) {
        input.parse::<Token![-]>()?;
        return number(input, true);
    }
    if input.peek(LitFloat) || input.peek(LitInt) {
        return number(input, false);
    }
    if input.peek(LitStr) {
        let lit: LitStr = input.parse()?;
        return Ok(StyleValue::Text(lit.value(), lit.span()));
    }
    // Any word, so `twill: true` reads as a name.
    let name: Ident = input.call(Ident::parse_any)?;
    if input.peek(syn::token::Paren) {
        let content;
        parenthesized!(content in input);
        let mut args = Vec::new();
        while !content.is_empty() {
            let named = content.peek(Ident) && content.peek2(Token![:]);
            let name = if named {
                let name: Ident = content.parse()?;
                content.parse::<Token![:]>()?;
                Some(name)
            } else {
                None
            };
            let value: StyleValue = content.parse()?;
            args.push(StyleArg { name, value });
            if !content.is_empty() {
                content.parse::<Token![,]>()?;
            }
        }
        return Ok(StyleValue::Call { name, args });
    }
    let mut path = vec![name];
    while input.peek(Token![.]) {
        input.parse::<Token![.]>()?;
        path.push(input.parse()?);
    }
    Ok(StyleValue::Name(path))
}

/// Parse the body of a `style!` invocation.
pub fn parse_style(tokens: TokenStream) -> syn::Result<StyleItem> {
    syn::parse2(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_style_reads_rules_motion_and_attributes() {
        let style = parse_style(quote::quote! {
            motion {
                jelly: spring(period: 0.42, damping: 0.42),
            }
            SidebarItem {
                corner: 8,
                hover: scale(1.03, snappy),
                selected: fill(accent),
            }
            IconButton {
                hover: background(fill.subtle) + scale(1.08, snappy),
            }
            #[target(macos)]
            #[exempt(gtk, "no transforms on rows")]
            Button.hero { size: large, pressed: scale(x: 0.97, y: 0.9, press) }
        })
        .unwrap();
        assert_eq!(style.motion.len(), 1);
        assert_eq!(style.motion[0].name, "jelly");
        assert_eq!(style.rules.len(), 3);
        let icon = &style.rules[1];
        let StyleValue::Sum(parts) = &icon.props[0].value else {
            panic!("a sum")
        };
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0].clone().terms().len(), 1);
        let hero = &style.rules[2];
        assert_eq!(hero.variant.as_ref().unwrap(), "hero");
        assert_eq!(hero.targets[0], "macos");
        assert_eq!(hero.exempt[0].1, "no transforms on rows");
        let StyleValue::Call { args, .. } = &hero.props[1].value else {
            panic!("a call")
        };
        assert_eq!(args[0].name.as_ref().unwrap(), "x");
        assert!(matches!(args[2].value, StyleValue::Name(_)));
        assert!(parse_style(quote::quote! { Button { corner } }).is_err());
    }
}
