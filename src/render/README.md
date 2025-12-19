# Software Architecture Specification: `petty::render` Module

## 1. Executive Summary

The `petty::render` module acts as the final stage of the document generation pipeline. Its primary responsibility is to consume the geometry-aware layout tree (`PositionedElement`s) produced by the `core::layout` module and serialize it into a final document format, specifically PDF.

Architecturally, this module is designed around a **Driver-Backend** pattern. It defines a unified interface (`DocumentRenderer`) that allows the orchestration layer to drive document generation without knowledge of the underlying PDF library. The module currently provides two distinct implementations:
1.  **Legacy/High-Level Backend (`pdf.rs`):** Built on `printpdf`, offering a simpler object model.
2.  **High-Performance/Streaming Backend (`lopdf_renderer.rs`):** Built on `lopdf` and a custom `StreamingPdfWriter`, enabling low-memory generation of large documents and direct control over PDF structures (Cross-Reference tables, Object Streams).

A key architectural feature is the **Stateless Drawing** capability, which decouples drawing logic from document state, enabling parallel rendering of page content before sequential serialization.

## 2. System Architecture

The module is stratified into four logical layers:

1.  **Abstraction Layer (`renderer.rs`)**:
  *   Defines the contract (`DocumentRenderer` trait) that all backends must implement.
  *   Defines standardized error types (`RenderError`) and metadata structures (`Pass1Result`).

2.  **Drawing Layer (`drawing/`)**:
  *   **Input:** `PositionedElement` (Layout).
  *   **Output:** PDF Operators (Draw Text, Draw Rect, etc.).
  *   **Responsibility:** Translates internal geometry and style representations into primitive graphics commands. Crucially, this layer contains logic for both stateful (sequential) and stateless (parallel) execution.

3.  **Backend Implementation Layer**:
  *   **`LopdfRenderer`:** Manages the lifecycle of a generic PDF structure, handles resource mapping (Fonts/Images), and integrates with the streaming writer.
  *   **`PdfDocumentRenderer`:** Wraps the `printpdf` library to manage high-level PDF concepts (Layers, Pages).

4.  **Serialization & Composition Layer**:
  *   **`StreamingPdfWriter` (`streaming_writer.rs`)**: A custom serializer that writes PDF objects to the I/O stream immediately upon generation, managing the byte offsets and XRef table manually.
  *   **`Composer` (`composer.rs`)**: Provides post-processing capabilities, such as merging multiple PDF documents and overlaying content (watermarks, headers/footers) by manipulating the raw PDF Object Model (DOM).

### Data Flow Diagram

```mermaid
graph TD
    Layout[Layout Engine] -->|Vec PositionedElement| Interface(DocumentRenderer Trait)
    
    subgraph "Render Module"
        Interface -->|Dispatch| Backend{Backend Strategy}
        
        Backend -- Strategy A --> PrintPDF[PdfDocumentRenderer]
        Backend -- Strategy B --> LoPDF[LopdfRenderer]
        
        subgraph "Drawing Layer"
            PrintPDF --> Draw[drawing/mod.rs]
            LoPDF --> Draw
            Draw -->|Stateless Call| Primitives[Text/Rect/Image]
            Primitives -->|PDF Ops| OpBuffer[Operation Buffer]
        end
        
        LoPDF -->|Buffer Objects| Stream[StreamingPdfWriter]
        Stream -->|Write Bytes| IO[Output Stream (File/Stdout)]
        
        PrintPDF -->|Save| IO
    end
    
    subgraph "Post-Processing"
        IO -.->|Read Back| Composer[Composer]
        Composer -->|Merge/Overlay| FinalPDF[Final PDF]
    end
```

## 3. Core Abstractions & Data Models

### Key Traits

*   **`DocumentRenderer` (`renderer.rs`)**
  *   **Contract:** The lifecycle interface for creating a document.
  *   **Key Methods:**
    *   `begin_document(writer)`: Initializes headers.
    *   `add_resources(resources)`: Pre-loads global assets (images/fonts).
    *   `render_page_content(...) -> ObjectId`: Generates the content stream for a page but does not finalize the page node (allows for reuse).
    *   `write_page_object(...)`: Links content streams and annotations into a Page dictionary.
    *   `finish(...)`: Writes the Trailer, XRef, and EOF.

### Key Structs

*   **`Pass1Result` (`renderer.rs`)**
  *   **Role:** Container for metadata collected during a "dry run" or analysis pass.
  *   **Contents:** `resolved_anchors` (link targets), `toc_entries` (table of contents), `hyperlink_locations`.

*   **`StreamingPdfWriter` (`streaming_writer.rs`)**
  *   **Role:** Low-level PDF serializer.
  *   **Mechanism:** Tracks byte offsets of every object written to the stream. Builds the Cross-Reference (XRef) table in memory and writes it at the end of the file. This avoids holding the entire PDF DOM in RAM.

*   **`RenderContext` (`pdf.rs`)**
  *   **Role:** A read-only context passed to stateless drawing functions.
  *   **Contents:** Mappings for Fonts and XObjects (images) to their PDF Resource IDs, plus page dimensions.

## 4. Subsystem Analysis

### 4.1. Drawing Subsystem
**Directory:** `src/render/drawing/`
**Responsibility:** Converting `LayoutElement` enums into PDF drawing operations.

