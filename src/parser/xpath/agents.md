# AI Agent Rules: `xpath` Module

This module provides a generic, standards-compliant XPath 1.0 engine. It is designed to select data from any hierarchical data source that conforms to the `DataSourceNode` trait.

## 1. Purpose & Scope
- **Mission:** To provide a powerful and correct implementation of the XPath 1.0 specification for selecting nodes and computing values from a generic data tree.
- **Decoupling:** The engine **must not** have any knowledge of the underlying data format (XML, JSON, etc.). All operations must be performed via the `DataSourceNode` trait.

## 2. Core Rules
- **Rule 1: Adhere to the XPath 1.0 Data Model:** The engine's behavior must align with the official specification. This includes the definition of node types (root, element, attribute, text), the string-value of a node, and the behavior of axes.
- **Rule 2: Maintain Separation of Concerns:** Keep the module's functionality divided into distinct parts.
  1.  **`ast.rs`:** Defines the Abstract Syntax Tree for a parsed XPath expression. This is the immutable, structural representation.
  2.  **`parser.rs`:** A `nom`-based parser that transforms a string expression into the AST defined in `ast.rs`. It handles the full grammar, including axes, node tests, predicates, and function calls.
  3.  **`engine.rs`:** The evaluation engine. It takes a parsed AST and an `EvaluationContext` (containing the context node, root node, etc.) and executes the expression, returning an `XPathValue`. It is generic over the `DataSourceNode` trait.
  4.  **`functions.rs`:** Contains the implementations for the XPath 1.0 core function library.
- **Rule 3: Compile Expressions (Parse, Don't Re-Parse):** The public API exposes `parse_expression`. This is the "compilation" step. The rest of the system should pass the resulting `Expression` AST around, avoiding the need to re-parse the same string repeatedly, especially in loops.
- **Rule 4: Immutability and Pure Functions:** All evaluation functions (`evaluate`, `evaluate_location_path`, etc.) must be pure. They take immutable references to the `DataSourceNode` and the context and must not modify any state.
- **Rule 5: Correct Axis Handling:** The implementation of each axis (`child`, `parent`, `descendant`, `ancestor`, etc.) in `engine.rs` is critical. It must correctly use the `DataSourceNode` trait's methods (`.children()`, `.parent()`) to navigate the tree.