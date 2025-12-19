# Layout Engine Architecture

The `src/core/layout` module converts the high-level, geometry-agnostic **IDF** (`Intermediate Document Format`) into positioned elements ready for rendering. This process is split into two phases: **Tree Construction (Measurement)** and **Pagination (Positioning)**.

## 1. The Data Pipeline

1.  **Parsing**: The parser produces an `IRNode` tree. This tree describes *what* the content is (Block, Paragraph, Image) and *how* it looks (Styles), but not *where* it goes.
2.  **Tree Construction (`LayoutEngine::build_render_tree`)**:
    *   The engine traverses the `IRNode` tree.
    *   It resolves styles (inheritance, classes, inline overrides) into `ComputedStyle`.
    *   It creates a parallel `RenderNode` tree.
    *   **Memory Management**: `RenderNode`s are allocated in a `Bump` arena (`LayoutStore`). This avoids thousands of small heap allocations and makes cleanup instantaneous (dropping the arena).
3.  **Pagination (`LayoutEngine::paginate`)**:
    *   This is an iterator that produces pages.
    *   It maintains a `LayoutContext` cursor (X, Y position).
    *   It calls `layout()` on the `RenderNode` root.
    *   If content overflows a page, the layout function returns `LayoutResult::Break(State)`. The engine saves this state and resumes layout on the next page starting from that state.

## 2. Key Data Structures

### `IRNode` vs `RenderNode`
*   **`IRNode`**: Owned, expensive-to-clone tree (uses `Vec<IRNode>`). Contains raw strings and unresolved styles.
*   **`RenderNode`**: Lightweight, reference-heavy tree. Uses `&'a [RenderNode<'a>]` slices pointing to the arena. Contains `Arc<ComputedStyle>` and often pre-calculated measurements.

### `LayoutStore` (The Arena)
We use the `bumpalo` crate for a Bump Pointer Allocator.
*   **Why?** Layout generates thousands of temporary objects (text spans, line items, node wrappers) that live only as long as the document processing.
*   **How?** Instead of `malloc`/`free` for every node, we allocate sequentially in a large chunk of memory. Deallocation is O(1) (resetting the pointer).

### `LayoutContext`
A mutable context passed down during the positioning pass.
*   **Cursor**: Tracks current `(x, y)` position.
*   **Bounds**: The printable area of the current page.
*   **Margin Collapsing**: Tracks `last_v_margin` to implement the CSS block layout model (margins collapse; the distance between blocks is `max(bottom_margin, top_margin)`, not sum).

## 3. The Layout Cycle: Measure -> Layout

The layout engine uses a standard UI toolkit pattern:

1.  **Measure (`measure()`)**:
    *   **Input**: `BoxConstraints` (min/max width/height).
    *   **Output**: `Size` (how big the element wants to be).
    *   *Note*: This phase is stateless and side-effect free (mostly, barring caches). It asks "If I gave you X width, how tall would you be?".

2.  **Layout (`layout()`)**:
    *   **Input**: `LayoutContext` (position), `BoxConstraints`, `NodeState` (resume info).
    *   **Output**: `LayoutResult` (Finished or Break).
    *   **Action**: Places `PositionedElement`s into the context's list. If it hits the bottom of the page, it returns `Break` with enough state to resume (e.g., "I was on child #3, line #5").

## 4. Text Handling & Caching
Text layout is the most expensive operation.
*   **Shaping**: We use `rustybuzz` (HarfBuzz port) to convert characters to glyphs.
*   **Line Breaking**: We implement a custom greedy line breaker that handles multi-style text runs.
*   **Caching**:
    *   **Shaping Cache**: Maps `(Text, StyleID)` -> `Vec<Glyph>`. Reused across pages.
    *   **Layout Cache**: Maps `(NodeID, AvailableWidth)` -> `LineBreaks`. This prevents re-calculating line breaks for a paragraph every time we check if it fits on a page.