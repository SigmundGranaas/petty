// FILE: src/parser/xpath/ast.rs
//! Defines the Abstract Syntax Tree (AST) for XPath 1.0 expressions.

/// The top-level expression that can be evaluated.
#[derive(Debug, Clone, PartialEq)]
pub enum Expression {
    Literal(String),
    Number(f64),
    LocationPath(LocationPath),
    Variable(String),
    FunctionCall {
        name: String,
        args: Vec<Expression>,
    },
    BinaryOp {
        left: Box<Expression>,
        op: BinaryOperator,
        right: Box<Expression>,
    },
    // TODO: UnaryOp for negation
}

impl Expression {
    /// Checks if the expression is a `LocationPath` variant.
    pub fn is_location_path(&self) -> bool {
        matches!(self, Expression::LocationPath(_))
    }

    /// Checks if the expression is a `BinaryOp` variant.
    pub fn is_binary_op(&self) -> bool {
        matches!(self, Expression::BinaryOp { .. })
    }
}

/// A binary operator used in an expression.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOperator {
    // Logical
    Or,
    And,
    // Equality
    Equals,
    NotEquals,
    // Relational
    LessThan,
    LessThanOrEqual,
    GreaterThan,
    GreaterThanOrEqual,
    // Additive
    Plus,
    Minus,
    // Multiplicative
    Multiply,
    Divide,
    Modulo,
    // Set
    Union,
}

/// Represents a full location path, like `/child::foo`, `descendant::bar[1]`, or `$var/item`.
#[derive(Debug, Clone, PartialEq)]
pub struct LocationPath {
    /// An optional starting expression, for paths like `$var/foo` or `func()/foo`.
    /// If `None`, the path starts from the context node or root.
    pub start_point: Option<Box<Expression>>,
    /// True if the path starts from the document root (e.g., `/foo`).
    /// Meaningless if `start_point` is `Some`.
    pub is_absolute: bool,
    pub steps: Vec<Step>,
}

/// Represents a single step in a location path, like `child::foo[position() > 1]`.
#[derive(Debug, Clone, PartialEq)]
pub struct Step {
    pub axis: Axis,
    pub node_test: NodeTest,
    pub predicates: Vec<Expression>,
}

/// The axis of movement from the context node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    Child,
    Descendant,
    DescendantOrSelf,
    Attribute,
    Parent,
    Ancestor,
    // TODO: Implement remaining axes
}

/// A test to apply to nodes on a given axis to see if they should be included.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeTest {
    /// A qualified name test (e.g., `foo`, `xsl:if`).
    Name(String),
    /// A wildcard test (`*`).
    Wildcard,
    /// A node type test (e.g., `text()`, `node()`).
    NodeType(NodeTypeTest),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeTypeTest {
    Text,
    Node,
    // TODO: Add `comment()` and `processing-instruction()`
}