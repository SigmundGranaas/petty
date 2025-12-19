//! A simple, JSON-native path and expression engine.
//!
//! This module provides a powerful way to select data from a JSON object and
//! perform transformations using path expressions and custom functions. It is
//! used by the JSON template parser.

pub mod ast;
pub mod engine;
pub mod error;
pub mod functions;
mod parser;

// --- Public API ---
pub use ast::{Expression, PathSegment, Selection};
pub use engine::{EvaluationContext, evaluate, evaluate_as_bool, evaluate_as_string, select};
pub use error::JPathError;
pub use functions::{FunctionRegistry, JPathFunction};
pub use parser::parse_expression;

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::HashMap;

    #[test]
    fn test_parse_and_eval_simple_path() {
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
    fn test_parse_and_eval_path_with_index() {
        let expr = parse_expression("orders[1].id").unwrap();
        let data = json!({ "orders": [ { "id": "A" }, { "id": "B" } ] });
        let vars = HashMap::new();
        let funcs = FunctionRegistry::default();
        let e_ctx = EvaluationContext {
            context_node: &data,
            variables: &vars,
            functions: &funcs,
            loop_position: None,
        };
        let result = evaluate(&expr, &e_ctx).unwrap();
        assert_eq!(result, json!("B"));
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
    fn test_parse_and_eval_nested_function_with_path() {
        let expr = parse_expression("concat('ID: ', upper(customer.orders[0].id))").unwrap();
        let data = json!({ "customer": { "orders": [{ "id": "xn123" }] } });
        let vars = HashMap::new();
        let funcs = FunctionRegistry::default();
        let e_ctx = EvaluationContext {
            context_node: &data,
            variables: &vars,
            functions: &funcs,
            loop_position: None,
        };
        let result = evaluate_as_string(&expr, &e_ctx).unwrap();
        assert_eq!(result, "ID: XN123");
    }

    #[test]
    fn test_current_context_selection() {
        let expr = parse_expression(".").unwrap();
        let data = json!("current value");
        let vars = HashMap::new();
        let funcs = FunctionRegistry::default();
        let e_ctx = EvaluationContext {
            context_node: &data,
            variables: &vars,
            functions: &funcs,
            loop_position: None,
        };
        let result = evaluate(&expr, &e_ctx).unwrap();
        assert_eq!(result, data);
    }
}
