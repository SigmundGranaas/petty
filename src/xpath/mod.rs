// FILE: src/xpath/mod.rs
// src/xpath/mod.rs
use crate::parser::ParseError;
use serde_json::Value;
use std::collections::HashMap;

// --- PRIVATE HELPERS for SELECTION ---

/// Selects JSON values based on a JSON Pointer path relative to the context.
fn select_pointer<'a>(context: &'a Value, path: &str) -> Vec<&'a Value> {
    context.pointer(path).map_or(vec![], |v| vec![v])
}

// --- PUBLIC API for SELECTION ---

/// A pre-compiled representation of an XPath selection path.
#[derive(Debug, Clone, PartialEq)]
pub enum Selection {
    /// Selects the current context node (`.`).
    CurrentContext,
    /// Selects a node using a JSON Pointer path.
    JsonPointer(String),
    /// Selects a value from the current variable scope.
    Variable(String),
}

impl Selection {
    /// Evaluates the compiled selection against a JSON context and variable scope.
    pub fn select<'a>(
        &self,
        context: &'a Value,
        variables: &'a HashMap<String, Value>,
    ) -> Vec<&'a Value> {
        match self {
            Selection::CurrentContext => vec![context],
            Selection::JsonPointer(path) => select_pointer(context, path),
            Selection::Variable(name) => variables.get(name).map_or(vec![], |v| vec![v]),
        }
    }
}

/// Parses a path string into a compiled `Selection`.
pub fn parse_selection(path_str: &str) -> Result<Selection, ParseError> {
    if path_str == "." {
        Ok(Selection::CurrentContext)
    } else if let Some(var_name) = path_str.strip_prefix('$') {
        Ok(Selection::Variable(var_name.to_string()))
    } else {
        // Unify path handling by always using the JSON pointer mechanism.
        // A path like "user/name" is transformed into "/user/name".
        let pointer_path = if path_str.starts_with('/') {
            path_str.to_string()
        } else {
            format!("/{}", path_str.replace('.', "/"))
        };
        Ok(Selection::JsonPointer(pointer_path))
    }
}

/// A helper function to evaluate a selection and return the result as a string.
pub fn select_as_string(
    selection: &Selection,
    context: &Value,
    variables: &HashMap<String, Value>,
) -> String {
    selection
        .select(context, variables)
        .first()
        .map(|v| match v {
            Value::String(s) => s.clone(),
            Value::Number(n) => n.to_string(),
            Value::Bool(b) => b.to_string(),
            _ => String::new(),
        })
        .unwrap_or_default()
}

// --- PUBLIC API for CONDITIONS ---

/// A pre-compiled representation of a boolean XPath expression.
#[derive(Debug, Clone, PartialEq)]
pub enum Condition {
    /// Checks for the existence and "truthiness" of a path.
    Exists(Selection),
    /// Compares the string value of a path to a literal.
    Equals(Selection, Value),
    /// A logical OR between two conditions.
    Or(Box<Condition>, Box<Condition>),
}

impl Condition {
    /// Evaluates the compiled condition against a JSON context.
    pub fn evaluate(&self, context: &Value) -> bool {
        // For now, conditions don't use variables, but this could be extended.
        let variables = HashMap::new();
        match self {
            Condition::Exists(selection) => {
                let results = selection.select(context, &variables);
                !results.is_empty()
                    && results.iter().all(|v| !v.is_null() && v.as_bool() != Some(false))
            }
            Condition::Equals(selection, literal_val) => {
                let selected_value_str = select_as_string(selection, context, &variables);
                let literal_str = match literal_val {
                    Value::String(s) => s.clone(),
                    Value::Number(n) => n.to_string(),
                    Value::Bool(b) => b.to_string(),
                    _ => String::new(),
                };
                selected_value_str == literal_str
            }
            Condition::Or(c1, c2) => c1.evaluate(context) || c2.evaluate(context),
        }
    }
}

/// Parses a boolean expression string into a compiled `Condition`.
pub fn parse_condition(expr: &str) -> Result<Condition, ParseError> {
    // This is a recursive-descent parser. Start with the lowest precedence operator, 'or'.
    parse_or_expr(expr)
}

