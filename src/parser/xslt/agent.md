# Module Specification: Petty XSLT Engine

**Version:** 1.0  
**Module Path:** `src/parser/xslt/`  
**Language:** Rust

## 1. Executive Summary

The Petty XSLT Engine is a high-performance, XSLT 1.0 compliant processor designed to transform hierarchical data sources into an **Intermediate Document Format (IDF)**. Its primary architectural goal is **data source agnosticism**; it can process XML or JSON inputs using the same compiled XSLT stylesheets via a unified abstraction layer.

The module utilizes a **Compiler-Executor** architecture:
1.  **Compiler:** Parses XSLT source code into an optimized Abstract Syntax Tree (AST).
2.  **Executor:** Runs the AST against a data source, managing state, context, and variable scoping.
3.  **Output:** Generates an IDF tree (IRNodes) via an abstract builder interface, decoupling the XSLT logic from the specific output format.

---

## 2. System Architecture Layers

The module is organized into four distinct layers:

1.  **Data Abstraction Layer (`datasource/`)**: Defines the contract for read-only tree navigation.
2.  **XPath Engine (`xpath/`)**: A pure functional subsystem for selecting nodes and computing values.
3.  **Compiler (`compiler/`, `parser.rs`)**: Transforms raw XSLT text into executable instructions.
4.  **Runtime (`executor/`, `output.rs`)**: Executes instructions and produces output.

---

## 3. Data Abstraction Layer

This is the foundation of the engine. It allows XPath and XSLT logic to operate independently of the underlying file format.

### Core Trait: `DataSourceNode`
**Location:** `src/parser/xslt/datasource/mod.rs`

Any data format wishing to be processed by this engine must implement this trait. It models the **XPath 1.0 Data Model**.

```rust
pub trait DataSourceNode<'a>: Copy + Clone + PartialEq + Eq + Hash + PartialOrd + Ord {
    fn node_type(&self) -> NodeType;
    fn name(&self) -> Option<QName<'a>>;
    fn string_value(&self) -> String;
    fn attributes(&self) -> Box<dyn Iterator<Item = Self> + 'a>;
    fn children(&self) -> Box<dyn Iterator<Item = Self> + 'a>;
    fn parent(&self) -> Option<Self>;
}
```

### Implementations
1.  **XML (`xml/mod.rs`)**: Wraps the `roxmltree` crate.
    *   Maps XML Elements, Attributes, Text, Comments, and PIs to `DataSourceNode`.
    *   Handles namespaces via `QName`.
2.  **JSON Virtual DOM (`json_ds/mod.rs`)**: Wraps `serde_json::Value`.
    *   Transforms JSON trees into a virtual XML-like structure.
    *   **Mapping Rules:**
        *   JSON Objects become Elements.
        *   JSON keys starting with `@` become Attributes.
        *   JSON primitives become Text nodes inside Elements.
        *   JSON Arrays generate wrapper elements (children are standardized as `<item>`).

---

## 4. Subsystem: XPath 1.0 Engine

**Location:** `src/parser/xslt/xpath/`

A complete XPath 1.0 evaluation engine. It is fully decoupled from XSLT and can be used standalone.

### Features
*   **Parsing:** Uses `nom` to parse XPath string expressions into an Expression AST.
*   **Axes:** Implements all 13 XPath axes (child, parent, ancestor, descendant, following-sibling, etc.).
*   **Types:** Supports `NodeSet`, `String`, `Number` (f64), and `Boolean`.
*   **Functions:** Implements the standard XPath 1.0 function library (`count`, `position`, `last`, `contains`, `substring`, etc.).

### Key APIs

**`parse_expression`**
Compiles a string into an expression tree.
```rust
pub fn parse_expression(input: &str) -> Result<Expression, ParseError>;
```

**`evaluate`**
Evaluates a compiled expression against a context.
```rust
pub fn evaluate<'a, N>(
    expr: &Expression,
    e_ctx: &EvaluationContext<'a, '_, N>,
) -> Result<XPathValue<N>, ExecutionError>;
```

