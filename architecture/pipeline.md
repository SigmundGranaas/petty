## **Module: `pipeline`**

### 1. Overview

The `pipeline` module is the high-level orchestrator that connects the `parser`, `layout`, and `render` stages into a high-performance, concurrent document generation process. It provides the main public API (`PipelineBuilder`) and manages the execution flow.

### 2. Core Principles & Architecture

The pipeline is built on a decoupled, two-stage **Provider/Renderer** pattern, allowing for flexible and efficient document generation strategies.

1.  **Data Provider Stage (`provider::DataSourceProvider`)**: This stage prepares the data for rendering. It can be a simple pass-through for streaming or a complex analysis pass that consumes the entire data source to generate document-wide metadata.
2.  **Rendering Stage (`renderer::RenderingStrategy`)**: This stage consumes the artifacts from the provider (which could be a data iterator, metadata, or a pre-rendered file) and produces the final document.

This architecture enables two primary execution paths, which are selected automatically based on template features:

-   **Fast Path (Streaming)**: `PassThroughProvider` + `SinglePassStreamingRenderer`. This is a low-memory, high-throughput path for simple templates that don't require forward references (like a table of contents). Data flows directly from the source, through layout workers, to the final output file.
-   **Advanced Path (Metadata/Composition)**: `MetadataGeneratingProvider` + `ComposingRenderer`. This path is used for templates with features like a table of contents, page headers/footers, or page-X-of-Y numbering.
    -   The `Provider` first runs a full layout pass on the data, rendering the main body to a temporary file and simultaneously collecting metadata (headings, anchors, page count) into a `Document` object.
    -   The `Renderer` then takes this `Document` object and the temporary file. It generates new content (like a ToC page) using the metadata, merges it with the body, applies overlays (like headers), and performs "fixups" like adding hyperlink annotations and PDF outlines.

### 3. Public API & Top-Level Definitions

#### **Module: `pipeline::builder` - The Entrypoint**

The `PipelineBuilder` is the sole public entry point for configuring and creating a document generation pipeline.

-   `pub struct PipelineBuilder`: A fluent builder for configuration.
    -   `pub fn new() -> Self`: Creates a new builder with default settings.
    -   `pub fn with_template_file(path) -> Result<Self>`: Loads and compiles a template from a file, inferring the language (JSON/XSLT) from the extension.
    -   `pub fn with_template_source(source, extension) -> Result<Self>`: Loads a template from a string.
    -   `pub fn with_system_fonts(bool) -> Self`: Enables scanning for and loading system-installed fonts.
    -   `pub fn with_font_dir(path) -> Self`: Loads custom fonts from a directory.
    -   `pub fn with_generation_mode(mode: GenerationMode) -> Self`: Allows overriding the automatic pipeline selection.
    -   `pub fn build(self) -> Result<DocumentPipeline>`: Consumes the builder, intelligently selects the appropriate Provider/Renderer pair based on the template's features and the `GenerationMode`, and returns a ready-to-use `DocumentPipeline`.

#### **Module: `pipeline::orchestrator` - The Executor**

-   `pub struct DocumentPipeline`: The configured pipeline object returned by the builder.
    -   `pub async fn generate<W, I>(&self, data_iterator: I, writer: W) -> Result<W>`: The primary method for generating a document. It takes any data iterator and a writer, and executes the configured provider-renderer strategy within a `tokio` task.
    -   `pub fn generate_to_file<P>(&self, data: Vec<Value>, path: P) -> Result<()>`: A convenience method for generating a document from an in-memory dataset to a file.

#### **Module: `pipeline::api` - The Data Contracts**

This module defines the serializable data structures that act as a public contract, especially for templates that need document-wide context (like a table of contents).

-   `pub struct PreparedDataSources`: The handover object between the `Provider` and `Renderer` stages.
    -   `data_iterator: Box<dyn Iterator<Item = Value> + Send>`: The (potentially empty) iterator for the renderer to consume.
    -   `document: Option<Arc<Document>>`: The metadata object produced by the `MetadataGeneratingProvider`.
    -   `body_artifact: Option<Box<dyn ReadSeekSend>>`: The pre-rendered PDF body as a temporary file handle.

