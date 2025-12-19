# Software Architecture Specification: Petty Document Engine

**Version:** 1.0
**Language:** Rust
**Scope:** Core Library (`src/`)

---

## 1. Executive Summary

**Petty** is a high-performance, concurrent document generation engine designed to transform data (JSON/XML) and templates (XSLT/JSON) into paginated PDF documents.

Its architecture follows a **Compiler-Executor-Renderer** pipeline pattern. It prioritizes low memory usage via streaming processing and high throughput via parallel layout calculation. The system is heavily decoupled: the layout engine knows nothing about PDF, the XSLT engine knows nothing about the output format, and the pipeline orchestrator abstracts the complexity of threading and multi-pass generation.

**Key Architectural Patterns:**
*   **Intermediate Representation (IR):** A unified semantic tree (`IRNode`) acts as the "narrow waist" of the system, decoupling input parsers from the layout engine.
*   **Virtual DOM / Data Abstraction:** The XSLT/XPath engine operates on a generic `DataSourceNode` trait, allowing it to process XML and JSON interchangeably without performance penalties.
*   **Stateless Layout & Rendering:** Layout nodes and rendering commands are designed to be stateless or immutable where possible to facilitate parallel processing across thread pools.
*   **Strategy Pattern:** Used extensively for rendering backends (Streaming vs. Composing) and data providers.

---

## 2. System Architecture

The system is stratified into five logical layers. Data flows sequentially through these layers, often via streaming iterators to minimize resident memory.

### 2.1 Logical Layers

1.  **Input & Compilation Layer (`parser/`, `templating/`)**
  *   **Responsibility:** Loads templates, validates syntax, and compiles them into executable instruction sets.
  *   **Output:** `CompiledTemplate` (Abstract instruction set).

2.  **Orchestration Layer (`pipeline/`)**
  *   **Responsibility:** Configures resources (Fonts, Caches), manages thread pools, and selects the execution strategy (Single-pass Streaming vs. Multi-pass Composition).
  *   **Output:** Coordinated streams of data sent to workers.

3.  **Intermediate Representation Layer (`core/idf/`)**
  *   **Responsibility:** Defines the semantic structure of the document (Blocks, Paragraphs, Tables) without geometric data.
  *   **Output:** `IRNode` Tree.

4.  **Layout Engine Layer (`core/layout/`)**
  *   **Responsibility:** Applies styles, calculates geometry (width/height/x/y), handles text shaping/wrapping, and performs pagination.
  *   **Output:** `PositionedElement` (Geometric primitives).

5.  **Rendering Layer (`render/`)**
  *   **Responsibility:** Serializes geometric primitives into the final output format (PDF), handles PDF object structure (Pages, Resources, XRef).
  *   **Output:** Binary Byte Stream (PDF).

### 2.2 Data Flow Diagram

```text
[Raw Data Stream] --> [Pipeline Provider]
                            |
                            v
[Template Source] --> [Parser/Compiler] --> [Executor (Worker Threads)]
                                                    |
                                          (Produces IRNode Tree)
                                                    |
                                            [Layout Engine]
                                                    |
                                       (Produces PositionedElements)
                                                    |
                                                    v
                                            [Output Renderer] --> [Final PDF]
```

---

## 3. Core Abstractions & Data Models

### 3.1 Interfaces & Traits

*   **`DataSourceNode<'a>` (`parser/xslt/datasource`)**:
  *   **Contract:** Defines a generic, read-only tree navigation interface (parent, children, attributes, node type).
  *   **Role:** Allows the XPath engine to query both XML documents and JSON trees (via a Virtual DOM wrapper) using identical logic.

*   **`LayoutNode` (`core/layout/interface.rs`)**:
  *   **Contract:** `measure(constraints) -> Size` and `layout(context, constraints) -> LayoutResult`.
  *   **Role:** Implemented by every document element (Paragraph, Block, Table). Defines how an element sizes and positions itself and its children.

*   **`DocumentRenderer` (`render/renderer.rs`)**:
  *   **Contract:** Methods for document lifecycle (`begin_document`, `render_page_content`, `finish`).
  *   **Role:** Decouples the layout engine from the specific PDF library (e.g., `lopdf` vs `printpdf`).

