// FILE: src/parser/xpath/functions.rs
//! Defines the registry and built-in implementations for XPath 1.0 functions.

use super::engine::{EvaluationContext, XPathValue};
use crate::parser::datasource::DataSourceNode;
use crate::parser::ParseError;
use std::collections::HashMap;

// A simple registry that just holds the names of built-in functions.
pub struct FunctionRegistry {
    functions: HashMap<&'static str, ()>,
}

impl FunctionRegistry {
    pub fn new() -> Self {
        Self { functions: HashMap::new() }
    }
    pub fn register(&mut self, name: &'static str) {
        self.functions.insert(name, ());
    }
    pub fn get(&self, name: &str) -> Option<()> {
        self.functions.get(name.to_lowercase().as_str()).copied()
    }
}

/// Dispatches a function call to the correct implementation.
pub fn evaluate_function<'a, 'd, N: DataSourceNode<'a>>(
    name: &str,
    args: Vec<XPathValue<N>>,
    e_ctx: &EvaluationContext<'a, 'd, N>,
) -> Result<XPathValue<N>, ParseError> {
    match name.to_lowercase().as_str() {
        "string" => Ok(func_string(args)),
        "count" => Ok(func_count(args)),
        "position" => Ok(func_position(e_ctx)),
        _ => Err(ParseError::TemplateRender(format!("Unknown XPath function: {}", name))),
    }
}

// --- Built-in Function Implementations ---

fn func_string<'a, N: DataSourceNode<'a>>(args: Vec<XPathValue<N>>) -> XPathValue<N> {
    let val = args.get(0).expect("string() requires at least one argument");
    XPathValue::String(val.to_string())
}

fn func_count<'a, N: DataSourceNode<'a>>(args: Vec<XPathValue<N>>) -> XPathValue<N> {
    let val = args.get(0).expect("count() requires one argument");
    let count = match val {
        XPathValue::NodeSet(nodes) => nodes.len() as f64,
        _ => 0.0,
    };
    XPathValue::Number(count)
}

fn func_position<'a, 'd, N: DataSourceNode<'a>>(e_ctx: &EvaluationContext<'a, 'd, N>) -> XPathValue<N> {
    XPathValue::Number(e_ctx.context_position as f64)
}

impl Default for FunctionRegistry {
    fn default() -> Self {
        let mut registry = Self::new();
        registry.register("string");
        registry.register("count");
        registry.register("position");
        registry
    }
}