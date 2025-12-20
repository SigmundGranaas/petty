//! Defines the Abstract Syntax Tree (AST) for JPath expressions.
use serde_json::Value;

/// The top-level representation of a parsed expression.
#[derive(Debug, Clone, PartialEq)]
pub enum Expression {
    /// A literal value, like a string, number, or boolean.
    Literal(Value),
    /// A path to select data from the context.
    Selection(Selection),
    /// A call to a registered function.
    FunctionCall { name: String, args: Vec<Expression> },
}

/// Represents a segment in a JPath selection.
#[derive(Debug, Clone, PartialEq)]
pub enum PathSegment {
    /// An object key (e.g., `.name`).
    Key(String),
    /// An array index (e.g., `[0]`).
    Index(usize),
}

/// Represents a path for selecting data.
#[derive(Debug, Clone, PartialEq)]
pub enum Selection {
    /// Selects the current context node (`.`).
    CurrentContext,
    /// Selects a value from the current variable scope.
    Variable(String),
    /// Selects a node using a sequence of key/index lookups.
    Path(Vec<PathSegment>),
}
