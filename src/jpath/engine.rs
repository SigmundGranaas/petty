// FILE: /home/sigmund/RustroverProjects/petty/src/jpath/engine.rs
//! The evaluation engine for executing a parsed JPath AST.
use super::ast::{Expression, PathSegment, Selection};
use super::functions::FunctionRegistry;
use crate::parser::ParseError;
use serde_json::Value;
use std::collections::HashMap;

/// A container for all state needed during expression evaluation.
#[derive(Clone)]
pub struct EvaluationContext<'a> {
    pub context_node: &'a Value,
    pub variables: &'a HashMap<String, Value>,
    pub functions: &'a FunctionRegistry,
    pub loop_position: Option<usize>,
}

/// Evaluates a compiled expression to a `serde_json::Value`.
pub fn evaluate(expr: &Expression, e_ctx: &EvaluationContext) -> Result<Value, ParseError> {
    match expr {
        Expression::Literal(val) => Ok(val.clone()),
        Expression::Selection(sel) => {
            Ok(select_first(sel, e_ctx.context_node, e_ctx.variables)
                .cloned()
                .unwrap_or(Value::Null))
        }
        Expression::FunctionCall { name, args } => {
            let function = e_ctx
                .functions
                .get(name)
                .ok_or_else(|| ParseError::TemplateRender(format!("Unknown function: {}", name)))?;
            let evaluated_args = args
                .iter()
                .map(|arg| evaluate(arg, e_ctx))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(function(e_ctx, evaluated_args))
        }
    }
}

/// Evaluates an expression and coerces the result to a boolean.
/// "Truthiness" rules: `false`, `null`, `0`, `""`, empty arrays/objects are false.
pub fn evaluate_as_bool(expr: &Expression, e_ctx: &EvaluationContext) -> Result<bool, ParseError> {
    let value = evaluate(expr, e_ctx)?;
    Ok(match value {
        Value::Bool(b) => b,
        Value::Null => false,
        Value::String(s) => !s.is_empty(),
        Value::Number(n) => n.as_f64().unwrap_or(0.0) != 0.0,
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
    })
}

/// Evaluates an expression and coerces the result to a string.
pub fn evaluate_as_string(
    expr: &Expression,
    e_ctx: &EvaluationContext,
) -> Result<String, ParseError> {
    let value = evaluate(expr, e_ctx)?;
    Ok(match value {
        Value::String(s) => s,
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => String::new(),
        _ => serde_json::to_string(&value).unwrap_or_default(),
    })
}

/// Selects values based on a `Selection` path.
pub fn select<'a>(
    sel: &Selection,
    context: &'a Value,
    variables: &'a HashMap<String, Value>,
) -> Vec<&'a Value> {
    match sel {
        Selection::CurrentContext => vec![context],
        Selection::Variable(name) => variables.get(name).map_or(vec![], |v| vec![v]),
        Selection::Path(segments) => {
            let mut current = context;
            for segment in segments {
                let next_val = match segment {
                    PathSegment::Key(k) => current.get(k),
                    PathSegment::Index(i) => current.get(i),
                };
                if let Some(next) = next_val {
                    current = next;
                } else {
                    return vec![]; // Path does not exist
                }
            }
            vec![current] // Return final value
        }
    }
}

/// Selects the first value from a `Selection` path.
pub fn select_first<'a>(
    sel: &Selection,
    context: &'a Value,
    variables: &'a HashMap<String, Value>,
) -> Option<&'a Value> {
    select(sel, context, variables).into_iter().next()
}