---

## 5. Subsystem: The Compiler

**Location:** `src/parser/xslt/compiler.rs` & `src/parser/xslt/compiler_handlers/`

The compiler converts XSLT XML into an internal executable format (`CompiledStylesheet`). It handles parsing, validation, and optimization (e.g., pre-parsing XPath).

### Compiler Architecture
1.  **Parser Driver (`parser.rs`)**: Uses `quick-xml` to iterate over the XSLT source.
2.  **Builder (`compiler.rs`)**: Maintains a state stack (e.g., inside a template, inside a choose block).
3.  **Handlers (`compiler_handlers/`)**: Specific logic for different XSLT tags.
    *   `control_flow.rs`: `xsl:if`, `xsl:choose`, `xsl:when`.
    *   `loops.rs`: `xsl:for-each`, `xsl:sort`.
    *   `template.rs`: `xsl:template`, `xsl:call-template`.
    *   `variables.rs`: `xsl:variable`, `xsl:param`.
    *   `literals.rs`: Handling literal result elements (HTML/FO tags).

### Output: The AST
**Location:** `src/parser/xslt/ast.rs`

The compiler produces a `CompiledStylesheet` containing:
*   **Template Rules:** A map of templates indexed by mode, sorted by priority.
*   **Named Templates:** For `call-template`.
*   **Keys:** Definitions for `xsl:key`.
*   **Instructions:** The executable body of templates (`XsltInstruction` enum).

**Key AST Enum: `XsltInstruction`**
Includes variants for:
*   `ValueOf`, `CopyOf`
*   `If`, `Choose`
*   `ForEach`, `CallTemplate`, `ApplyTemplates`
*   `Variable`
*   `Element`, `Attribute`, `ContentTag` (Literal elements)

---

## 6. Subsystem: The Executor

**Location:** `src/parser/xslt/executor.rs` & `src/parser/xslt/executor_handlers/`

The runtime engine that processes the `CompiledStylesheet`.

### Core Logic (`TemplateExecutor`)
*   **State Management:** Maintains a stack of variable scopes (`variable_stack`) for local variables and parameters.
*   **Key Indexing:** On initialization, it performs a single pass over the input document to build all `xsl:key` indexes for O(1) lookup.
*   **Template Matching:** Finds the correct template for a node based on `match` patterns and priority.
*   **Dispatch:** Executes instructions via handlers in `executor_handlers/`.

### Execution Flow
1.  **Initialization:** Build key indexes from the `DataSourceNode`.
2.  **Root Template:** Find and execute the template matching `/`.
3.  **Recursion:** Instructions like `apply-templates` trigger recursive template matching and execution.
4.  **Output:** Output commands are sent to the `OutputBuilder`.

### Public API

```rust
pub struct TemplateExecutor<'s, 'a, N: DataSourceNode<'a>> { ... }

impl<'s, 'a, N: DataSourceNode<'a>> TemplateExecutor<'s, 'a, N> {
    // Initialize the executor and build indexes
    pub fn new(stylesheet: &'s CompiledStylesheet, root_node: N, strict: bool) -> Result<Self, ParseError>;

    // Execute and return an IR Tree
    pub fn build_tree(&mut self) -> Result<Vec<IRNode>, ExecutionError>;
}
```

---

## 7. Subsystem: Output Generation

**Location:** `src/parser/xslt/output.rs` & `src/parser/xslt/idf_builder.rs`

To keep the engine generic, it does not produce strings or DOM nodes directly. Instead, it drives an abstract builder.

### Trait: `OutputBuilder`
Defines semantic output actions. This allows the XSLT engine to drive different backends (e.g., PDF generation, HTML generation, layout engine).

```rust
pub trait OutputBuilder {
    fn start_block(&mut self, styles: &PreparsedStyles);
    fn end_block(&mut self);
    fn add_text(&mut self, text: &str);
    fn set_attribute(&mut self, name: &str, value: &str);
    // ... specific methods for Tables, Lists, Images, Links ...
}
```

