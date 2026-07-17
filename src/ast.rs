//! Abstract Syntax Tree for the client hook language.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Program {
    pub body: Vec<Statement>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Statement {
    /// ES6 top-level import: import X from 'm', import { a, b } from 'm', import X, { a } from 'm'
    ImportDecl {
        default_binding: Option<String>,
        named_bindings: Vec<(String, String)>,
        source: String,
    },
    VariableDecl {
        kind: VarKind,
        declarations: Vec<VariableDeclarator>,
    },
    Return { argument: Option<Box<Expr>> },
    Expr { expression: Box<Expr> },
    If {
        test: Box<Expr>,
        consequent: Vec<Statement>,
        alternate: Option<Vec<Statement>>,
    },
    For {
        init: Option<Box<Statement>>,
        test: Option<Box<Expr>>,
        update: Option<Box<Expr>>,
        body: Vec<Statement>,
    },
    /// `for (const x of arr) { ... }` — arrays only (no iterators).
    ForOf {
        kind: VarKind,
        id: String,
        right: Box<Expr>,
        body: Vec<Statement>,
    },
    While {
        test: Box<Expr>,
        body: Vec<Statement>,
    },
    /// Exclusive cases (no fall-through); sugar for if/else chains.
    Switch {
        discriminant: Box<Expr>,
        cases: Vec<SwitchCase>,
    },
    Break,
    Continue,
    Try {
        block: Vec<Statement>,
        handler: Option<CatchClause>,
        finalizer: Option<Vec<Statement>>,
    },
    FunctionDecl {
        id: Option<String>,
        params: Vec<Param>,
        body: Vec<Statement>,
        r#async: bool,
    },
    ExportDefault { declaration: Box<ExportDefaultDecl> },
    ExportNamed { declaration: ExportNamedDecl },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SwitchCase {
    /// `None` = `default` clause.
    pub test: Option<Box<Expr>>,
    pub consequent: Vec<Statement>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum VarKind {
    Const,
    Let,
    Var,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VariableDeclarator {
    pub id: Pattern,
    pub init: Option<Box<Expr>>,
}

/// Optional type annotation for variables and params (e.g. "number", "string").
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TypeAnnotation {
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Pattern {
    Identifier {
        name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        type_annotation: Option<TypeAnnotation>,
    },
    Array { elements: Vec<Option<Pattern>> },
    Object { properties: Vec<ObjectProp> },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObjectProp {
    pub key: String,
    pub value: Option<Box<Pattern>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Param {
    pub name: String,
    pub rest: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<Box<Expr>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub type_annotation: Option<TypeAnnotation>,
}

/// Element of an array literal: expression or `...spread`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ArrayElement {
    Expr(Expr),
    Spread(Box<Expr>),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ExportDefaultDecl {
    Function { id: Option<String>, params: Vec<Param>, body: Vec<Statement>, r#async: bool },
    Expr { expression: Box<Expr> },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ExportNamedDecl {
    Function { id: String, params: Vec<Param>, body: Vec<Statement> },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CatchClause {
    pub param: Option<String>,
    pub body: Vec<Statement>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Expr {
    Literal(Literal),
    Identifier { name: String },
    Binary {
        left: Box<Expr>,
        op: BinaryOp,
        right: Box<Expr>,
    },
    Unary { op: UnaryOp, argument: Box<Expr> },
    Call {
        callee: Box<Expr>,
        arguments: Vec<Expr>,
        #[serde(default)]
        optional: bool,
    },
    Member {
        object: Box<Expr>,
        property: Box<Expr>,
        optional: bool,
    },
    ArrowFunction {
        params: Vec<Param>,
        body: ArrowBody,
    },
    Function {
        id: Option<String>,
        params: Vec<Param>,
        body: Vec<Statement>,
        r#async: bool,
    },
    Conditional {
        test: Box<Expr>,
        consequent: Box<Expr>,
        alternate: Box<Expr>,
    },
    Array { elements: Vec<ArrayElement> },
    Object { properties: Vec<ObjectExprProp> },
    TemplateLiteral {
        quasis: Vec<String>,
        expressions: Vec<Expr>,
    },
    Import { source: Box<Expr> },
    Await { argument: Box<Expr> },
    PostfixIncr { argument: Box<Expr> },
    PostfixDecr { argument: Box<Expr> },
    PrefixIncr { argument: Box<Expr> },
    PrefixDecr { argument: Box<Expr> },
    Assign {
        left: Box<Expr>,
        right: Box<Expr>,
    },
    JsxElement(Box<JsxElement>),
    JsxFragment { children: Vec<JsxChild> },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Literal {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    StrictEq,
    Neq,
    StrictNeq,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
    NullishCoalesce, // ??
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnaryOp {
    Not,
    Neg,
    Typeof,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ArrowBody {
    Block(Vec<Statement>),
    Expr(Box<Expr>),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ObjectExprProp {
    KeyValue { key: String, value: Box<Expr> },
    Spread { argument: Box<Expr> },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JsxElement {
    pub tag: JsxTag,
    pub props: Vec<JsxProp>,
    pub children: Vec<JsxChild>,
    pub self_closing: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum JsxTag {
    Identifier { name: String },
    Member { object: String, property: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum JsxProp {
    Attribute { name: String, value: JsxAttrValue },
    Spread { argument: Box<Expr> },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum JsxAttrValue {
    String(String),
    Expr(Box<Expr>),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum JsxChild {
    Text(String),
    Expr(Box<Expr>),
    Element(Box<JsxElement>),
    Fragment { children: Vec<JsxChild> },
}