*   **`CompiledTemplate` (`parser/processor.rs`)**:
  *   **Contract:** `execute(data_source) -> Result<Vec<IRNode>>`.
  *   **Role:** A thread-safe, executable artifact produced by parsing a template.

### 3.2 Data Structures

*   **`IRNode` (`core/idf/mod.rs`)**:
  *   The **Intermediate Document Format**. An enum representing semantic blocks (e.g., `Block`, `Paragraph`, `Table`, `Image`). It contains metadata (IDs, Styles) but *no* geometry.

*   **`PositionedElement` (`core/layout/elements.rs`)**:
  *   The output of the layout engine. Contains `x`, `y`, `width`, `height`, resolved `ComputedStyle`, and the primitive `LayoutElement` (Text, Rectangle, Image).

*   **`ComputedStyle` (`core/layout/style.rs`)**:
  *   A fully resolved, immutable style struct. It aggregates cascading styles (CSS-like) and handles inheritance. It uses a pre-calculated hash for fast caching.

*   **`PipelineContext` (`pipeline/context.rs`)**:
  *   A shared, reference-counted context containing read-only resources (Fonts, Templates) accessible by all worker threads.

---

## 4. Subsystem Analysis

### 4.1. Core Module (`src/core/`)
**Responsibility:** Defines primitive types, the IDF, and the Layout Engine.
*   **`base/`**: Primitive types (`Color`, `Rect`, `Size`).
*   **`idf/`**: Defines the `IRNode` tree structure.
*   **`style/`**: Defines CSS-like properties (`Border`, `Margins`, `FlexDirection`). Contains the `Stylesheet` definition.
*   **`layout/`**: The heavy lifter.
  *   **`engine.rs`**: Manages caches (fonts, shaping) and drives pagination.
  *   **`nodes/`**: Implementation of `LayoutNode` for every element. `block.rs` handles box-model logic; `text/` handles shaping via `rustybuzz`.
  *   **`algorithms/`**: Specialized solvers for Flexbox (`taffy` integration) and Tables.

### 4.2. Parser Module (`src/parser/`)
**Responsibility:** Transforming templates into executable instructions.
*   **`json/`**: Handles JSON-based templates using a custom JPath engine (`jpath/`).
  *   **`compiler.rs`**: Flattens JSON structure into a linear instruction set.
  *   **`executor.rs`**: A stack machine that executes instructions against data to build `IRNode`s.
*   **`xslt/`**: A fully compliant XSLT 1.0 engine.
  *   **`xpath/`**: A complete XPath 1.0 evaluator working against `DataSourceNode`.
  *   **`compiler/`**: Parses XSLT XML into an AST (`XsltInstruction`).
  *   **`json_ds/`**: A Virtual DOM that presents `serde_json::Value` as an XML tree for XPath queries.

### 4.3. Pipeline Module (`src/pipeline/`)
**Responsibility:** Orchestration, concurrency, and strategy selection.
*   **`builder.rs`**: Fluent API for configuring the system.
*   **`concurrency.rs`**: Manages `tokio` tasks and channels. Implements an **Ordered Streaming Consumer** pattern to allow out-of-order parallel layout while maintaining output order.
*   **`provider/`**:
  *   *Passthrough:* Streams data directly to layout (Fast Path).
  *   *MetadataGenerating:* Performs a "dry run" layout to a temp file to calculate ToC/Index page numbers before the final render.
*   **`renderer/`**:
  *   *Streaming:* Writes pages immediately to the output stream.
  *   *Composing:* Merges a pre-rendered body with dynamically generated headers/footers/ToC.

### 4.4. Render Module (`src/render/`)
**Responsibility:** PDF Serialization.
*   **`lopdf_renderer.rs`**: Translates `PositionedElement`s into PDF commands (`BT`, `Tf`, `Tj`).
*   **`streaming_writer.rs`**: A custom PDF writer that serializes objects immediately (low memory footprint), building the XRef table on the fly.
*   **`composer.rs`**: Post-processing logic to merge PDF documents (e.g., prepending a generated Table of Contents to the main body).

---

## 5. API Specification