### Implementation: `IdfBuilder`
Constructs an **Intermediate Document Format (IDF)** tree (`Vec<IRNode>`).
*   Maintains a stack of open nodes (`IRNode`).
*   Handles the conversion of XSL-FO style attributes (e.g., `fo:block`) into IDF nodes (`IRNode::Block`).
*   Manages hierarchy (e.g., ensuring Inline nodes are wrapped in Paragraphs).

---

## 8. Feature Specification

### Supported XSLT 1.0 Features
*   **Templates:** `xsl:template` (match & name), `xsl:apply-templates`, `xsl:call-template`.
*   **Logic:** `xsl:if`, `xsl:choose`, `xsl:when`, `xsl:otherwise`.
*   **Loops:** `xsl:for-each` (with sorting).
*   **Variables:** `xsl:variable`, `xsl:param`, `xsl:with-param` (scoped correctly).
*   **Output:** `xsl:value-of`, `xsl:copy-of`, `xsl:copy`, `xsl:text`.
*   **Creation:** `xsl:element`, `xsl:attribute`, Literal Result Elements.
*   **Indexing:** `xsl:key` and the `key()` function.
*   **Sorting:** `xsl:sort` (text/number, ascending/descending).
*   **AVT:** Attribute Value Templates (e.g., `<div id="{@id}">`) are supported on literal elements and specific instructions.

### Supported XPath 1.0 Features
*   **Axes:** All 13 axes supported.
*   **Predicates:** Filter expressions (e.g., `book[price > 10]`).
*   **Operators:** `+`, `-`, `*`, `div`, `mod`, `=`, `!=`, `<`, `<=`, `>`, `>=`, `and`, `or`, `|` (union).
*   **Functions:** Core library (`string`, `number`, `boolean`, `count`, `sum`, `floor`, `ceiling`, `round`, `concat`, `substring`, `contains`, `starts-with`, `string-length`, `normalize-space`, `translate`, `not`, `true`, `false`, `lang`, `local-name`, `name`, `generate-id`).

### Custom Extensions (Petty Specific)
*   **`petty:role`**: An attribute on `xsl:template` that allows defining templates for specific document roles (e.g., `page-header`, `table-of-contents`) that can be executed independently of the main document flow.
*   **`petty:index`**: A placeholder function detected during compilation to flag templates that require layout-engine indexing.

### XSL-FO / Layout Support
The engine specifically detects XSL-FO tags (and their HTML equivalents) to map them to the IDF structure:
*   `fo:block` / `div` -> `IRNode::Block`
*   `fo:inline` / `span` -> `InlineNode::StyledSpan`
*   `fo:basic-link` / `a` -> `InlineNode::Hyperlink`
*   `fo:table` / `table` -> `IRNode::Table`
*   `fo:external-graphic` / `img` -> `IRNode::Image`

---

## 9. Processing Pipeline (Processor)

**Location:** `src/parser/xslt/processor.rs`

This is the high-level public API used by the rest of the application.

1.  **`XsltParser`**: Implements `TemplateParser`.
    *   Parses the XSLT file.
    *   Identifies "Role Templates" (`petty:role`).
    *   Returns a `TemplateFeatures` struct containing the main template and role templates.

2.  **`XsltTemplate`**: Implements `CompiledTemplate`.
    *   Holds the `CompiledStylesheet`.
    *   **`execute`**:
        1.  Detects input format (XML or JSON).
        2.  Parses input into a `DataSourceNode` tree.
        3.  Initializes `TemplateExecutor`.
        4.  Runs execution.
        5.  Returns `Vec<IRNode>`.

---

## 10. Error Handling

The module uses a hierarchical error model:

1.  **`ParseError`**: Errors during XSLT compilation or Input Parsing (Syntax errors, invalid XML).
2.  **`ExecutionError`**: Runtime errors (XPath evaluation failure, Type errors, Missing templates/variables).
3.  **Strict Mode**: When enabled via configuration, the Executor upgrades certain warnings (like undeclared variables or parameters) into `ExecutionError`s to assist in debugging.