use crate::ast::*;
use syn::parse::{Parse, ParseStream};
use syn::{Ident, LitStr, Token, braced, parenthesized, token};

/// Parse a restricted expression. `allow_braces` is false where a `{` must
/// start a block, mirroring Rust's rule for `if` conditions.
pub(crate) fn parse_expr(input: ParseStream<'_>, allow_braces: bool) -> syn::Result<Expr> {
    parse_binary(input, 1, allow_braces)
}

fn peek_binary(input: ParseStream<'_>) -> Option<(BinaryOp, u8)> {
    if input.peek(Token![||]) {
        Some((BinaryOp::Or, 1))
    } else if input.peek(Token![&&]) {
        Some((BinaryOp::And, 2))
    } else if input.peek(Token![==]) {
        Some((BinaryOp::Eq, 3))
    } else if input.peek(Token![!=]) {
        Some((BinaryOp::Ne, 3))
    } else if input.peek(Token![<=]) {
        Some((BinaryOp::Le, 3))
    } else if input.peek(Token![>=]) {
        Some((BinaryOp::Ge, 3))
    } else if input.peek(Token![<]) {
        Some((BinaryOp::Lt, 3))
    } else if input.peek(Token![>]) && !input.peek(Token![=>]) {
        Some((BinaryOp::Gt, 3))
    } else {
        None
    }
}

fn consume_binary(input: ParseStream<'_>, op: BinaryOp) -> syn::Result<proc_macro2::Span> {
    Ok(match op {
        BinaryOp::Or => input.parse::<Token![||]>()?.spans[0],
        BinaryOp::And => input.parse::<Token![&&]>()?.spans[0],
        BinaryOp::Eq => input.parse::<Token![==]>()?.spans[0],
        BinaryOp::Ne => input.parse::<Token![!=]>()?.spans[0],
        BinaryOp::Le => input.parse::<Token![<=]>()?.spans[0],
        BinaryOp::Ge => input.parse::<Token![>=]>()?.spans[0],
        BinaryOp::Lt => input.parse::<Token![<]>()?.span,
        BinaryOp::Gt => input.parse::<Token![>]>()?.span,
    })
}

fn parse_binary(input: ParseStream<'_>, min: u8, allow_braces: bool) -> syn::Result<Expr> {
    let mut lhs = parse_unary(input, allow_braces)?;
    while let Some((op, precedence)) = peek_binary(input) {
        if precedence < min {
            break;
        }
        let span = consume_binary(input, op)?;
        let rhs = parse_binary(input, precedence + 1, allow_braces)?;
        // Comparisons do not chain: `a == b == c` is rejected.
        if precedence == 3
            && let Some((_, 3)) = peek_binary(input)
        {
            return Err(syn::Error::new(
                input.span(),
                "comparison operators cannot be chained; use parentheses with && or ||",
            ));
        }
        lhs = Expr::Binary {
            op,
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
            span,
        };
    }
    Ok(lhs)
}

fn parse_unary(input: ParseStream<'_>, allow_braces: bool) -> syn::Result<Expr> {
    if input.peek(Token![!]) {
        let span = input.parse::<Token![!]>()?.span;
        let expr = parse_unary(input, allow_braces)?;
        return Ok(Expr::Unary {
            op: UnaryOp::Not,
            expr: Box::new(expr),
            span,
        });
    }
    if input.peek(Token![-]) {
        let span = input.parse::<Token![-]>()?.span;
        let expr = parse_unary(input, allow_braces)?;
        return Ok(Expr::Unary {
            op: UnaryOp::Neg,
            expr: Box::new(expr),
            span,
        });
    }
    parse_primary(input, allow_braces)
}

/// `if cond { a } else { b }`, with `else if` for more cases; every case
/// has a value, so `else` is required.
fn parse_if(input: ParseStream<'_>) -> syn::Result<Expr> {
    let span = input.parse::<Token![if]>()?.span;
    let cond = parse_expr(input, false)?;
    let branch = |input: ParseStream<'_>| -> syn::Result<Expr> {
        let content;
        braced!(content in input);
        let value = parse_expr(&content, true)?;
        if !content.is_empty() {
            return Err(content.error("a branch holds one value"));
        }
        Ok(value)
    };
    let then = branch(input)?;
    if !input.peek(Token![else]) {
        return Err(input.error("a value `if` needs an `else`"));
    }
    input.parse::<Token![else]>()?;
    let otherwise = if input.peek(Token![if]) {
        parse_if(input)?
    } else {
        branch(input)?
    };
    Ok(Expr::If {
        cond: Box::new(cond),
        then: Box::new(then),
        otherwise: Box::new(otherwise),
        span,
    })
}