### 5.1 Public Entry Point: `PipelineBuilder`

```rust
// src/pipeline/builder.rs

pub struct PipelineBuilder { ... }

impl PipelineBuilder {
    pub fn new() -> Self;
    
    // Configuration
    pub fn with_template_file<P: AsRef<Path>>(mut self, path: P) -> Result<Self, PipelineError>;
    pub fn with_template_source(mut self, source: &str, extension: &str) -> Result<Self, PipelineError>;
    
    // Resources
    pub fn with_system_fonts(mut self, system_fonts: bool) -> Self;
    pub fn with_font_dir<P: AsRef<Path>>(self, path: P) -> Self;
    
    // Tuning
    pub fn with_generation_mode(mut self, mode: GenerationMode) -> Self;
    pub fn with_pdf_backend(mut self, backend: PdfBackend) -> Self;
    
    pub fn build(self) -> Result<DocumentPipeline, PipelineError>;
}
```

### 5.2 Execution: `DocumentPipeline`

```rust
// src/pipeline/orchestrator.rs

pub struct DocumentPipeline { ... }

impl DocumentPipeline {
    /// Streams data through the pipeline, writing the PDF to `writer`.
    pub async fn generate<W, I>(&self, data: I, writer: W) -> Result<W, PipelineError>
    where
        W: io::Write + io::Seek + Send + 'static,
        I: Iterator<Item = serde_json::Value> + Send + 'static;

    /// Helper for file output (blocking).
    pub fn generate_to_file<P, I>(&self, data: I, path: P) -> Result<(), PipelineError>;
}
```

### 5.3 Public Data Models

**`Document` (Metadata)**
Used in multi-pass generation to expose document structure to templates (e.g., for generating a ToC).
```rust
pub struct Document {
    pub page_count: usize,
    pub headings: Vec<Heading>, // { id, level, text, page_number }
    pub anchors: Vec<Anchor>,
    pub index_entries: Vec<IndexEntry>,
    // ...
}
```

---

## 6. Feature Specification

*   **Template Support:**
  *   **XSLT 1.0:** Full support for template matching, flow control, and variables. Supports both XML and JSON input data.
  *   **JSON Templates:** Custom schema with JPath selectors (`{{user.name}}`) and control flow (`#each`, `#if`).
*   **Layout Features:**
  *   **Flow Content:** Paragraphs, Headings, Images.
  *   **Flexbox:** Full implementation via `taffy` (row/column, wrap, align, justify).
  *   **Tables:** Auto-sizing columns, header repetition across pages, row/colspan.
  *   **Pagination:** Automatic page breaks, widow/orphan control, manual page breaks with master page switching.
*   **Text Features:**
  *   TrueType/OpenType font support.
  *   Complex text shaping (ligatures, kerning) via `rustybuzz`.
  *   Text alignment (Left, Right, Center, Justify).
*   **PDF Features:**
  *   Internal Hyperlinks (Anchors).
  *   PDF Outlines (Bookmarks).
  *   Metadata generation (Title, Author).
*   **Performance:**
  *   Parallel layout calculation.
  *   Streaming output (O(1) memory usage relative to page count for simple docs).

---

## 7. Error Handling Strategy

The module uses the `thiserror` crate to define precise error hierarchies. Errors propagate up the stack and are wrapped in higher-level enums.

1.  **Level 1: Specialized Errors**
  *   `ParseError` (in `parser/error.rs`): XML syntax, JSON syntax, XPath grammar errors.
  *   `LayoutError` (in `core/layout/mod.rs`): "Element too large", builder mismatches.
  *   `RenderError` (in `render/renderer.rs`): I/O errors, missing font resources, PDF serialization issues.

2.  **Level 2: Aggregation**
  *   `PipelineError` (in `error.rs`): The top-level error exposed to the user. It wraps the specialized errors.

**Strategy:**
*   **Fail Fast:** Compilation errors (invalid template syntax) are caught during `PipelineBuilder::build`.
*   **Graceful Degradation:** Missing resources (e.g., an image file not found) log a warning but allow the document rendering to continue if possible.
*   **Result Propagation:** All public APIs return `Result<T, PipelineError>`. Panics are strictly avoided in library code.