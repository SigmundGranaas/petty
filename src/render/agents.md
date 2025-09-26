# AI Agent Rules: `render` Module

This module is the final stage of the pipeline, responsible for converting the abstract `PositionedElement`s from the layout engine into a concrete document format (e.g., PDF).

## 1. Purpose & Scope
- **Mission:** To take a sequence of pages, where each page is a `Vec<PositionedElement>`, and write a finalized document to an output stream.
- **Input:** `Vec<PositionedElement>` per page, plus resources like images and fonts.
- **Output:** A byte stream (e.g., a PDF file).

## 2. Core Rules
- **Rule 1: Implement the `DocumentRenderer` Trait:** All rendering backends **must** implement the `DocumentRenderer` trait defined in `renderer.rs`. This provides a consistent, stream-oriented interface (`begin_document`, `render_page`, `finalize`) for the pipeline's consumer stage.
- **Rule 2: Isolate Backend Logic:** All code specific to a particular PDF generation library (`printpdf`, `lopdf`) must be contained within its own file (`pdf.rs`, `lopdf_renderer.rs`, respectively).
- **Rule 3: Centralize Drawing Logic:** The `drawing/` submodule contains the low-level logic for converting a `PositionedElement` into primitive drawing commands (e.g., PDF `Op`s). This logic should be shared between different backends where possible.
    - `rect.rs`: Handles backgrounds and borders.
    - `text.rs`: Handles text rendering.
    - `image.rs`: Handles image rendering.
- **Rule 4: Enable Parallel Rendering:** To support high-performance backends, create stateless versions of drawing functions (e.g., `draw_element_stateless`). These functions must not depend on a stateful `Renderer` object. Instead, they must accept all necessary context (drawing operations buffer, graphics state, resource maps) as arguments. This allows the rendering of multiple pages to be parallelized before being written to the output stream in sequence.
- **Rule 5: Resource Management:** The `add_resources` method is responsible for preparing shared resources (like images) for use in the document *before* any pages are rendered.