fn parse_or_expr(expr: &str) -> Result<Condition, ParseError> {
    // Find the last ' or ' to maintain left-to-right evaluation order.
    if let Some(index) = expr.rfind(" or ") {
        let lhs = parse_or_expr(&expr[..index])?;
        let rhs = parse_equality_expr(expr[index + 4..].trim())?;
        return Ok(Condition::Or(Box::new(lhs), Box::new(rhs)));
    }
    parse_equality_expr(expr)
}

fn parse_equality_expr(expr: &str) -> Result<Condition, ParseError> {
    if let Some((lhs_path, rhs_str)) = expr.split_once(" = ") {
        let lhs_path = lhs_path.trim();
        let rhs_str = rhs_str.trim();
        let selection = parse_selection(lhs_path)?;

        let literal_value =
            if let Some(s) = rhs_str.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')) {
                // It's a string literal.
                Value::String(s.to_string())
            } else if let Ok(n) = rhs_str.parse::<f64>() {
                // It's a number.
                Value::Number(serde_json::Number::from_f64(n).unwrap())
            } else if let Ok(b) = rhs_str.parse::<bool>() {
                // It's a boolean.
                Value::Bool(b)
            } else {
                return Err(ParseError::XPathParse(
                    expr.to_string(),
                    format!("Unrecognized literal value: {}", rhs_str),
                ));
            };
        return Ok(Condition::Equals(selection, literal_value));
    }
    // No operator found, treat as a path existence check.
    Ok(Condition::Exists(parse_selection(expr)?))
}

// --- PUBLIC API for MATCHING ---

/// Checks if a given JSON node matches an XSLT match pattern.
/// This is a simplified version for JSON, not a full XPath implementation.
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

    fn get_test_data() -> Value {
        json!({ "name": "Acme", "active": true, "rating": 4.5, "type": "partner" })
    }

    #[test]
    fn test_parse_and_evaluate_selection() {
        let data = get_test_data();
        let vars = HashMap::new();
        let sel = parse_selection("name").unwrap();
        assert_eq!(sel.select(&data, &vars), vec![&json!("Acme")]);
    }

    #[test]
    fn test_parse_and_evaluate_condition() {
        let data = get_test_data();
        // Existence
        assert!(parse_condition("name").unwrap().evaluate(&data));
        assert!(!parse_condition("nonexistent").unwrap().evaluate(&data));
        // Equality
        assert!(parse_condition("type = 'partner'").unwrap().evaluate(&data));
        assert!(parse_condition("rating = 4.5").unwrap().evaluate(&data));
        assert!(!parse_condition("type = 'customer'").unwrap().evaluate(&data));
        // OR logic
        let or_cond = parse_condition("type = 'customer' or rating = 4.5").unwrap();
        assert!(or_cond.evaluate(&data));
    }

    #[test]
    fn test_variable_selection() {
        let data = get_test_data();
        let mut vars = HashMap::new();
        vars.insert("myVar".to_string(), json!("test value"));

        let sel = parse_selection("$myVar").unwrap();
        assert_eq!(select_as_string(&sel, &data, &vars), "test value");
    }

    #[test]
    fn test_matches_function() {
        let data = json!({ "item": { "type": "A" }, "description": "text", "values": [1,2] });
        let item_node = &data["item"];
        let desc_node = &data["description"];
        let values_node = &data["values"];

        // Name tests
        assert!(matches(item_node, Some("item"), "item"));
        assert!(!matches(item_node, Some("item"), "wrong_name"));
        // FIX: The `desc_node`'s real name is "description". The test was incorrectly passing "item".
        assert!(!matches(desc_node, Some("description"), "item")); // Node name is "description", pattern is "item"

        // Wildcard tests
        assert!(matches(item_node, Some("item"), "*"));
        assert!(matches(values_node, Some("values"), "*"));
        assert!(!matches(desc_node, Some("description"), "*")); // Primitives don't match *

        // Text node tests
        assert!(matches(desc_node, Some("description"), "text()"));
        assert!(!matches(item_node, Some("item"), "text()"));
    }
}