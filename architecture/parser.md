## **Module: `parser`**

### 1. Overview

The `parser` module is responsible for transforming a declarative template (in JSON or XSLT format) and a data context (JSON or XML) into the project's canonical `idf::IRNode` intermediate representation. It acts as the bridge between user-defined templates and the layout engine.

### 2. Core Principles & Architecture

-   **Compile-Execute Pattern**: Each parser is split into two distinct phases.
    1.  **Compilation**: A data-agnostic phase that parses a template source file into a validated, optimized, and executable instruction set (e.g., `JsonInstruction` or `XsltInstruction`). This happens once per template.
    2.  **Execution**: A data-aware phase that takes the compiled instructions and executes them against a specific data source, building the final `IRNode` tree.
-   **Unified Interface**: The `processor.rs` module defines the `TemplateParser` and `CompiledTemplate` traits, providing a consistent public interface that allows the rest of the system to treat different template languages interchangeably.
-   **Centralized Styling**: All parsing of CSS-like style values (e.g., `"10pt"`, `"#FFF"`, `"1pt solid red"`) is delegated to the functions in `style.rs` and `style_parsers.rs`, ensuring consistent interpretation across the application.

### 3. Public API & Top-Level Definitions

#### **Module: `parser::processor`**
This module defines the public contract for all parsers.

-   **Trait `TemplateParser`**: The factory for creating compiled templates.
    -   `fn parse(&self, template_source: &str, resource_base_path: PathBuf) -> Result<Arc<dyn CompiledTemplate>, PipelineError>`: Consumes a template string and produces a reusable, compiled artifact.

-   **Trait `CompiledTemplate`**: A reusable, compiled template artifact.
    -   `fn execute(&self, data_source: &str, config: ExecutionConfig) -> Result<Vec<IRNode>, PipelineError>`: Executes the template against a data source to produce the `IRNode` tree.
    -   `fn stylesheet(&self) -> Arc<Stylesheet>`: Returns the associated stylesheet.
    -   `fn resource_base_path(&self) -> &Path`: Returns the base path for resolving relative resource URLs.
    -   `fn features(&self) -> TemplateFeatures`: Reports features detected in the template that require special handling by the layout engine.

-   **Struct `ExecutionConfig`**: Configuration for a single execution run.
    -   `format: DataSourceFormat`: Specifies whether the data source is `Xml` or `Json`.
    -   `strict: bool`: If true, enables strict compliance checks (e.g., error on undeclared variables).

-   **Struct `TemplateFeatures`**: A summary of features found in a template.
    -   `has_table_of_contents: bool`: True if a TOC should be generated.
    -   `has_page_number_placeholders: bool`: True if page number placeholders exist.

#### **Module: `parser::error`**
Defines the unified error handling mechanism for all parsing operations.

-   **Enum `ParseError`**: A comprehensive error enum covering JSON, XML, template syntax, styling, and XPath parsing errors.
-   **Struct `Location`**: Holds `line` and `col` information for precise error reporting.

#### **Module: `parser::style` & `parser::style_parsers`**
This pair of modules handles all CSS-like value parsing. `style_parsers` contains the low-level `nom` combinators, while `style.rs` provides a high-level, ergonomic facade.

-   **`style_parsers.rs`**:
    -   `fn run_parser<T, F>(parser: F, input: &str) -> Result<T, ParseError>`: Executes a `nom` parser on a string, handling whitespace and error conversion.
    -   Contains low-level parsers like `parse_length`, `parse_dimension`, `parse_color`, `parse_border`, and `parse_shorthand_margins`.
-   **`style.rs`**:
    -   `fn apply_style_property(style: &mut ElementStyle, attr_name: &str, value: &str)`: The central dispatcher that applies a single string property (e.g., `font-weight: bold`) to an `ElementStyle` struct.
    -   `fn parse_inline_css(css: &str, style_override: &mut ElementStyle)`: Parses a full inline `style="..."` attribute string.
    -   Contains high-level string-to-enum parsers like `parse_font_weight`, `parse_text_align`, etc.

---

## **Module: `parser::json`**

### 1. Overview
The JSON parser provides a native, straightforward approach for generating documents from JSON data. The template itself is a JSON file that mirrors the desired output structure, enhanced with special keys for control flow and a custom expression language (`JPath`) for data binding.

### 2. Core Components

#### **Module: `parser::json::jpath`**
A simple, JSON-native path and expression engine used for data binding. It is **not** a full implementation of JSONPath but a purpose-built utility.

