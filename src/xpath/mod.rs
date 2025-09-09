use serde_json::Value;

/// Parses a very simple equality expression like `key = 'value'`.
fn parse_simple_equality(expression: &str) -> Option<(&str, &str)> {
    let parts: Vec<&str> = expression.split('=').map(|s| s.trim()).collect();
    if parts.len() == 2 {
        let key = parts[0];
        let value_part = parts[1];
        if value_part.starts_with('\'') && value_part.ends_with('\'') {
            let value = &value_part[1..value_part.len() - 1];
            return Some((key, value));
        }
    }
    None
}

/// Selects JSON values based on a simple path that is compatible with JSON Pointer.
///
/// # Arguments
/// * `context` - The `serde_json::Value` to search within.
/// * `path` - A JSON Pointer string (e.g., `/customers/0/name`). A path of `.` returns the context itself.
///
/// # Returns
/// A `Vec` of references to the matched `Value`s. Returns an empty `Vec` if not found.
pub fn select<'a>(context: &'a Value, path: &str) -> Vec<&'a Value> {
    // Handle simple equality check for `xsl:if`
    if let Some((key, expected_val_str)) = parse_simple_equality(path) {
        if let Some(actual_val) = context.get(key) {
            if let Some(actual_val_str) = actual_val.as_str() {
                if actual_val_str == expected_val_str {
                    return vec![actual_val]; // Condition met
                }
            }
        }
        return vec![]; // Condition not met
    }

    if path == "." {
        return vec![context];
    }

    // If it's a JSON pointer path (starts with /), use it directly.
    if path.starts_with('/') {
        return context.pointer(path).map_or(vec![], |v| vec![v]);
    }

    // Otherwise, treat it as a simple relative key lookup from the current context.
    // This is a simplification of XPath, supporting `select="key"`.
    if let Some(obj) = context.as_object() {
        if let Some(value) = obj.get(path) {
            return vec![value];
        }
    }
    vec![]
}

/// Selects the first matching value and converts it to a `String`.
///
/// # Arguments
/// * `context` - The `serde_json::Value` to search within.
/// * `path` - A JSON Pointer string.
///
/// # Returns
/// A `String` representation of the found value, or an empty string if not found or not a primitive.
pub fn select_as_string(context: &Value, path: &str) -> String {
    select(context, path)
        .first()
        .map(|v| match v {
            Value::String(s) => s.clone(),
            Value::Number(n) => n.to_string(),
            Value::Bool(b) => b.to_string(),
            _ => String::new(),
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn get_test_data() -> Value {
        json!({
            "name": "Acme Corp",
            "active": true,
            "rating": 4.5,
            "customers": [
                { "id": "A", "name": "Alice" },
                { "id": "B", "name": "Bob" }
            ],
            "metadata": null
        })
    }

    #[test]
    fn test_select_equality_pass() {
        let data = get_test_data();
        let result = select(&data, "name = 'Acme Corp'");
        assert_eq!(result, vec![&json!("Acme Corp")]);
    }

    #[test]
    fn test_select_equality_fail() {
        let data = get_test_data();
        let result = select(&data, "name = 'Globex'");
        assert!(result.is_empty());
    }

    #[test]
    fn test_select_relative_key() {
        let data = get_test_data();
        let result = select(&data, "name");
        assert_eq!(result, vec![&json!("Acme Corp")]);
    }

    #[test]
    fn test_select_root_string() {
        let data = get_test_data();
        let result = select(&data, "/name");
        assert_eq!(result, vec![&json!("Acme Corp")]);
    }

    #[test]
    fn test_select_root_number() {
        let data = get_test_data();
        let result = select(&data, "/rating");
        assert_eq!(result, vec![&json!(4.5)]);
    }

    #[test]
    fn test_select_nested_value() {
        let data = get_test_data();
        let result = select(&data, "/customers/1/name");
        assert_eq!(result, vec![&json!("Bob")]);
    }

    #[test]
    fn test_select_entire_object() {
        let data = get_test_data();
        let result = select(&data, "/customers/0");
        assert_eq!(result, vec![&json!({ "id": "A", "name": "Alice" })]);
    }

    #[test]
    fn test_select_entire_array() {
        let data = get_test_data();
        let result = select(&data, "/customers");
        assert_eq!(result.len(), 1);
        assert!(result[0].is_array());
    }

    #[test]
    fn test_select_context_itself() {
        let data = get_test_data();
        let customer_obj = &data["customers"][0];
        let result = select(customer_obj, ".");
        assert_eq!(result, vec![&customer_obj]);
    }

    #[test]
    fn test_select_not_found() {
        let data = get_test_data();
        let result = select(&data, "/customers/2/name");
        assert!(result.is_empty());
        let result2 = select(&data, "/nonexistent");
        assert!(result2.is_empty());
    }

    #[test]
    fn test_select_as_string_works() {
        let data = get_test_data();
        assert_eq!(select_as_string(&data, "/name"), "Acme Corp");
        assert_eq!(select_as_string(&data, "/rating"), "4.5");
        assert_eq!(select_as_string(&data, "/active"), "true");
    }

    #[test]
    fn test_select_as_string_not_found_or_complex() {
        let data = get_test_data();
        assert_eq!(select_as_string(&data, "/nonexistent"), "");
        assert_eq!(select_as_string(&data, "/customers"), ""); // array
        assert_eq!(select_as_string(&data, "/metadata"), ""); // null
    }
}