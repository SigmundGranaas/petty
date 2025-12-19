# Software Architecture Specification: `petty::parser::json`

## 1. Executive Summary
The `petty::parser::json` module is a specialized template engine designed to transform JSON-based document templates into a structural Intermediate Representation (`IRNode`) tree. It utilizes a **Compiler-Executor** architectural pattern.

Unlike string-substitution engines (like Handlebars), this module treats the template as a structured object tree. It parses a raw JSON AST, compiles it into an optimized instruction set, and executes it against a dynamic data context using a custom expression language called **JPath**.

**Key Capabilities:**
*   **Structured Parsing:** Relies on `serde_json` for strict schema validation of input templates.
*   **JPath Engine:** A bespoke, `nom`-based expression language for querying JSON data and performing logic (e.g., `upper(user.orders[0].id)`).
*   **Two-Phase Processing:** Separates compilation (validation/optimization) from execution (rendering), allowing templates to be cached and reused with different datasets.

---

## 2. System Architecture

### 2.1 Logical Layers

The module is organized into four distinct processing layers:

1.  **Input Layer (`ast.rs`):** Defines the deserialization schema. It maps raw JSON text into Rust structs (`TemplateNode`).
2.  **Compilation Layer (`compiler.rs`):** Validates the input AST, resolves references (styles, sub-templates), and parses embedded string expressions (`{{...}}`) into executable instructions.
3.  **Runtime Layer (`executor.rs`):** A state machine that iterates through instructions, maintains the context stack, and constructs the output tree.
4.  **Logic Layer (`jpath/`):** A pure functional core that handles data selection, variable resolution, and function execution.

### 2.2 Data Flow

```mermaid
graph TD
    JSON_Source[JSON Template Source] -->|Serde Deserialize| Raw_AST[Raw AST (TemplateNode)]
    Raw_AST -->|Compiler| Instruction_Set[JsonInstruction Vec]
    
    Data[Data Context (JSON)] -->|Input| Executor
    Instruction_Set -->|Input| Executor
    
    Executor -->|Calls| JPath_Engine[JPath Engine]
    JPath_Engine -->|Returns Value| Executor
    
    Executor -->|Constructs| IR_Tree[IRNode Tree]
```

---

## 3. Core Abstractions & Data Models

### 3.1 The Template AST (`ast.rs`)
This is the *Input Model*. It reflects the user-facing JSON structure.
*   **`TemplateNode`**: The root enum. Can be `Static` (content) or `Control` (logic).
*   **`ControlNode`**: Handles flow, specifically `Each` (loops) and `If` (conditionals).
*   **`JsonNode`**: Represents concrete document elements (Paragraphs, Tables, Images).
*   **`StylesheetDef`**: Defines global styles and page layouts extracted during the initial parse.

### 3.2 The Instruction Set (`compiler.rs`)
This is the *Executable Model*. It is flatter and more strict than the AST.
*   **`JsonInstruction`**: An enum representing a single operation the executor must perform. Unlike `TemplateNode`, these instructions contain pre-compiled JPath expressions and validated style references.
*   **`CompiledString`**: An optimization for text content. It splits a string like `"Hello {{name}}"` into static parts (`"Hello "`) and dynamic parts (`Expression::Selection(...)`) to avoid re-parsing at runtime.

### 3.3 The JPath Expression (`jpath/ast.rs`)
*   **`Expression`**: The root of a logic query. Can be a `Literal`, a `Selection` (variable/path lookup), or a `FunctionCall`.
*   **`Selection`**: Represents a specific path into the JSON data (e.g., `.users[0].name`).

---

## 4. Subsystem Analysis

### 4.1 Subsystem: JPath Engine
**Directory:** `jpath/`
**Responsibility:** Provides a query language for extracting data from `serde_json::Value` and performing transformations.

*   **`parser.rs`**: Uses the `nom` parser combinator library to parse string expressions into an `Expression` AST.
*   **`engine.rs`**: The evaluator. It takes an `Expression` and a `EvaluationContext` and returns a `Value`. It handles truthiness coercion (`evaluate_as_bool`) and string coercion (`evaluate_as_string`).
*   **`functions.rs`**: A registry of built-in functions (`upper`, `count`, `concat`). It implements the command pattern to allow dynamic function dispatch.

### 4.2 Subsystem: Compiler
**File:** `compiler.rs`
**Responsibility:** Transforms the `TemplateNode` tree into a `JsonInstruction` vector.

*   **Style Resolution:** It validates that every `style_name` referenced in the JSON actually exists in the `Stylesheet`. If not, it errors immediately (Fail Fast).
*   **Expression Parsing:** It scans text fields for `{{ ... }}` patterns. It invokes the `jpath::parser` to compile these substrings into efficient expressions.
*   **Flattening:** It flattens complex recursive structures where possible to simplify the executor's job.

