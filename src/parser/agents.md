# AI Agent Rules: `parser` Module

This module is responsible for transforming a template (e.g., XSLT, JSON) and a data context into the `idf::IRNode` intermediate representation.

## 1. Purpose & Scope
- **Mission:** To parse a declarative template, apply a JSON data context, and produce a valid `idf::IRNode` tree.
- **Input:** Template string, JSON `Value`.
- **Output:** `Vec<IRNode>`.

## 2. Core Rules
- **Rule 1: Isolate Template Languages:** All logic specific to a single template language must be contained within its own submodule (e.g., `xslt/`, `json/`).
- **Rule 2: Implement the Compile-Execute Pattern:** Each parser must be split into two distinct phases to separate concerns:
    1.  **Compilation Phase (e.g., `compiler.rs`):** This phase is data-agnostic. It parses the template source file into a validated, optimized, executable instruction set or AST. It resolves style names against the stylesheet and checks for syntax errors. This step happens once per template.
    2.  **Execution Phase (e.g., `executor.rs`):** This phase is data-aware. It takes the compiled instructions from the first phase and executes them against a specific JSON data context. It handles data-binding (e.g., Handlebars), control flow (`for-each`, `if`), and builds the final `IRNode` tree.
- **Rule 3: Centralize Style Value Parsing:** All parsing of CSS-like values (e.g., `"10pt"`, `"#FFF"`, `"1pt solid red"`) must be implemented in and delegated to the functions in `src/parser/style.rs`. Do not duplicate this logic.
- **Rule 4: The `TemplateProcessor` Trait:** The `processor.rs` file defines the unified `TemplateProcessor` trait. This is the public interface that the `pipeline` module uses to interact with any parser, ensuring parsers are interchangeable.
- **Rule 5: Error Reporting:** Use the custom `ParseError` enum. For syntax errors in templates, use the `TemplateSyntax` variant and provide accurate line and column information.
