// FILE: /home/sigmund/RustroverProjects/petty/src/xpath/mod.rs
//! A simplified, JSON-centric XPath-like expression engine.
//!
//! This module provides a powerful way to select data from a JSON object and
//! perform transformations using path expressions and custom functions. It is
//! used by both the XSLT and JSON template parsers to provide a unified
//! data-binding and expression language.

pub mod ast;
pub mod engine;
pub mod functions;
mod parser;

// --- Public API ---
pub use ast::{Expression, Selection};
pub use engine::{evaluate, evaluate_as_bool, evaluate_as_string, select, EvaluationContext};
pub use functions::{FunctionRegistry, XPathFunction};
pub use parser::parse_expression;
use serde_json::Value;

/// Checks if a given JSON node matches an XSLT match pattern.
/// This is a simplified version for JSON, not a full XPath implementation.
/// It is kept for use by the XSLT `match` attribute logic.
pub fn matches(node: &Value, name: Option<&str>, pattern: &str) -> bool {
    match pattern {
        // The wildcard `*` matches any object or array (i.e., any non-primitive value).
        "*" => node.is_object() || node.is_array(),
        // `text()` matches any primitive value that can be represented as text.
        "text()" => node.is_string() || node.is_number() || node.is_boolean(),
        // Otherwise, it's a name test. It matches if the node's name (its key in the parent object)
        // is equal to the pattern. Nodes from an array have no name and cannot match a name test.
        p => name.map_or(false, |n| n == p),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::HashMap;

    #[test]
    fn test_parse_and_eval_path() {
        let expr = parse_expression("customer.name").unwrap();
        let data = json!({ "customer": { "name": "ACME" } });
        let vars = HashMap::new();
        let funcs = FunctionRegistry::default();
        let e_ctx = EvaluationContext {
            context_node: &data,
            variables: &vars,
            functions: &funcs,
            loop_position: None,
        };
        let result = evaluate(&expr, &e_ctx).unwrap();
        assert_eq!(result, json!("ACME"));
    }

    #[test]
    fn test_parse_and_eval_function() {
        let expr = parse_expression("upper('hello')").unwrap();
        let data = json!(null);
        let vars = HashMap::new();
        let funcs = FunctionRegistry::default();
        let e_ctx = EvaluationContext {
            context_node: &data,
            variables: &vars,
            functions: &funcs,
            loop_position: None,
        };
        let result = evaluate(&expr, &e_ctx).unwrap();
        assert_eq!(result, json!("HELLO"));
    }

    #[test]
    fn test_parse_and_eval_nested_function() {
        let expr = parse_expression("concat('User: ', upper(customer.name))").unwrap();
        let data = json!({ "customer": { "name": "acme" } });
        let vars = HashMap::new();
        let funcs = FunctionRegistry::default();
        let e_ctx = EvaluationContext {
            context_node: &data,
            variables: &vars,
            functions: &funcs,
            loop_position: None,
        };
        let result = evaluate_as_string(&expr, &e_ctx).unwrap();
        assert_eq!(result, "User: ACME");
    }

    #[test]
    fn test_evaluate_as_bool() {
        let data = json!({ "show": true, "hide": false, "empty_str": "", "non_empty": "a", "zero": 0, "one": 1, "empty_arr": [], "full_arr": [1] });
        let vars = HashMap::new();
        let funcs = FunctionRegistry::default();
        let e_ctx = EvaluationContext {
            context_node: &data,
            variables: &vars,
            functions: &funcs,
            loop_position: None,
        };

        assert!(evaluate_as_bool(&parse_expression("show").unwrap(), &e_ctx).unwrap());
        assert!(!evaluate_as_bool(&parse_expression("hide").unwrap(), &e_ctx).unwrap());
        assert!(!evaluate_as_bool(&parse_expression("empty_str").unwrap(), &e_ctx).unwrap());
        assert!(evaluate_as_bool(&parse_expression("non_empty").unwrap(), &e_ctx).unwrap());
        assert!(!evaluate_as_bool(&parse_expression("zero").unwrap(), &e_ctx).unwrap());
        assert!(evaluate_as_bool(&parse_expression("one").unwrap(), &e_ctx).unwrap());
        assert!(!evaluate_as_bool(&parse_expression("empty_arr").unwrap(), &e_ctx).unwrap());
        assert!(evaluate_as_bool(&parse_expression("full_arr").unwrap(), &e_ctx).unwrap());
        assert!(!evaluate_as_bool(&parse_expression("non_existent").unwrap(), &e_ctx).unwrap());
    }

    #[test]
    fn test_equals_function() {
        let data = json!({ "status": "active" });
        let vars = HashMap::new();
        let funcs = FunctionRegistry::default();
        let e_ctx = EvaluationContext {
            context_node: &data,
            variables: &vars,
            functions: &funcs,
            loop_position: None,
        };
        let expr = parse_expression("equals(status, 'active')").unwrap();
        assert!(evaluate_as_bool(&expr, &e_ctx).unwrap());
        let expr_false = parse_expression("equals(status, 'inactive')").unwrap();
        assert!(!evaluate_as_bool(&expr_false, &e_ctx).unwrap());
    }
}