### 4.3 Subsystem: Executor
**File:** `executor.rs`
**Responsibility:** Builds the output `IRNode` tree.

*   **State Management:**
    *   **`node_stack`**: A stack of block-level elements (e.g., `Block`, `Table`).
    *   **`inline_stack`**: A stack for nested inline elements (e.g., `StyledSpan`, `Hyperlink`).
*   **Context Management:** Manages the "Current Context" (the `.` in JPath) as it iterates through loops (`JsonInstruction::ForEach`).
*   **Isolation:** Uses `sub_executor` instances to build isolated sub-trees (e.g., for Table Cells or containers) before attaching them to the main tree.

### 4.4 Subsystem: Processor (Adapter)
**File:** `processor.rs`
**Responsibility:** Implements the `TemplateParser` trait, serving as the public API adapter for the main application.

*   **Feature Detection:** It scans the AST (`scan_json_node_for_features`) to set flags like `has_table_of_contents` or `uses_index_function`. This allows the core pipeline to optimize later rendering passes.
*   **Role Management:** Splits the input file into the "Main" template and "Role" templates (reusable components).

---

## 5. API Specification

### 5.1 Public Interface (`processor.rs`)

The primary entry point is the `JsonParser` struct implementing `TemplateParser`.

```rust
impl TemplateParser for JsonParser {
    /// Parses a raw JSON string into a compiled template object.
    /// Performs schema validation and compilation.
    fn parse(
        &self,
        source: &str,
        resource_base_path: PathBuf,
    ) -> Result<TemplateFeatures, PipelineError>;
}

impl CompiledTemplate for JsonTemplate {
    /// Executes the compiled instructions against a data source.
    fn execute(
        &self,
        data_source: &str,
        config: ExecutionConfig,
    ) -> Result<Vec<IRNode>, PipelineError>;
}
```

### 5.2 JPath Interface (`jpath/mod.rs`)

Used internally but exposed for testing or standalone expression evaluation.

```rust
/// Parses a string expression (e.g., "user.name") into an AST.
pub fn parse_expression(input: &str) -> Result<Expression, ParseError>;

/// Evaluates a compiled expression against a context.
pub fn evaluate(
    expr: &Expression, 
    e_ctx: &EvaluationContext
) -> Result<Value, ParseError>;
```

---

## 6. Feature Specification

### 6.1 Templating Features
*   **Control Flow:**
    *   `"type": "If"`, `"test": "condition"`: Conditional rendering.
    *   `"type": "Each"`, `"each": "array_path"`: Iteration over arrays.
*   **Dynamic Text:** Use `{{ path }}` or `{{ function(path) }}` inside any string property (content, image source, style names).
*   **Modular Templates:** Define templates in `definitions` and invoke them via `"type": "RenderTemplate"`.

### 6.2 Document Elements
The module supports compiling the following into IR:
*   **Block:** Paragraphs, Lists, FlexContainers, Tables, Headings, Images.
*   **Inline:** StyledSpans, Hyperlinks, InlineImages, PageReferences.
*   **Special:** PageBreaks, TableOfContents markers, IndexMarkers.

### 6.3 JPath Expressions
*   **Literals:** Strings (`'text'`), Numbers, Booleans, Null.
*   **Accessors:** Dot notation (`.key`), Bracket notation (`[0]`), Current Context (`.`).
*   **Built-in Functions:**
    *   String: `upper`, `lower`, `concat`, `contains`.
    *   Logic: `equals`.
    *   Meta: `count` (array length), `position` (loop index).

---

## 7. Error Handling Strategy

The module employs a hierarchical error handling strategy using Rust `Result` types.

1.  **Parse Time (`ParseError`):**
    *   **Schema Violation:** Handled by `serde` (e.g., missing required field).
    *   **JPath Syntax:** Handled by `nom` (e.g., unclosed parenthesis).
    *   **Semantic Error:** Handled by `Compiler` (e.g., referencing a style that does not exist in the stylesheet). *Design Choice: These are fatal errors preventing compilation.*

2.  **Runtime Time (`ParseError` wrapped in `PipelineError`):**
    *   **Missing Data:** Generally handled gracefully (coerced to null/false) by `jpath/engine.rs`, but specific expectations (like `#each` expecting an array) will return an error.
    *   **Missing Template:** Runtime lookup failure for `RenderTemplate` is a fatal error.

3.  **Recovery:**
    *   The `JPath` engine prefers safe defaults (empty strings, false) over panics for type mismatches (e.g., calling `upper` on a number).