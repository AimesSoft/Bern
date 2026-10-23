use crate::ast::*;
use crate::expr::parse_expr;
use syn::parse::{Parse, ParseStream};
use syn::{Ident, Token, braced, parenthesized, token};

impl Parse for UiBody {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let nodes = parse_nodes(input)?;
        Ok(UiBody { nodes })
    }
}

fn parse_nodes(input: ParseStream<'_>) -> syn::Result<Vec<Node>> {
    let mut nodes = Vec::new();
    while !input.is_empty() {
        nodes.push(parse_node(input)?);
        // Children are separated by line breaks; a stray comma is tolerated
        // only to give a precise message.
        if input.peek(Token![,]) {
            return Err(input.error("separate child nodes with line breaks, not commas"));
        }
    }
    Ok(nodes)
}

fn parse_block(input: ParseStream<'_>) -> syn::Result<Vec<Node>> {
    let content;
    braced!(content in input);
    parse_nodes(&content)
}

fn parse_node(input: ParseStream<'_>) -> syn::Result<Node> {
    if input.peek(Token![if]) {
        return parse_if(input);
    }
    if input.peek(Token![match]) {
        return parse_match(input).map(Node::Match);
    }
    if input.peek(Token![for]) {
        return Err(input.error(
            "`for` loops are not part of the grammar; use `List(items: …, key: …) |item| { … }` \
             or `ForEach(items: …, key: …) |item| { … }`",
        ));
    }
    if input.peek(Token![let]) {
        return Err(input.error("`let` statements are not allowed inside `ui!`"));
    }
    if input.peek(Ident) {
        let fork = input.fork();
        let name: Ident = fork.parse()?;
        let first = name.to_string().chars().next().unwrap_or('a');
        if first.is_ascii_uppercase() {
            return parse_element(input).map(Node::Element);
        }
        return parse_component_call(input).map(Node::Component);
    }
    Err(input.error("expected an element, a component call, `if` or `match`"))
}

fn parse_props(input: ParseStream<'_>) -> syn::Result<Vec<Prop>> {
    let content;
    parenthesized!(content in input);
    let mut props = Vec::new();
    while !content.is_empty() {
        let attrs = content.call(syn::Attribute::parse_outer)?;
        let name: Ident = content.parse()?;
        if !content.peek(Token![:]) {
            return Err(syn::Error::new(
                name.span(),
                format!("expected `{name}: value`; properties are always named"),
            ));
        }
        content.parse::<Token![:]>()?;
        let value = match call_segments(&content) {
            Some(segments) if name == "query" => {
                if segments < 2 {
                    return Err(content.error(
                        "a query is called on a typed client, for example \
                         `query: app.search(draft.text)`",
                    ));
                }
                Expr::Query(parse_call(&content)?)
            }
            _ => parse_expr(&content, true)?,
        };
        if props.iter().any(|prop: &Prop| prop.name == name) {
            return Err(syn::Error::new(
                name.span(),
                format!("property `{name}` is given twice"),
            ));
        }
        props.push(Prop { attrs, name, value });
        if content.is_empty() {
            break;
        }
        content.parse::<Token![,]>()?;
    }
    Ok(props)
}

fn parse_element(input: ParseStream<'_>) -> syn::Result<Element> {
    let name: Ident = input.parse()?;
    let span = name.span();
    let props = if input.peek(token::Paren) {
        parse_props(input)?
    } else {
        Vec::new()
    };
    let template = if input.peek(Token![|]) {
        input.parse::<Token![|]>()?;
        let ident: Ident = input.parse()?;
        input.parse::<Token![|]>()?;
        Some(ident)
    } else {
        None
    };
    let children = if input.peek(token::Brace) {
        Some(parse_block(input)?)
    } else {
        None
    };
    if template.is_some() && children.is_none() {
        return Err(syn::Error::new(
            span,
            "a row template `|item|` must be followed by `{ … }`",
        ));
    }
    let action = if input.peek(Token![=>]) {
        input.parse::<Token![=>]>()?;
        Some(parse_action(input)?)
    } else {
        None
    };
    Ok(Element {
        name,
        props,
        template,
        children,
        action,
        span,
    })
}

/// Number of path segments when the input starts with `a.b(…)`.
fn call_segments(input: ParseStream<'_>) -> Option<usize> {
    let fork = input.fork();
    fork.parse::<Ident>().ok()?;
    let mut segments = 1;
    while fork.peek(Token![.]) {
        fork.parse::<Token![.]>().ok()?;
        fork.parse::<Ident>().ok()?;
        segments += 1;
    }
    fork.peek(token::Paren).then_some(segments)
}

fn parse_action(input: ParseStream<'_>) -> syn::Result<Action> {
    if input.peek(Token![|]) || input.peek(Token![move]) || input.peek(token::Brace) {
        return Err(input.error(
            "arbitrary callbacks are not allowed; bind a registered command such as \
             `=> app.save()`",
        ));
    }
    parse_call(input)
}

