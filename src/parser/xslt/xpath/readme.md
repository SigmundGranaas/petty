# XPath 1.0 Engine (`parser::xpath`)

This module provides a generic XPath 1.0 processing engine. It is designed to operate on any hierarchical data source that implements the `DataSourceNode` trait, making it completely decoupled from the original input format (e.g., XML, JSON).

## Overview

The engine is built on a standard compiler-like architecture: a string expression is first parsed into an Abstract Syntax Tree (AST), which is then evaluated by an engine against a specific data source context.

### Core Concepts

- **The `DataSourceNode` Trait:** The entire XPath engine is written to work with `DataSourceNode`. This trait provides a generic interface for navigating a tree and querying node properties, allowing the same XPath expression to be executed against an XML document or a virtual JSON document without any changes.

- **Parsing & Evaluation:** The `parser::parse_expression` function consumes a string and produces an `xpath::ast::Expression`. The `engine::evaluate` function then takes this parsed `Expression` and an `EvaluationContext` to produce an `XPathValue` (NodeSet, String, Number, or Boolean).

- **Function Library:** The engine includes a `FunctionRegistry` that holds implementations of the XPath 1.0 core function library.

## Current Status & Future Improvements

### Completed Milestones

The engine has reached a significant level of maturity and compliance with the XPath 1.0 specification. Key completed milestones include:

-   **Full Axis Implementation:** All 11 user-facing axes (`child::`, `descendant::`, `parent::`, `ancestor::`, `following-sibling::`, `preceding-sibling::`, `following::`, `preceding::`, `attribute::`, `self::`, `descendant-or-self::`) are fully implemented and tested.
-   **Comprehensive Function Library:** A large majority of the XPath 1.0 core function library is implemented, covering the most common use cases for node-set, string, boolean, and number manipulation.
-   **Correct Document Order:** The `key()` function and the union (`|`) operator correctly return node-sets that are sorted in document order and free of duplicates, as required by the specification.
-   **Optimized JSON VDOM:** The JSON `DataSourceNode` implementation was refactored to compute `string-value()` on-demand, significantly reducing memory usage and initial parsing time for large documents.

### What Can Be Improved?

The following areas represent the best opportunities for future enhancement, focusing on performance and API robustness.

1.  **Performance:**
  *   **Optimize Key Indexing (Highest Priority):**
    *   **Relevance:** The current key indexing mechanism in `executor.rs` iterates over the entire document for each `<xsl:key>` definition. This is inefficient for stylesheets with multiple keys.
    *   **Action:** Refactor `build_key_indexes` to perform a **single pass** over the document, checking each node against all key patterns simultaneously to populate all indexes at once. This will provide a major performance boost.
  *   **Reduce Cloning:**
    *   **Relevance:** The XPath engine and executor frequently clone `XPathValue` and node-set vectors. While safe, this can create heap allocation pressure.
    *   **Action:** A future performance pass could analyze hot paths and use techniques like `Cow` or more complex lifetime management to reduce allocations.

2.  **API and Feature Enhancements:**
  *   **Explicit Data Source Type:**
    *   **Relevance:** The current `execute` method auto-detects the input format (XML then JSON). This can be brittle.
    *   **Action:** Change the `execute` method to take an enum indicating the source type, e.g., `execute(data: &str, format: DataSourceFormat::Xml)`. This makes the API more robust and predictable.
  *   **Full Attribute Value Template (AVT) Support:**
    *   **Relevance:** AVTs are currently only implemented for attributes on literal result elements. They are also allowed in many XSLT instructions (e.g., `<xsl:attribute name="{$name}">`).
    *   **Action:** Extend the compiler to parse and evaluate AVTs in all places the XSLT specification requires.
  *   **Strict Mode:**
    *   **Relevance:** XSLT 1.0 is lenient by default (e.g., an unknown variable is an empty string). A strict mode that errors on such cases would be a valuable debugging tool.
    *   **Action:** Add a configuration option to the executor to enable strict error checking.

3.  **Refactoring:**
  *   **Consolidate Compiler Handlers:**
    *   **Relevance:** Some compiler handlers are free functions while others are methods on `CompilerBuilder`.
    *   **Action:** Refactor the remaining free-function handlers (e.g., `handle_value_of`) into methods on `CompilerBuilder` for better code consistency and organization.