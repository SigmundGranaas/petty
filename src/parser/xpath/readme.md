# XPath 1.0 Engine (`parser::xpath`)

This module provides a generic XPath 1.0 processing engine. It is designed to operate on any hierarchical data source that implements the `DataSourceNode` trait, making it completely decoupled from the original input format (e.g., XML, JSON).

## Overview

The engine is built on a standard compiler-like architecture: a string expression is first parsed into an Abstract Syntax Tree (AST), which is then evaluated by an engine against a specific data source context.

## Core Concepts

### 1. The `DataSourceNode` Trait

The entire XPath engine is written to work with `DataSourceNode`. This trait, defined in `parser::datasource`, provides a generic interface for navigating a tree (accessing children, parents, attributes) and querying node properties (name, type, string-value). This allows the same XPath expression to be executed against an XML document or a virtual JSON document without any changes.

### 2. Parsing

The `parser::parse_expression` function consumes a string and produces an `xpath::ast::Expression`. The parser is written with `nom` and supports a growing subset of the XPath 1.0 grammar.

**Supported Syntax (Initial):**
- **Location Paths:** `foo/bar`, `/foo/bar`, `//foo`
- **Axes:** `child::`, `attribute::`, `descendant::`, `descendant-or-self::`, `parent::`, `ancestor::`
- **Abbreviated Syntax:** `foo` (for `child::foo`), `@id` (for `attribute::id`), `//` (for `/descendant-or-self::node()/`)
- **Node Tests:** `*`, `name-test`, `text()`, `node()`
- **Core Functions:** `string()`, `count()`, `position()`

### 3. Evaluation

The `engine::evaluate` function takes a parsed `Expression` and an `EvaluationContext` and returns an `XPathValue`.

- **`EvaluationContext`:** Holds the state for a single evaluation, including:
  - The context node (`N: DataSourceNode`) from which the expression is evaluated.
  - The root node of the entire document.
  - A reference to the function library.
- **`XPathValue`:** An enum representing the four data types in XPath 1.0: `NodeSet`, `String`, `Number`, and `Boolean`.

### 4. Function Library

The engine includes a `FunctionRegistry` that holds implementations of the XPath 1.0 core function library. Functions are implemented as generic closures that operate on `XPathValue` arguments.

## Usage Example

```rust
use petty::parser::datasource::DataSourceNode;
use petty::parser::xml::XmlDocument;
use petty::parser::xpath::{evaluate, parse_expression, EvaluationContext, XPathValue};
use petty::parser::xpath::functions::FunctionRegistry;

// 1. Create a data source
let xml_text = "<doc><item id='a'>Hello</item><item id='b'>World</item></doc>";
let doc = XmlDocument::parse(xml_text).unwrap();
let root_node = doc.root_node(); // This is the document root, parent of <doc>

// 2. Parse the XPath expression
// Selects the string value of the first <item> element in the document
let expr = parse_expression("string(//item)").unwrap();

// 3. Create an evaluation context, using the root node as the initial context
let funcs = FunctionRegistry::default();
let e_ctx = EvaluationContext::new(root_node, root_node, &funcs);

// 4. Evaluate the expression
let result = evaluate(&expr, &e_ctx).unwrap();

if let XPathValue::String(s) = result {
    assert_eq!(s, "Hello"); // string() on a node-set returns the value of the first node
} else {
    panic!("Expected a string result");
}