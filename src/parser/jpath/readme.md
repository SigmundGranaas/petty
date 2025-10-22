# JPath Module (`petty::jpath`)

This module provides a simple, JSON-native path language for selecting data from a `serde_json::Value` structure. It serves as a parallel implementation to the `xpath` module, offering a more familiar syntax for developers accustomed to JavaScript or JSON-based systems.

## Overview

JPath is designed for selecting data from JSON objects and arrays using a concise and intuitive syntax. It supports object property access, array indexing, function calls, and literals. It is **not** a full implementation of JSONPath, but a purpose-built utility that borrows common conventions.

## Core Concepts

An expression in JPath can be one of three things: a Literal, a Selection, or a Function Call.

### 1. Selections (JPath Syntax)

Selections are paths used to retrieve data from the current JSON context.

-   **Object Property Access:** Use dot notation to access object properties.
    -   `user.name` -> Selects the `name` property from the `user` object.
-   **Array Index Access:** Use square brackets with a numeric index to access array elements.
    -   `products[0]` -> Selects the first element of the `products` array.
    -   `user.orders[1].item` -> A chained selection on objects and arrays.
-   **Current Context:** Use a single dot `.` to refer to the current node. This is especially useful in loops or when the context itself is a primitive value.
    -   `.`
-   **Variable Access:** Use a `$` prefix to access a variable from the current scope.
    -   `$myVar`

### 2. Literals

JPath supports JSON-like literals directly in expressions, primarily for use as arguments in function calls.

-   **Strings:** Enclosed in single quotes (e.g., `'hello world'`).
-   **Numbers:** Standard integer or floating-point numbers (e.g., `123`, `45.6`).
-   **Booleans:** `true` or `false`.
-   **Null:** `null`.

### 3. Functions

JPath supports a set of built-in functions for data manipulation. The syntax is `functionName(arg1, arg2, ...)`.

**Built-in Functions:**

-   `upper(string)`: Converts a string to uppercase.
-   `lower(string)`: Converts a string to lowercase.
-   `concat(val1, val2, ...)`: Concatenates multiple values into a single string.
-   `contains(haystack, needle)`: Returns `true` if the first string contains the second.
-   `count(array)`: Returns the number of items in an array.
-   `position()`: Returns the 1-based index of the current item in a loop.
-   `equals(val1, val2)`: Returns `true` if the two values are equal (string-based comparison).

## Usage Example

```rust
use petty::jpath::{parse_expression, evaluate, EvaluationContext};
use petty::jpath::functions::FunctionRegistry;
use serde_json::json;
use std::collections::HashMap;

// 1. Parse the expression string once
let expr = parse_expression("concat(upper(customer.name), ' - ID: ', customer.orders[0].id)").unwrap();

// 2. Create the data context
let data = json!({
  "customer": {
    "name": "Acme Corp",
    "orders": [
      { "id": "XN-123", "amount": 99.95 },
      { "id": "AB-456", "amount": 12.50 }
    ]
  }
});

// 3. Create an evaluation context
let vars = HashMap::new();
let funcs = FunctionRegistry::default();
let e_ctx = EvaluationContext {
    context_node: &data,
    variables: &vars,
    functions: &funcs,
    loop_position: None,
};

// 4. Evaluate the expression against the context
let result = evaluate(&expr, &e_ctx).unwrap();

assert_eq!(result, json!("ACME CORP - ID: XN-123"));