-   **`jpath::ast`**: Defines the Abstract Syntax Tree for a parsed expression.
    -   `enum Expression`: `Literal(Value)`, `Selection(Selection)`, `FunctionCall { name, args }`.
    -   `enum Selection`: Represents a data path, e.g., `CurrentContext` (`.`), `Variable(String)`, or `Path(Vec<PathSegment>)`.
-   **`jpath::parser`**: A `nom`-based parser.
    -   **Entrypoint**: `pub fn parse_expression(input: &str) -> Result<Expression, ParseError>`.
-   **`jpath::engine`**: The evaluation engine.
    -   **Entrypoint**: `pub fn evaluate(expr: &Expression, e_ctx: &EvaluationContext) -> Result<Value, ParseError>`. Also provides `evaluate_as_bool` and `evaluate_as_string`.
    -   `struct EvaluationContext<'a>`: Contains the `context_node`, `variables`, `functions`, and `loop_position` needed for evaluation.
-   **`jpath::functions`**: Implements the built-in function library.
    -   `struct FunctionRegistry`: Holds all available functions. `FunctionRegistry::default()` creates a registry with all built-ins.
    -   **Functions**: `upper`, `lower`, `concat`, `contains`, `count`, `position`, `equals`.

#### **Module: `parser::json::ast`**
Defines the **input AST** that is deserialized directly from the template JSON file using Serde.

-   `enum TemplateNode`: `Control(ControlNode)` or `Static(JsonNode)`.
-   `enum ControlNode`:
    -   `Each { each, template }`: Iteration. The `each` field is a JPath string.
    -   `If { test, then, else_branch }`: Conditional rendering. The `test` field is a JPath string.
-   `enum JsonNode`: A `#[serde(tag = "type")]` enum representing all possible static output elements like `Paragraph`, `Image`, `Table`, `Text`, `StyledSpan`, etc.
-   `struct JsonTemplateFile`: The top-level structure of a template file, containing `_stylesheet` and `_template` keys.

#### **Module: `parser::json::compiler`**
Implements the **compilation phase**. It transforms the Serde-parsed `json::ast` into a validated, executable instruction set that is data-agnostic.

-   **Entrypoint**: `pub struct Compiler` with `fn compile(&self, root_node: &TemplateNode) -> Result<Vec<JsonInstruction>, ParseError>`.
-   **Output `enum JsonInstruction`**: The compiled, executable instruction set. Variants include `Block`, `Paragraph`, `Text`, `ForEach`, `If`, etc. This is the internal representation used by the executor.
-   **Data Binding**:
    -   `fn parse_expression_string(text: &str) -> Result<CompiledString, ParseError>`: Parses strings containing `{{...}}` into parts.
    -   `enum CompiledString`: Represents a string that can be either `Static` or `Dynamic(Vec<ExpressionPart>)`.
    -   `enum ExpressionPart`: `Static(String)` or `Dynamic(jpath::Expression)`.

#### **Module: `parser::json::executor`**
Implements the **execution phase**. It walks the `JsonInstruction` set, applies a `serde_json::Value` data context, and generates the final `IRNode` tree.

-   **Entrypoint**: `pub struct TemplateExecutor` with `fn build_tree(&mut self, instructions: &[JsonInstruction], context: &Value) -> Result<Vec<IRNode>, ParseError>`.
-   **Functionality**: Traverses the instruction tree, evaluates JPath expressions against the current data context, handles control flow (`If`, `ForEach`), and constructs the `IRNode` tree by pushing nodes onto an internal stack.

#### **Module: `parser::json::processor`**
The public-facing entrypoint that integrates the JSON compiler and executor.

-   **`pub struct JsonParser`**: Implements the `TemplateParser` trait. Its `parse` method reads a JSON template string, creates the `Stylesheet`, compiles the template body using `json::compiler`, and returns a `JsonTemplate`.
-   **`pub struct JsonTemplate`**: Implements the `CompiledTemplate` trait. Its `execute` method takes a JSON data string, creates a `json::executor`, and runs it to produce the `IRNode` tree.

---

## **Module: `parser::xslt`**

### 1. Overview
A powerful, standards-based engine that processes XSLT 1.0 stylesheets. Its key feature is the ability to operate on both **XML** and **JSON** data sources interchangeably via the `DataSourceNode` abstraction.

### 2. Core Components

#### **Module: `parser::xslt::datasource`**
Defines the core abstraction for a navigable, read-only data source tree.

-   **Trait `DataSourceNode<'a>`**: The universal contract for a node in a data source. The entire XPath and XSLT engine is written against this trait.
    -   **Key Methods**: `node_type()`, `name()`, `string_value()`, `attributes()`, `children()`, `parent()`.