-   `pub struct Document`: The top-level metadata object.
    -   `page_count: usize`: Total pages in the main body.
    -   `headings: Vec<Heading>`: List of all headings with their text, level, ID, and page number.
    -   `anchors: Vec<Anchor>`: List of all named anchors with their page and Y-position.
    -   `hyperlinks: Vec<Hyperlink>`: List of all hyperlink locations and their targets.

#### **Module: `pipeline::config` - Configuration Options**

-   `pub enum GenerationMode`:
    -   `Auto` (Default): Automatically selects the Fast Path or Advanced Path based on template features.
    -   `ForceStreaming`: Forces the use of the Fast Path, which will fail if the template uses advanced features.
-   `pub enum PdfBackend`: Selects the underlying PDF library (e.g., `Lopdf`, `PrintPdf`).

---

### 4. Core Implementation Modules

#### **Module: `pipeline::provider` - Data Preparation Stage**

Defines the `DataSourceProvider` trait and its concrete implementations.

-   **Trait `DataSourceProvider`**:
    -   `fn provide(&self, context: &PipelineContext, data_iterator: I) -> Result<PreparedDataSources>`: The core method to prepare data.

-   **`struct PassThroughProvider` (Fast Path)**: A zero-cost implementation that simply boxes the user's data iterator and passes it along in `PreparedDataSources` with `document` and `body_artifact` set to `None`.

-   **`struct MetadataGeneratingProvider` (Advanced Path)**: A complex implementation that consumes the entire `data_iterator`.
    -   It uses the shared `strategy::two_pass` concurrency engine to perform a full layout pass.
    -   The consumer stage renders the PDF pages to a temporary file (`body_artifact`).
    -   Simultaneously, it analyzes the layout results to collect headings, anchors, and page count, which it assembles into a `Document` object.
    -   It returns `PreparedDataSources` with the `document` and `body_artifact` fields populated.

#### **Module: `pipeline::renderer` - Rendering Stage**

Defines the `RenderingStrategy` trait and its concrete implementations.

-   **Trait `RenderingStrategy`**:
    -   `fn render<W>(&self, context: &PipelineContext, sources: PreparedDataSources, writer: W) -> Result<W>`: The core method to render the final document.

-   **`struct SinglePassStreamingRenderer` (Fast Path)**:
    -   Consumes the `data_iterator` from `PreparedDataSources`.
    -   Uses the shared `strategy::two_pass` concurrency engine to perform layout.
    -   The consumer stage renders PDF pages directly to the final output `writer` as they become available, ensuring low memory usage.

-   **`struct ComposingRenderer` (Advanced Path)**:
    -   Expects `PreparedDataSources` to contain a `Document` and a `body_artifact`.
    -   **Composition**: Executes special "role templates" (e.g., a table of contents template), using the `Document` object as its data source. It renders these to new PDF pages and merges them with the `body_artifact`.
    -   **Overlays**: Executes "overlay templates" (e.g., for page headers/footers) for each page, applying their content on top of the existing page content.
    -   **Fixups**: Uses the `Document` metadata to perform a final pass on the merged PDF, adding interactive features like hyperlink annotations and PDF outlines (bookmarks).

#### **Modules: `pipeline::strategy` & `pipeline::worker` - Shared Concurrency Engine**

These modules contain the reusable, high-performance concurrency pattern that powers both the streaming and metadata-generating pipelines.

-   **`strategy::PipelineContext`**: A simple struct holding shared, read-only resources like the compiled template and font manager, passed through all stages.
-   **`strategy::two_pass`**: Implements the producer-worker-consumer pattern.
    -   `producer_task`: Reads from the data iterator and sends work items (data context + sequence index) to a channel.
    -   `spawn_workers`: Spawns a pool of worker threads. Each worker receives a work item, executes the full template-to-layout pipeline for that item, and sends the result (`LaidOutSequence`) to a second channel.
    -   `run_in_order_streaming_consumer`: The final stage. It receives results from workers, which may arrive out of order. It uses a re-ordering buffer to ensure sequences are processed and written to the output stream in their original order. A boolean flag (`perform_analysis`) controls whether it collects metadata or just renders.
-   **`worker`**: Defines the data and logic for a single unit of work.
    -   `struct LaidOutSequence`: The data packet produced by a worker, containing the laid-out pages, discovered anchors, ToC entries, and required resources for a single data item.
    -   `fn finish_layout_and_resource_loading`: The core function executed by a worker. It takes a parsed `IRNode` tree, loads its resources (e.g., images), and runs the layout engine to produce pages of positioned elements.