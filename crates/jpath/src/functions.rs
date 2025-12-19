//! Defines the registry and built-in implementations for JPath functions.
use super::engine::EvaluationContext;
use serde_json::{json, Value};
use std::collections::HashMap;

/// The signature for a custom JPath function implementation.
pub type JPathFunction = fn(e_ctx: &EvaluationContext, args: Vec<Value>) -> Value;

/// A registry to hold all available functions for the evaluation engine.
pub struct FunctionRegistry {
    functions: HashMap<String, JPathFunction>,
}

impl FunctionRegistry {
    /// Creates a new, empty function registry.
    pub fn new() -> Self {
        Self { functions: HashMap::new() }
    }

    /// Registers a new function.
    pub fn register(&mut self, name: &str, func: JPathFunction) {
        self.functions.insert(name.to_lowercase(), func);
    }

    /// Finds a function by name (case-insensitive).
    pub fn get(&self, name: &str) -> Option<&JPathFunction> {
        self.functions.get(&name.to_lowercase())
    }
}

// --- Helper for string coercion ---
fn to_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        _ => "".to_string(),
    }
}

// --- Built-in Function Implementations ---

fn upper(_e_ctx: &EvaluationContext, args: Vec<Value>) -> Value {
    args.first().and_then(|v| v.as_str()).map(|s| s.to_uppercase().into()).unwrap_or(Value::Null)
}

fn lower(_e_ctx: &EvaluationContext, args: Vec<Value>) -> Value {
    args.first().and_then(|v| v.as_str()).map(|s| s.to_lowercase().into()).unwrap_or(Value::Null)
}

fn concat(_e_ctx: &EvaluationContext, args: Vec<Value>) -> Value {
    args.iter().map(to_string).collect::<String>().into()
}

fn contains(_e_ctx: &EvaluationContext, args: Vec<Value>) -> Value {
    let haystack = args.first().and_then(|v| v.as_str());
    let needle = args.get(1).and_then(|v| v.as_str());
    match (haystack, needle) {
        (Some(h), Some(n)) => h.contains(n).into(),
        _ => false.into(),
    }
}

fn count(_e_ctx: &EvaluationContext, args: Vec<Value>) -> Value {
    args.first()
        .and_then(|v| v.as_array())
        .map(|arr| arr.len() as f64)
        .map(|len_f64| json!(len_f64))
        .unwrap_or(json!(0.0))
}

fn position(e_ctx: &EvaluationContext, _args: Vec<Value>) -> Value {
    // Position is 1-based for user-facing templates.
    json!(e_ctx.loop_position.unwrap_or(0).saturating_add(1))
}

fn equals(_e_ctx: &EvaluationContext, args: Vec<Value>) -> Value {
    if args.len() != 2 {
        return json!(false);
    }
    // Simple string-based comparison for now
    json!(to_string(&args[0]) == to_string(&args[1]))
}

impl Default for FunctionRegistry {
    /// Creates a new registry populated with all built-in functions.
    fn default() -> Self {
        let mut registry = Self::new();
        registry.register("upper", upper);
        registry.register("lower", lower);
        registry.register("concat", concat);
        registry.register("contains", contains);
        registry.register("count", count);
        registry.register("position", position);
        registry.register("equals", equals);
        registry
    }
}