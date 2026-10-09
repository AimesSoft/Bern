//! Syntax tree for Bern declarations. Spans are kept so diagnostics from
//! both the macros and the build tool point at the developer's source.

use proc_macro2::Span;
use syn::Ident;

/// One translation inside `tr!`.
#[derive(Clone, Debug)]
pub struct TrEntry {
    pub language: String,
    pub text: String,
    pub span: Span,
}

/// An interpolation argument after the `;` in `tr!`.
#[derive(Clone, Debug)]
pub struct TrArg {
    pub name: Ident,
    pub value: Expr,
}

/// `tr! { "zh-Hans" => "…{count}…", "en" => "…"; count: app.count }`
#[derive(Clone, Debug)]
pub struct Tr {
    pub entries: Vec<TrEntry>,
    pub args: Vec<TrArg>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Literal {
    Text(String),
    Int(i64),
    Float(f64),
    Bool(bool),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnaryOp {
    Not,
    Neg,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinaryOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
}

impl BinaryOp {
    pub fn symbol(self) -> &'static str {
        match self {
            BinaryOp::Eq => "==",
            BinaryOp::Ne => "!=",
            BinaryOp::Lt => "<",
            BinaryOp::Le => "<=",
            BinaryOp::Gt => ">",
            BinaryOp::Ge => ">=",
            BinaryOp::And => "&&",
            BinaryOp::Or => "||",
        }
    }
}

/// `a`, `a.b` or `a.b.c`. Roots are component parameters or template
/// variables; a lone identifier may also be a keyword such as `segmented`.
#[derive(Clone, Debug)]
pub struct PathExpr {
    pub segments: Vec<Ident>,
}

impl PathExpr {
    pub fn span(&self) -> Span {
        self.segments[0].span()
    }

    pub fn dotted(&self) -> String {
        self.segments
            .iter()
            .map(Ident::to_string)
            .collect::<Vec<_>>()
            .join(".")
    }

    pub fn single(&self) -> Option<&Ident> {
        (self.segments.len() == 1).then(|| &self.segments[0])
    }
}

/// `Type::Variant` or `Type::Variant { field: expr }`.
#[derive(Clone, Debug)]
pub struct VariantExpr {
    pub ty: Ident,
    pub variant: Ident,
    pub fields: Vec<(Ident, Expr)>,
    pub has_braces: bool,
}

/// Restricted expressions: literals, translations, typed bindings, enum
/// values and a fixed set of operators. Arbitrary function calls do not
/// exist in this grammar.
#[derive(Clone, Debug)]
pub enum Expr {
    Literal {
        value: Literal,
        span: Span,
    },
    Tr(Tr),
    Path(PathExpr),
    Variant(VariantExpr),
    Unary {
        op: UnaryOp,
        expr: Box<Expr>,
        span: Span,
    },
    Binary {
        op: BinaryOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
        span: Span,
    },
}

impl Expr {
    pub fn span(&self) -> Span {
        match self {
            Expr::Literal { span, .. } | Expr::Unary { span, .. } | Expr::Binary { span, .. } => {
                *span
            }
            Expr::Tr(tr) => tr.span,
            Expr::Path(path) => path.span(),
            Expr::Variant(variant) => variant.ty.span(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Prop {
    pub name: Ident,
    pub value: Expr,
}

/// `receiver.member.method(args)` after `=>`.
#[derive(Clone, Debug)]
pub struct Action {
    pub receiver: Vec<Ident>,
    pub method: Ident,
    pub args: Vec<Expr>,
    pub span: Span,
}

impl Action {
    pub fn receiver_path(&self) -> String {
        self.receiver
            .iter()
            .map(Ident::to_string)
            .collect::<Vec<_>>()
            .join(".")
    }
}

/// A built-in component such as `Button(...) { ... } => action`.
#[derive(Clone, Debug)]
pub struct Element {
    pub name: Ident,
    pub props: Vec<Prop>,
    /// Row template variable, as in `List(...) |item| { ... }`.
    pub template: Option<Ident>,
    pub children: Option<Vec<Node>>,
    pub action: Option<Action>,
    pub span: Span,
}

impl Element {
    pub fn prop(&self, name: &str) -> Option<&Prop> {
        self.props.iter().find(|prop| prop.name == name)
    }
}

/// A reference to a declared Bern component with named arguments.
#[derive(Clone, Debug)]
pub struct ComponentCall {
    pub path: Vec<Ident>,
    pub args: Vec<Prop>,
    pub span: Span,
}

impl ComponentCall {
    pub fn name(&self) -> &Ident {
        self.path.last().expect("non-empty path")
    }

    pub fn path_string(&self) -> String {
        self.path
            .iter()
            .map(Ident::to_string)
            .collect::<Vec<_>>()
            .join("::")
    }
}

#[derive(Clone, Debug)]
pub struct IfNode {
    pub branches: Vec<(Expr, Vec<Node>)>,
    pub otherwise: Option<Vec<Node>>,
    pub span: Span,
}

/// `if let Some(name) = expr { … } else { … }`
#[derive(Clone, Debug)]
pub struct IfLetNode {
    pub binding: Ident,
    pub value: Expr,
    pub then: Vec<Node>,
    pub otherwise: Option<Vec<Node>>,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum Pattern {
    Variant {
        ty: Ident,
        variant: Ident,
        bindings: Vec<Ident>,
    },
    Wildcard(Span),
}

#[derive(Clone, Debug)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub body: Vec<Node>,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct MatchNode {
    pub value: Expr,
    pub arms: Vec<MatchArm>,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum Node {
    Element(Element),
    Component(ComponentCall),
    If(IfNode),
    IfLet(IfLetNode),
    Match(MatchNode),
}

impl Node {
    pub fn span(&self) -> Span {
        match self {
            Node::Element(element) => element.span,
            Node::Component(call) => call.span,
            Node::If(node) => node.span,
            Node::IfLet(node) => node.span,
            Node::Match(node) => node.span,
        }
    }
}

/// The contents of one `ui!` invocation.
#[derive(Clone, Debug)]
pub struct UiBody {
    pub nodes: Vec<Node>,
}

/// Bern's view of a Rust type used in declarations.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TypeRef {
    Unit,
    Bool,
    Int,
    Float,
    Text,
    /// A declared enum, record or handle type; the path is kept for
    /// resolution and the last segment names the type.
    Named(Vec<String>),
    Optional(Box<TypeRef>),
    List(Box<TypeRef>),
}

impl TypeRef {
    pub fn name(&self) -> Option<&str> {
        match self {
            TypeRef::Named(path) => path.last().map(String::as_str),
            _ => None,
        }
    }
}