fn parse_primary(input: ParseStream<'_>, allow_braces: bool) -> syn::Result<Expr> {
    if input.peek(Token![if]) {
        return parse_if(input);
    }
    if input.peek(token::Paren) {
        let content;
        parenthesized!(content in input);
        let expr = parse_expr(&content, true)?;
        if !content.is_empty() {
            return Err(content.error("unexpected tokens in parentheses"));
        }
        return Ok(expr);
    }
    if input.peek(syn::Lit) {
        let lit: syn::Lit = input.parse()?;
        let span = lit.span();
        let value = match &lit {
            syn::Lit::Str(value) => Literal::Text(value.value()),
            syn::Lit::Int(value) => {
                if !value.suffix().is_empty() {
                    return Err(syn::Error::new(span, "numeric suffixes are not allowed"));
                }
                Literal::Int(value.base10_parse()?)
            }
            syn::Lit::Float(value) => {
                if !value.suffix().is_empty() {
                    return Err(syn::Error::new(span, "numeric suffixes are not allowed"));
                }
                Literal::Float(value.base10_parse()?)
            }
            syn::Lit::Bool(value) => Literal::Bool(value.value),
            _ => {
                return Err(syn::Error::new(
                    span,
                    "only text, integer, float and bool literals are allowed",
                ));
            }
        };
        return Ok(Expr::Literal { value, span });
    }
    if input.peek(Ident) && input.peek2(Token![!]) {
        let fork = input.fork();
        let name: Ident = fork.parse()?;
        if name == "tr" {
            input.parse::<Ident>()?;
            input.parse::<Token![!]>()?;
            let content;
            let span = name.span();
            if input.peek(token::Brace) {
                braced!(content in input);
            } else if input.peek(token::Paren) {
                parenthesized!(content in input);
            } else {
                return Err(input.error("expected `tr! { ... }`"));
            }
            let mut tr = parse_tr_body(&content)?;
            tr.span = span;
            return Ok(Expr::Tr(tr));
        }
        return Err(syn::Error::new(
            name.span(),
            format!("macro `{name}!` is not allowed here; only `tr!` may appear in declarations"),
        ));
    }
    if input.peek(Ident) || input.peek(Token![self]) {
        if input.peek(Token![self]) {
            return Err(input.error("`self` is not available in declarations"));
        }
        let first: Ident = input.parse()?;
        if input.peek(Token![::]) {
            input.parse::<Token![::]>()?;
            let variant: Ident = input.parse()?;
            if input.peek(Token![::]) {
                return Err(input.error(
                    "use `Type::Variant`; import the type instead of writing a module path",
                ));
            }
            let mut fields = Vec::new();
            let mut has_braces = false;
            if allow_braces && input.peek(token::Brace) {
                has_braces = true;
                let content;
                braced!(content in input);
                while !content.is_empty() {
                    let name: Ident = content.parse()?;
                    content.parse::<Token![:]>()?;
                    let value = parse_expr(&content, true)?;
                    fields.push((name, value));
                    if content.is_empty() {
                        break;
                    }
                    content.parse::<Token![,]>()?;
                }
            }
            if input.peek(token::Paren) {
                return Err(input.error(
                    "function calls are not allowed in declarations; \
                     use named fields `Type::Variant { field: value }`",
                ));
            }
            return Ok(Expr::Variant(VariantExpr {
                ty: first,
                variant,
                fields,
                has_braces,
            }));
        }
        let mut segments = vec![first];
        while input.peek(Token![.]) && !input.peek(Token![..]) {
            input.parse::<Token![.]>()?;
            if input.peek(syn::LitInt) {
                return Err(input.error("tuple field access is not supported"));
            }
            segments.push(input.parse()?);
        }
        if input.peek(token::Paren) {
            return Err(syn::Error::new(
                segments.last().unwrap().span(),
                "function and method calls are not allowed in expressions; \
                 move the computation into a query or command",
            ));
        }
        if input.peek(token::Bracket) {
            return Err(input.error("indexing is not allowed in expressions"));
        }
        return Ok(Expr::Path(PathExpr { segments }));
    }
    Err(input.error("expected a literal, `tr!`, a binding path or `Type::Variant`"))
}

pub(crate) fn parse_tr_body(input: ParseStream<'_>) -> syn::Result<Tr> {
    let mut entries = Vec::new();
    let span = input.span();
    while !input.is_empty() && !input.peek(Token![;]) {
        let language: LitStr = input.parse()?;
        input.parse::<Token![=>]>()?;
        let text: LitStr = input.parse()?;
        entries.push(TrEntry {
            language: language.value(),
            text: text.value(),
            span: language.span(),
        });
        if input.is_empty() || input.peek(Token![;]) {
            break;
        }
        input.parse::<Token![,]>()?;
    }
    let mut args = Vec::new();
    if input.peek(Token![;]) {
        input.parse::<Token![;]>()?;
        while !input.is_empty() {
            let name: Ident = input.parse()?;
            if input.peek(Token![=]) {
                input.parse::<Token![=]>()?;
            } else {
                input.parse::<Token![:]>()?;
            }
            let value = parse_expr(input, true)?;
            args.push(TrArg { name, value });
            if input.is_empty() {
                break;
            }
            input.parse::<Token![,]>()?;
        }
    }
    if entries.is_empty() {
        return Err(syn::Error::new(
            span,
            "`tr!` needs at least one translation such as `\"en\" => \"Text\"`",
        ));
    }
    Ok(Tr {
        entries,
        args,
        span,
    })
}

pub(crate) struct TrBody(pub Tr);

impl Parse for TrBody {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        parse_tr_body(input).map(TrBody)
    }
}

/// Parses one full expression from a token stream (used for attributes).
pub(crate) struct FullExpr(pub Expr);

impl Parse for FullExpr {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let expr = parse_expr(input, true)?;
        if !input.is_empty() {
            return Err(input.error("unexpected tokens after expression"));
        }
        Ok(FullExpr(expr))
    }
}