-   **`xslt::xml`**: An implementation of `DataSourceNode` for `roxmltree`, providing high-performance XML data source support.
-   **`xslt::json_ds`**: An implementation of `DataSourceNode` for `serde_json::Value`. It transforms the JSON into an in-memory "Virtual DOM" that can be navigated as if it were an XML document, following specific mapping rules (e.g., object keys become element names, array values become `<item>` elements).

#### **Module: `parser::xslt::xpath`**
A generic, standards-compliant XPath 1.0 engine.

-   **`xpath::ast`**: Defines the AST for a parsed XPath expression (`Expression`, `LocationPath`, `Step`, `Axis`, `NodeTest`).
-   **`xpath::parser`**: A `nom`-based parser for the full XPath 1.0 grammar.
    -   **Entrypoint**: `pub fn parse_expression(input: &str) -> Result<Expression, ParseError>`.
-   **`xpath::engine`**: The evaluation engine.
    -   **Entrypoint**: `pub fn evaluate<'a, N>(expr: &Expression, e_ctx: &EvaluationContext<'a, '_, N>) -> Result<XPathValue<N>>`.
    -   `enum XPathValue<N>`: The result of an evaluation (`NodeSet`, `String`, `Number`, `Boolean`).
    -   `struct EvaluationContext<'a, 'd, N>`: Contains all state for evaluation (`context_node`, `variables`, `functions`, `key_indexes`, etc.).
-   **`xpath::functions`**: Implements the XPath 1.0 core function library (`count`, `string`, `key`, `position`, `concat`, etc.).

#### **Module: `parser::xslt::pattern`**
A specialized engine for parsing and evaluating XSLT `match` patterns, which are a subset of XPath.

-   **Entrypoint**: `pub fn parse(text: &str) -> Result<Pattern, ParseError>`.
-   `struct Pattern`: The compiled representation of a match pattern.
    -   `fn matches<'a, N: DataSourceNode<'a>>(&self, node: N, root: N) -> bool`: Evaluates if a given node matches the pattern.

#### **Compilation Pipeline (`parser`, `compiler`, `compiler_handlers`)**
This set of modules implements the **compilation phase** for XSLT.

-   **`parser::xslt::parser`**: A "dumb" XML driver using `quick_xml`. It reads the XSLT source and notifies a `StylesheetBuilder` of events (start element, text, end element).
-   **`parser::xslt::compiler`**: The brain of the compilation.
    -   `pub fn compile(...) -> Result<CompiledStylesheet, ParseError>`: The main entrypoint.
    -   `struct CompilerBuilder`: Implements the `StylesheetBuilder` trait. It is a state machine that listens to the parser driver's events and constructs the final `CompiledStylesheet` AST.
-   **`parser::xslt::compiler_handlers`**: A set of modules containing the logic for how the `CompilerBuilder` should react to specific XSLT tags like `<xsl:if>`, `<xsl:for-each>`, `<xsl:template>`, etc.

#### **Execution Pipeline (`executor`, `executor_handlers`, `output`, `idf_builder`)**
This set of modules implements the **execution phase** for XSLT.

-   **`parser::xslt::output`**:
    -   **Trait `OutputBuilder`**: Decouples the executor from the concrete output format by defining a set of semantic actions (e.g., `start_paragraph`, `add_text`, `end_table`).
-   **`parser::xslt::idf_builder`**:
    -   `struct IdfBuilder`: The concrete implementation of `OutputBuilder` that constructs the final `idf::IRNode` tree.
-   **`parser::xslt::executor`**:
    -   **Entrypoint**: `pub struct TemplateExecutor` with `fn build_tree(&mut self) -> Result<Vec<IRNode>>`.
    -   **Functionality**: A stateful engine that traverses the `DataSourceNode` tree. For each node, it finds the best-matching template rule (`find_matching_template`), and executes its instructions. It manages state like the variable stack and pre-computed key indexes.
-   **`parser::xslt::executor_handlers`**: Contains the logic for executing specific `XsltInstruction` variants, such as handling `<xsl:if>`, `<xsl:value-of>`, `<xsl:apply-templates>`, etc. These handlers interact with the `OutputBuilder` to generate the final tree.

#### **Module: `parser::xslt::processor`**
The public-facing entrypoint that integrates the XSLT compiler and executor.

-   **`pub struct XsltParser`**: Implements `TemplateParser`. Its `parse` method uses `xslt::compiler::compile` to produce a compiled `XsltTemplate`.
-   **`pub struct XsltTemplate`**: Implements `CompiledTemplate`. Its `execute` method inspects the `ExecutionConfig` to determine the data source format (XML or JSON), creates the appropriate `DataSourceNode` tree, instantiates an `xslt::executor`, and runs it to produce the `IRNode` tree.