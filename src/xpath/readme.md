# XPath-like Module (`petty::xpath`)

This module provides a simplified, XPath-like expression language for selecting data from a `serde_json::Value` structure. It is designed primarily for the XSLT parser to provide a familiar syntax for data selection, conditional logic, and pattern matching within an XML-like template structure.

## Overview

The expression language is a purpose-built utility for JSON that borrows familiar syntax from XPath 1.0 but is **not** a full implementation. It is tailored to the structure of `serde_json::Value`.

## Core Concepts

An expression can be one of three things: a Literal, a Selection, or a Function Call.

### 1. Selections (Path Syntax)

Selections are paths used to retrieve data from the current JSON context.

-   **Object Property Access:** Use dot notation to access object properties. This is translated internally to a JSON Pointer.
    -   `user.name` -> Corresponds to JSON Pointer `/user/name`.
-   **Current Context:** Use a single dot `.` to refer to the current node. This is used extensively in `xsl:apply-templates`.
-   **Variable Access:** Use a `$` prefix to access a variable from the current scope (e.g., from `xsl:with-param`).
    -   `$myVar`

### 2. Literals

The language supports JSON-like literals directly in expressions, primarily for use as arguments in function calls.

-   **Strings:** Enclosed in single quotes (e.g., `'hello world'`).
-   **Numbers:** Standard integer or floating-point numbers (e.g., `123`, `45.6`).
-   **Booleans:** `true` or `false`.
-   **Null:** `null`.

### 3. Functions

A set of built-in functions inspired by XPath/XSLT is provided for data manipulation. The syntax is `functionName(arg1, arg2, ...)`.

**Built-in Functions:**

-   `upper(string)`: Converts a string to uppercase.
-   `lower(string)`: Converts a string to lowercase.
-   `concat(val1, val2, ...)`: Concatenates multiple values into a single string.
-   `contains(haystack, needle)`: Returns `true` if the first string contains the second.
-   `count(array)`: Returns the number of items in an array.
-   `position()`: Returns the 1-based index of the current item in a loop (`xsl:for-each`).
-   `equals(val1, val2)`: Returns `true` if the two values are equal (string-based comparison).

### 4. XSLT Matching

The `xpath::matches` function provides a simplified pattern matching capability for `xsl:template match="..."` attributes.

-   `*`: Matches any JSON object or array.
-   `text()`: Matches any JSON primitive (string, number, boolean).
-   `"name"`: Matches a node if its key in the parent object is `"name"`.

## Usage Example

```rust
use petty::xpath::{parse_expression, evaluate_as_string, EvaluationContext};
use petty::xpath::functions::FunctionRegistry;
use serde_json::json;
use std::collections::HashMap;

// 1. Parse the expression string once
let expr = parse_expression("concat('User: ', upper(customer.name))").unwrap();

// 2. Create the data context
let data = json!({ "customer": { "name": "acme" } });

// 3. Create an evaluation context
let e_ctx = EvaluationContext {
    context_node: &data,
    variables: &HashMap::new(),
    functions: &FunctionRegistry::default(),
    loop_position: None,
};

// 4. Evaluate the expression against the context
let result = evaluate_as_string(&expr, &e_ctx).unwrap();

assert_eq!(result, "User: ACME");
```
