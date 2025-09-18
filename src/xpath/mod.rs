// src/xpath/mod.rs
use serde_json::Value;

/// Selects JSON values based on a simple path.
/// This engine supports:
/// 1. Simple Key Lookup (e.g., `name` in the current object)
/// 2. Relative JSON Pointer (e.g., `user/name` from the current object)
/// 3. Absolute JSON Pointer (e.g., `/customers/0` from the root of the provided context)
/// 4. Current Context (e.g., `.` returns the current value itself)
pub fn select<'a>(context: &'a Value, path: &str) -> Vec<&'a Value> {
    if path == "." {
        return vec![context];
    }

    // Unify path handling by always using the JSON pointer mechanism.
    // The pointer is relative to the `context` Value.
    // A path like "user/name" is transformed into "/user/name" for the pointer.
    // A path like "items" is transformed into "/items".
    let pointer_path = if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{}", path)
    };

    context.pointer(&pointer_path).map_or(vec![], |v| vec![v])
}


/// Selects the first matching value and converts it to a `String`.
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
    fn test_select_json_pointer_style_relative_path() {
        let data = get_test_data();
        let result = select(&data, "customers/0/name");
        assert_eq!(result, vec![&json!("Alice")]);
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
        assert_eq!(result, vec![customer_obj]);
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