/// `receiver.member.method(positional args)`.
fn parse_call(input: ParseStream<'_>) -> syn::Result<Action> {
    let span = input.span();
    let mut segments: Vec<Ident> = vec![input.parse()?];
    while input.peek(Token![.]) {
        input.parse::<Token![.]>()?;
        segments.push(input.parse()?);
    }
    if input.peek(Token![=]) {
        return Err(
            input.error("direct assignment is not allowed; state changes go through commands")
        );
    }
    if !input.peek(token::Paren) {
        return Err(syn::Error::new(
            span,
            "an action must call a command, for example `=> app.save()`",
        ));
    }
    if segments.len() < 2 {
        return Err(syn::Error::new(
            span,
            "an action must call a command on a typed client, for example `=> app.save()`",
        ));
    }
    let content;
    parenthesized!(content in input);
    let mut args = Vec::new();
    while !content.is_empty() {
        if content.peek(Ident) && content.peek2(Token![:]) && !content.peek2(Token![::]) {
            return Err(content.error(
                "command arguments are positional, in the order the command declares them",
            ));
        }
        args.push(parse_expr(&content, true)?);
        if content.is_empty() {
            break;
        }
        content.parse::<Token![,]>()?;
    }
    let method = segments.pop().unwrap();
    Ok(Action {
        receiver: segments,
        method,
        args,
        span,
    })
}

fn parse_component_call(input: ParseStream<'_>) -> syn::Result<ComponentCall> {
    let span = input.span();
    let mut path: Vec<Ident> = vec![input.parse()?];
    while input.peek(Token![::]) {
        input.parse::<Token![::]>()?;
        path.push(input.parse()?);
    }
    if input.peek(Token![.]) {
        return Err(syn::Error::new(
            span,
            "a binding cannot be a child node; wrap it in `Text(content: …)`",
        ));
    }
    if input.peek(Token![!]) {
        return Err(syn::Error::new(
            span,
            "macros other than `tr!` are not allowed inside `ui!`",
        ));
    }
    if !input.peek(token::Paren) {
        return Err(syn::Error::new(
            span,
            "a component reference needs an argument list, for example `settings_screen()`",
        ));
    }
    let args = parse_props(input)?;
    if input.peek(token::Brace) {
        return Err(input.error("component references do not take children in this version"));
    }
    if input.peek(Token![=>]) {
        return Err(input.error("component references cannot bind actions"));
    }
    Ok(ComponentCall { path, args, span })
}

fn parse_if(input: ParseStream<'_>) -> syn::Result<Node> {
    let span = input.parse::<Token![if]>()?.span;
    if input.peek(Token![let]) {
        input.parse::<Token![let]>()?;
        let some: Ident = input.parse()?;
        if some != "Some" {
            return Err(syn::Error::new(
                some.span(),
                "only `if let Some(name) = value` is supported",
            ));
        }
        let content;
        parenthesized!(content in input);
        let binding: Ident = content.parse()?;
        if !content.is_empty() {
            return Err(content.error("expected a single binding name"));
        }
        input.parse::<Token![=]>()?;
        let value = parse_expr(input, false)?;
        let then = parse_block(input)?;
        let otherwise = parse_else(input)?;
        return Ok(Node::IfLet(IfLetNode {
            binding,
            value,
            then,
            otherwise,
            span,
        }));
    }
    let mut branches = Vec::new();
    let condition = parse_expr(input, false)?;
    branches.push((condition, parse_block(input)?));
    let mut otherwise = None;
    while input.peek(Token![else]) {
        input.parse::<Token![else]>()?;
        if input.peek(Token![if]) {
            input.parse::<Token![if]>()?;
            if input.peek(Token![let]) {
                return Err(input.error("`else if let` is not supported; nest the `if let`"));
            }
            let condition = parse_expr(input, false)?;
            branches.push((condition, parse_block(input)?));
        } else {
            otherwise = Some(parse_block(input)?);
            break;
        }
    }
    Ok(Node::If(IfNode {
        branches,
        otherwise,
        span,
    }))
}

fn parse_else(input: ParseStream<'_>) -> syn::Result<Option<Vec<Node>>> {
    if input.peek(Token![else]) {
        input.parse::<Token![else]>()?;
        if input.peek(Token![if]) {
            return Err(input.error("`else if` after `if let` is not supported"));
        }
        Ok(Some(parse_block(input)?))
    } else {
        Ok(None)
    }
}

fn parse_match(input: ParseStream<'_>) -> syn::Result<MatchNode> {
    let span = input.parse::<Token![match]>()?.span;
    let value = parse_expr(input, false)?;
    let content;
    braced!(content in input);
    let mut arms = Vec::new();
    while !content.is_empty() {
        let attrs = content.call(syn::Attribute::parse_outer)?;
        let arm_span = content.span();
        let pattern = if content.peek(Token![_]) {
            Pattern::Wildcard(content.parse::<Token![_]>()?.span)
        } else {
            let ty: Ident = content.parse()?;
            content.parse::<Token![::]>()?;
            let variant: Ident = content.parse()?;
            let mut bindings = Vec::new();
            if content.peek(token::Brace) {
                let fields;
                braced!(fields in content);
                while !fields.is_empty() {
                    bindings.push(fields.parse()?);
                    if fields.is_empty() {
                        break;
                    }
                    fields.parse::<Token![,]>()?;
                }
            } else if content.peek(token::Paren) {
                return Err(
                    content.error("use named fields in patterns: `Type::Variant { field }`")
                );
            }
            Pattern::Variant {
                ty,
                variant,
                bindings,
            }
        };
        if content.peek(Token![if]) {
            return Err(content.error("match guards are not supported"));
        }
        content.parse::<Token![=>]>()?;
        if !content.peek(token::Brace) {
            return Err(content.error("each match arm needs a `{ … }` block of nodes"));
        }
        let body = parse_block(&content)?;
        arms.push(MatchArm {
            attrs,
            pattern,
            body,
            span: arm_span,
        });
        if content.peek(Token![,]) {
            content.parse::<Token![,]>()?;
        }
    }
    if arms.is_empty() {
        return Err(syn::Error::new(span, "`match` needs at least one arm"));
    }
    Ok(MatchNode { value, arms, span })
}