*   **`mod.rs`**: The entry point. Dispatches to specific modules based on element type.
*   **`text.rs`**: Handles text rendering. Manages PDF text sections (`BT`/`ET`), font selection, font sizing, and text positioning matrices (`Tm`).
*   **`rect.rs`**: Handles vector graphics. Used for drawing background colors and borders.
*   **`image.rs`**: Handles raster images. Relies on "XObjects". It does not embed image data directly into the content stream; instead, it references an external Resource ID (`/Im1 Do`).
*   **Stateless vs. Stateful:**
  *   *Stateful:* `draw_element` operates on `PageRenderer`, mutating the current state (tracking open text sections, current color).
  *   *Stateless:* `draw_element_stateless` takes a `Vec<Op>` buffer and a `RenderContext`. It is pure logic, allowing it to be run in parallel worker threads without locking a central renderer instance.

### 4.2. LoPDF / Streaming Subsystem
**Files:** `lopdf_renderer.rs`, `streaming_writer.rs`, `lopdf_helpers.rs`
**Responsibility:** High-performance PDF generation.

*   **`LopdfRenderer`**: The bridge between the layout engine and the writer. It maps the engine's font definitions to PDF font dictionaries.
*   **`StreamingPdfWriter`**:
  *   Implements a custom writer for `lopdf` objects.
  *   **Optimization:** It creates a "skeleton" PDF. Page content is streamed immediately. The Page Tree (`/Pages` -> `/Kids`) is constructed internally and written only at the end.
*   **`lopdf_helpers.rs`**: Contains complex logic detached from the renderer instance, such as:
  *   `create_link_annotations`: Generates clickable regions based on layout geometry.
  *   `build_outlines`: Recursively constructs the PDF Bookmark tree from ToC entries.

### 4.3. Composer Subsystem
**File:** `composer.rs`
**Responsibility:** Manipulation of existing PDF documents.

*   **`ObjectCopier`**: A crucial utility struct that performs a "Deep Copy" of PDF objects from one document to another. It maintains an `id_map` to handle cyclic references (e.g., Page -> Parent -> Kids -> Page) to prevent stack overflows during recursion.
*   **`merge_documents`**: Uses the copier to append or prepend pages from a source PDF to a target PDF.
*   **`overlay_content`**: Injects a new content stream into an existing page's `/Contents` array. Used for applying headers/footers to pre-rendered pages.

## 5. API Specification

### `DocumentRenderer` Trait
The primary API for the module.

```rust
pub trait DocumentRenderer<W: Write + Seek + Send> {
    /// Initializes the PDF structure (Header, Root, etc.)
    fn begin_document(&mut self, writer: W) -> Result<(), RenderError>;

    /// Pre-loads shared resources like images to avoid duplication.
    fn add_resources(&mut self, resources: &HashMap<String, SharedData>) -> Result<(), RenderError>;

    /// Converts layout elements into a raw Content Stream Object.
    /// Returns the ObjectId of the stream.
    fn render_page_content(
        &mut self,
        elements: Vec<PositionedElement>,
        font_map: &HashMap<String, String>,
        page_width: f32,
        page_height: f32,
    ) -> Result<ObjectId, RenderError>;

    /// Creates the Page Dictionary, linking content streams and annotations.
    /// Returns the ObjectId of the Page.
    fn write_page_object(
        &mut self,
        content_stream_ids: Vec<ObjectId>,
        annotations: Vec<ObjectId>,
        page_width: f32,
        page_height: f32,
    ) -> Result<ObjectId, RenderError>;

    /// Sets the ID of the Outlines (Bookmarks) root dictionary.
    fn set_outline_root(&mut self, outline_root_id: ObjectId);

    /// Finalizes the document structure, writes XRef/Trailer, and returns the writer.
    fn finish(self: Box<Self>, page_ids: Vec<ObjectId>) -> Result<W, RenderError>;
}
```

### `composer` Module
Utilities for post-generation manipulation.

```rust
/// Merges `source` into `target`.
pub fn merge_documents(
    target: &mut Document, 
    source: Document, 
    prepend: bool
) -> Result<(), RenderError>;

/// Adds a raw content stream (bytes) to `page_id` in `doc`.
pub fn overlay_content(
    doc: &mut Document, 
    page_id: ObjectId, 
    content_stream: Vec<u8>
) -> Result<(), RenderError>;
```

## 6. Feature Specification

*   **Streaming Output:** The module supports writing the PDF incrementally to disk/network, significantly reducing RAM usage for large documents.
*   **Parallel Rendering:** The stateless drawing architecture allows multiple pages to be rendered to operation buffers simultaneously across different threads.
*   **Resource Deduplication:** Images (`XObjects`) and Fonts are stored once in the resource dictionary and referenced by ID, keeping file sizes optimized.
*   **Advanced PDF Features:**
  *   **Link Annotations:** Generates internal `/GoTo` links for Table of Contents and Cross-References.
  *   **Outlines:** Generates a hierarchical PDF bookmark sidebar.
  *   **Layers (OCGs):** Supported in the `printpdf` backend (Optional Content Groups).
*   **PDF Composition:** Ability to merge distinct PDF artifacts and overlay dynamic content onto static pages.

## 7. Error Handling Strategy

The module uses a unified `RenderError` enum (`renderer.rs`) to encapsulate all failure modes.

*   **Error Types:**
  *   `RenderError::Io`: Wraps `std::io::Error`. Common in the streaming writer.
  *   `RenderError::Pdf`: Wraps errors from the underlying `lopdf` library (e.g., object not found).
  *   `RenderError::InternalPdfError`: Logic errors within the renderer (e.g., missing font mapping).
  *   `RenderError::Template`: Errors during Handlebars rendering (for footer templates).

*   **Handling:**
  *   Most drawing operations propagate errors up to the `DocumentRenderer`.
  *   The `StreamingPdfWriter` is strict; I/O errors immediately halt generation to prevent corrupt output.
  *   Missing resources (e.g., an image that failed to load) are logged as warnings, and the renderer attempts to continue without the specific element, rather than crashing the entire pipeline (seen in `drawing/image.rs`).