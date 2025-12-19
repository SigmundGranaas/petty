# Software Architecture Specification: `petty::core::layout` Module

## 1. Executive Summary

The `petty::core::layout` module is a critical component responsible for transforming a semantic, geometry-agnostic document representation (the Intermediate Document Format, `IDF`) into a paginated, fully positioned, and styled list of drawable elements ready for rendering. It acts as the bridge between the document parsing stage and the final rendering pipeline.

Architecturally, the module employs a **Compiler-Executor** pattern. It "compiles" the `IDF` tree into an internal `RenderNode` tree, where each node implements a common `LayoutNode` trait. This `RenderNode` tree is then "executed" in a pagination loop, calculating element sizes and positions page by page. Key design patterns include:

*   **Tree Traversal and Recursion:** Layout logic often involves traversing the `RenderNode` tree recursively to measure and position children.
*   **Strategy Pattern:** Specialized algorithms for complex layout tasks like Flexbox (`taffy` crate integration), Table column sizing, and text wrapping are encapsulated.
*   **Caching:** Extensive use of both global (`LayoutCache`) and thread-local (`ThreadLocalCache`) caches to optimize performance for repetitive computations (e.g., text shaping, font lookups, repeated layout measurements).
*   **Arena Allocation (`bumpalo`):** Utilizes a bump allocator for efficient memory management of layout nodes and associated data, reducing overhead and improving performance by minimizing individual allocations and deallocations.
*   **Dependency Injection:** `LayoutEngine`, `LayoutContext`, and `LayoutEnvironment` provide necessary dependencies and mutable state to layout nodes without direct coupling.

The `layout` module's primary role is to produce a sequence of `PageOutput` structures, each containing `PositionedElement`s, `AnchorLocation`s, and `IndexEntry`s, effectively preparing the document for any downstream rendering backend.

## 2. System Architecture

The `petty::core::layout` module can be logically divided into the following layers, illustrating the flow of data:

1.  **Input Layer (IDF):**
    *   **Description:** The module consumes `IRNode`s from the `petty::core::idf` module. These nodes represent the document's semantic structure and associated styles without any spatial information.
    *   **Key Data:** `IRNode` (e.g., `IRNode::Paragraph`, `IRNode::Table`, `IRNode::FlexContainer`).

2.  **Style Resolution Layer:**
    *   **Description:** Before layout, raw `ElementStyle` definitions (from stylesheets and inline overrides) are resolved against a parent's style to produce a final, immutable `ComputedStyle` for each node. This process handles inheritance and cascading.
    *   **Key Component:** `LayoutEngine::compute_style`.
    *   **Key Data:** `ElementStyle`, `ComputedStyle`.

3.  **Layout Node Construction Layer:**
    *   **Description:** The `IRNode` tree is transformed into a `RenderNode` tree. Each `IRNode` type has a corresponding `LayoutNode` implementation (e.g., `IRNode::Paragraph` maps to `ParagraphNode`). This layer prepares the data for layout by allocating it within a `LayoutStore`'s `Bump` arena and canonicalizing styles.
    *   **Key Components:** `LayoutEngine::build_render_tree`, `nodes::build_node_tree`, `LayoutStore`.
    *   **Key Data:** `RenderNode` (enum), `LayoutNode` (trait).

4.  **Measurement and Layout Layer:**
    *   **Description:** This is the core "execution" layer. The `RenderNode` tree is traversed to first `measure` (determine ideal size) and then `layout` (position and potentially paginate) each element. This process is driven by `BoxConstraints` and results in `PositionedElement`s. It interacts heavily with sub-algorithms for text, tables, and flexbox.
    *   **Key Components:** `LayoutNode::measure`, `LayoutNode::layout`, `LayoutContext`, `LayoutEnvironment`, `algorithms` module (Flex, Table, Pagination solvers), `text` module (Shaper, Wrapper).
    *   **Key Data:** `BoxConstraints`, `Size`, `LayoutResult`, `NodeState`, `ShapedRun`, `LineLayout`.

5.  **Output Aggregation Layer:**
    *   **Description:** As elements are laid out, their final positions and styles are recorded. This layer aggregates these elements, along with metadata like anchors and index entries, into `PageOutput` structures, which represent the complete content for a single page.
    *   **Key Components:** `LayoutContext::push_element`, `PaginationIterator`.
    *   **Key Data:** `PositionedElement`, `AnchorLocation`, `IndexEntry`, `PageOutput`.

### Data Flow Diagram

```mermaid
graph TD
    A[IDF IRNode Tree] --> B(LayoutEngine::build_render_tree);
    B --> C{Style Resolution: ElementStyle -> ComputedStyle};
    C --> D[RenderNode Tree (LayoutStore/Bump Allocator)];
    D --> E(LayoutEngine::paginate);
    E -- Loop per Page --> F[LayoutContext + LayoutEnvironment (Page-specific)];
    F --> G{LayoutNode::measure & LayoutNode::layout};
    G -- Uses sub-algorithms --> G1[Text Shaping/Wrapping];
    G -- Uses sub-algorithms --> G2[Flexbox (Taffy)];
    G -- Uses sub-algorithms --> G3[Table Column Sizing/Pagination];
    G -- Accumulates --> H[PositionedElements, Anchors, IndexEntries];
    H --> I[PageOutput];
    I -- Sequence of Pages --> J[Rendering Backend];
```

## 3. Core Abstractions & Data Models

This section outlines the most critical interfaces, traits, and data structures that define the core logic and contracts of the layout module.

### `LayoutNode` Trait (`interface.rs`)

This is the central trait for all layoutable elements in the system. Any concrete node type (e.g., `BlockNode`, `ParagraphNode`, `TableNode`) must implement this trait.

```rust
pub trait LayoutNode: Debug + Sync {
    /// Determines the intrinsic size of the node given constraints, without producing any visual output.
    fn measure(&self, env: &LayoutEnvironment, constraints: BoxConstraints) -> Result<Size, LayoutError>;

    /// Performs the full layout calculation, positioning children and producing drawable elements.
    /// It can return a `LayoutResult::Break` to signal that the node needs to continue on a new page.
    fn layout(
        &self,
        ctx: &mut LayoutContext,
        constraints: BoxConstraints,
        break_state: Option<NodeState>,
    ) -> Result<LayoutResult, LayoutError>;

    /// Returns the computed style for this node.
    fn style(&self) -> &ComputedStyle;

    /// Checks if this node specifically requests a page break and optionally specifies a page master.
    fn check_for_page_break(&self) -> Option<Option<TextStr>>;
}
```

### `RenderNode` Enum (`nodes/mod.rs`)

A concrete enum that wraps all specific `LayoutNode` implementations, enabling polymorphic dispatch within the layout engine.

```rust
pub enum RenderNode<'a> {
    Block(&'a BlockNode<'a>),
    Flex(&'a FlexNode<'a>),
    Heading(&'a HeadingNode<'a>),
    Image(&'a ImageNode<'a>),
    IndexMarker(&'a IndexMarkerNode<'a>),
    List(&'a ListNode<'a>),
    ListItem(&'a ListItemNode<'a>),
    PageBreak(&'a PageBreakNode<'a>),
    Paragraph(&'a ParagraphNode<'a>),
    Table(&'a TableNode<'a>),
}
```
`RenderNode` implements `LayoutNode`, delegating calls to the wrapped node.

### `LayoutEngine` (`engine.rs`)

The primary orchestrator of the layout process. It manages global resources, caching, and initiates the layout and pagination.

```rust
pub struct LayoutEngine {
    pub font_library: SharedFontLibrary,
    pub cache: LayoutCache,
    pub profiler: Box<dyn Profiler>,
    config: LayoutConfig,
}
```

### `LayoutStore` (`engine.rs`)

An arena allocator and style canonicalizer. It provides efficient memory management for the `RenderNode` tree and associated data using `bumpalo`.

```rust
pub struct LayoutStore {
    pub bump: Bump, // The bump allocator
    style_cache: RefCell<HashMap<ComputedStyle, Arc<ComputedStyle>>>, // Canonicalizes Arc<ComputedStyle>
    node_id_counter: AtomicUsize, // Provides unique IDs for nodes, used in caching
}
```

### `LayoutContext` (`interface.rs`)

The mutable context passed down during the `layout` pass. It tracks the current position (`cursor`), available space, and collects output elements and metadata for the current page.

```rust
pub struct LayoutContext<'a> {
    pub env: LayoutEnvironment<'a>, // Immutable environment
    pub arena: &'a Bump,
    bounds: geometry::Rect,         // Current layout boundaries
    cursor: (f32, f32),             // Current X, Y position
    elements: &'a mut Vec<PositionedElement>, // Output elements
    defined_anchors: &'a mut HashMap<TextStr, AnchorLocation>,
    index_entries: &'a mut HashMap<TextStr, Vec<IndexEntry>>,
    pub last_v_margin: f32,         // For margin collapsing
}
```

### `LayoutEnvironment` (`interface.rs`)

An immutable, read-only environment passed down during both `measure` and `layout` passes, providing access to the `LayoutEngine` and thread-local caches.

```rust
pub struct LayoutEnvironment<'a> {
    pub engine: &'a LayoutEngine,
    pub local_page_index: usize,
    pub cache: &'a RefCell<HashMap<u64, Box<dyn Any + Send>>>, // Thread-local cache for specific layout data
}
```

### `ComputedStyle` (`style.rs`)

The fully resolved and canonicalized style applied to a layout node. This struct aggregates all style properties (text, box model, flex, etc.) after inheritance and cascade, and includes a pre-calculated hash for efficient caching.

```rust
#[derive(Debug, Clone)]
pub struct ComputedStyle {
    pub inner: ComputedStyleData, // The actual style properties
    cached_hash: u64,             // Pre-calculated hash for fast comparisons
}
// Also includes various `*Model` structs (BoxModel, TextModel, FlexModel, etc.) inside `ComputedStyleData`.
```

### Geometry Primitives (`core/base/geometry.rs` & `layout/mod.rs` re-exports)

These fundamental types define spatial relationships. The canonical definitions are in `core::base::geometry`, re-exported in `layout::mod.rs` for convenience.

```rust
pub struct Rect { /* x, y, width, height */ }
pub struct Size { /* width, height */ }
pub struct BoxConstraints { /* min_width, max_width, min_height, max_height */ }
```

### Text Layout Primitives (`text/shaper.rs`, `text/wrapper.rs`)

Intermediate data structures for text processing.

```rust
pub struct ShapedRun { /* glyphs, width, style, font_data, etc. */ }
pub struct LineLayout { /* items, width, height, baseline */ }
pub struct LineItem { /* run_index, start_glyph, end_glyph, x, width */ }
```

### Pagination State (`interface.rs`)

Enums representing the break state of a node, allowing layout to resume on a new page.

```rust
pub enum NodeState {
    Block(BlockState),
    Flex(FlexState),
    ListItem(ListItemState),
    Paragraph(ParagraphState),
    Table(TableState),
    Atomic, // For nodes that cannot split (e.g., images)
}
// ... with corresponding state structs like BlockState, ParagraphState, etc.
```

## 4. Subsystem Analysis

The `layout` module is organized into several distinct subsystems:

### `core/layout/algorithms`

**Responsibility:** Provides specialized layout algorithms that are too complex or generic to be embedded directly into individual `LayoutNode` implementations.
**Key Files:**
*   `flex_solver.rs`: Contains utility functions for converting `petty`'s `ComputedStyle` into `taffy`'s layout `Style` objects, facilitating integration with the `taffy` Flexbox engine.
*   `pagination.rs`: Offers low-level utilities like `check_child_fit` to determine if content fits within available vertical space, crucial for generic page breaking.
*   `table_solver.rs`: Implements the core logic for calculating optimal column widths in a table, considering fixed, percentage, and auto-sizing dimensions based on content measurements.

### `core/layout/nodes`

**Responsibility:** This is the heart of the layout module, containing concrete implementations of the `LayoutNode` trait for each `IRNode` type. It's also where the `RenderNode` enum (the polymorphic wrapper) and its builder are defined.
**Key Files:**
*   `mod.rs`: Defines the `RenderNode` enum and the `build_node_tree` factory function, which dispatches to the appropriate `NodeBuilder` based on the `IRNode` kind.
*   `block.rs`: Implements `BlockNode`. Handles general block-level layout, including margin collapsing, padding, borders, backgrounds, and recursive layout of child `RenderNode`s. It's a foundational building block for many other nodes.
*   `paragraph/`:
    *   `node.rs`: `ParagraphNode` definition and its `LayoutNode` implementation.
    *   `builder.rs`: Builds a `ParagraphNode` from an `IRNode::Paragraph`, preparing text spans and inline images.
    *   `layout.rs`: Contains the logic for resolving shaped text runs (with caching) and performing line breaking (`ParagraphLayout`). Also handles widow and orphan control during rendering.
*   `table/`:
    *   `node.rs`: `TableNode` definition, manages column widths and row heights using `TableSolver`.
    *   `builder.rs`: Constructs `TableNode` and its nested `TableRowNode` and `TableCellNode` children from `IRNode::Table`.
    *   `pagination.rs`: `TablePagination` encapsulates the complex logic of laying out table rows, handling cell spanning (colspan/rowspan), and ensuring headers repeat on new pages.
*   `flex.rs`: Implements `FlexNode`. Delegates the complex Flexbox calculations to the `taffy` crate, adapting `ComputedStyle` to `taffy`'s `Style` and then applying `taffy`'s layout results.
*   `image.rs`: Implements `ImageNode`. Handles the layout of images, including their box model properties and integration into the layout flow.
*   `heading.rs`: Implements `HeadingNode`. Delegates most of its layout logic to an underlying `ParagraphNode`, effectively treating headings as styled paragraphs with a level.
*   `list.rs`: Implements `ListNode`. Manages the overall list structure and delegates to `ListItemNode`s. Handles nesting.
*   `list_item.rs`: Implements `ListItemNode`. Lays out individual list items, including marker generation (bullets, numbers) and their positioning (inside/outside).
*   `page_break.rs`: Implements `PageBreakNode`. Forces a page break in the layout flow, optionally switching to a different page master.
*   `index_marker.rs`: Implements `IndexMarkerNode`. A non-visual node that registers an index entry at the current page and Y position.

### `core/layout/painting`

**Responsibility:** Generates drawable primitive elements (like rectangles for backgrounds and borders) based on the computed styles and layout dimensions.
**Key Files:**
*   `box_painter.rs`: Contains `create_background_and_borders`, a reusable function for generating `PositionedElement::Rectangle`s for backgrounds, top, right, bottom, and left borders of a given bounding box.

### `core/layout/text`

**Responsibility:** Handles all aspects of text processing, including extracting text runs, font shaping, and line breaking.
**Key Files:**
*   `builder.rs`: `TextBuilder` flattens a sequence of `InlineNode`s into `TextSpan`s, consolidating adjacent spans with identical styles and tracking hyperlinks and inline images.
*   `shaper.rs`: Uses `rustybuzz` to perform text shaping, converting `TextSpan`s into `ShapedRun`s (sequences of glyphs with position and advance information). Leverages font data from `SharedFontLibrary`.
*   `wrapper.rs`: Implements the line breaking algorithm, taking `ShapedRun`s and a maximum width to produce `LineLayout`s. It also handles text alignment (left, right, center, justify).

### `core/layout/engine.rs`, `core/layout/interface.rs`, `core/layout/cache.rs`

**Responsibility:** These files define the core runtime environment, contexts, and caching mechanisms that orchestrate the entire layout process.
*   `engine.rs`: `LayoutEngine` is the main entry point for the layout process, managing the `SharedFontLibrary`, global `LayoutCache`, and a `Profiler`. It exposes methods for building the `RenderNode` tree and starting pagination. `LayoutStore` (bump allocator and style canonicalizer) also lives here.
*   `interface.rs`: Defines core traits (`LayoutNode`), contexts (`LayoutContext`, `LayoutEnvironment`), and state enums (`NodeState`, `LayoutResult`).
*   `cache.rs`: Defines `LayoutCache` (global, thread-safe caches for fonts, shaping, measurements) and `ThreadLocalCache` (per-thread cache for node-specific layout results).

### `core/layout/fonts.rs`

**Responsibility:** Manages font loading, caching, and resolution based on style properties.
**Key Files:**
*   `fonts.rs`: `SharedFontLibrary` uses `fontdb` to find fonts and caches `FontData` (raw font binaries) for `rustybuzz` shaping.

### `core/layout/style.rs`

**Responsibility:** Defines the structure of a `ComputedStyle` (the fully resolved style for a node) and the logic for computing it from `ElementStyle`s and parent styles.
**Key Files:**
*   `style.rs`: `ComputedStyle` (and its inner `ComputedStyleData` containing `BoxModel`, `TextModel`, `FlexModel`, etc.) and the `compute_style` function.

## 5. API Specification

The public API of the `petty::core::layout` module revolves around the `LayoutEngine` and the output `PageOutput`.

### Main Entry Points

*   **`LayoutEngine::new(library: &SharedFontLibrary, config: LayoutConfig) -> Self`**
    *   **Purpose:** Constructor for the layout engine. Initializes internal caches and a profiler.
    *   **Inputs:**
        *   `library: &SharedFontLibrary`: A reference to the shared font library, used for font resolution and shaping.
        *   `config: LayoutConfig`: Configuration settings for the layout engine, e.g., cache capacity.
    *   **Outputs:** `LayoutEngine` instance.

*   **`LayoutEngine::build_render_tree<'a>(ir_root: &IRNode, store: &'a LayoutStore) -> Result<RenderNode<'a>, LayoutError>`**
    *   **Purpose:** Transforms an `IRNode` tree (semantic document structure) into a `RenderNode` tree (layout-specific structure). This is the initial "compilation" step.
    *   **Inputs:**
        *   `ir_root: &IRNode`: The root of the Intermediate Document Format tree.
        *   `store: &'a LayoutStore`: A bump allocator and style canonicalizer where layout nodes and their data are allocated. The lifetime `'a` ensures that the `RenderNode` references data within this store.
    *   **Outputs:** `Result<RenderNode<'a>, LayoutError>`: The root of the layout-ready `RenderNode` tree, or an error if tree construction fails.

*   **`LayoutEngine::paginate<'a>(stylesheet: &'a Stylesheet, root_node: RenderNode<'a>, store: &'a LayoutStore) -> Result<impl Iterator<Item = Result<PageOutput, LayoutError>> + 'a, LayoutError>`**
    *   **Purpose:** Initiates the pagination process, taking the `RenderNode` tree and producing an iterator over `PageOutput` instances, effectively laying out the document page by page.
    *   **Inputs:**
        *   `stylesheet: &'a Stylesheet`: The stylesheet containing page masters and named element styles.
        *   `root_node: RenderNode<'a>`: The root of the pre-built `RenderNode` tree.
        *   `store: &'a LayoutStore`: The `LayoutStore` used for allocating page-specific data during layout.
    *   **Outputs:** `Result<impl Iterator<Item = Result<PageOutput, LayoutError>> + 'a, LayoutError>`: An iterator that yields `Result<PageOutput, LayoutError>` for each page. This allows for streaming page generation.

### Output Types

*   **`PageOutput` (`engine.rs`)**
    *   **Purpose:** Represents the complete laid-out content of a single page.
    *   **Fields:**
        *   `elements: Vec<PositionedElement>`: A list of all drawable elements on this page, with their absolute positions.
        *   `anchors: HashMap<TextStr, AnchorLocation>`: A map of named anchors (`id` attributes from `IRNode`s) to their page-local location.
        *   `index_entries: HashMap<TextStr, Vec<IndexEntry>>`: A map of index terms to their page-local locations.
        *   `page_number: usize`: The 1-based index of this page within the document.

*   **`PositionedElement` (`elements.rs`)**
    *   **Purpose:** A final, geometry-aware data structure representing a single drawable item on a page.
    *   **Fields:**
        *   `x: f32`: X-coordinate relative to the top-left of the page content area.
        *   `y: f32`: Y-coordinate relative to the top-left of the page content area.
        *   `width: f32`: Width of the element.
        *   `height: f32`: Height of the element.
        *   `element: LayoutElement`: The specific type of drawable element (text, rectangle, image, etc.).
        *   `style: Arc<ComputedStyle>`: The final computed style applied to this element.

*   **`LayoutElement` (`elements.rs`)**
    *   **Purpose:** Enum differentiating the various types of content that can be drawn.
    *   **Variants:**
        *   `Text(TextElement)`: For text content.
        *   `Rectangle(RectElement)`: For backgrounds, borders, or arbitrary rectangles.
        *   `Image(ImageElement)`: For raster images.
        *   `PageNumberPlaceholder { target_id: String, href: Option<String> }`: A special element that will be replaced by a page number during a later rendering pass (e.g., for cross-references).

*   **`TextElement` (`elements.rs`)**
    *   **Purpose:** Details specific to a block of text to be drawn.
    *   **Fields:**
        *   `content: String`: The actual text string.
        *   `href: Option<String>`: If present, this text acts as a hyperlink.
        *   `text_decoration: TextDecoration`: Any text decoration (e.g., underline).

*   **`RectElement` (`elements.rs`)**
    *   **Purpose:** A marker struct for a simple rectangle. No additional fields as `PositionedElement` carries dimensions and `ComputedStyle` carries color.

*   **`ImageElement` (`elements.rs`)**
    *   **Purpose:** Details specific to an image to be drawn.
    *   **Fields:**
        *   `src: String`: The source path or identifier for the image.

*   **`AnchorLocation` (`interface.rs`)**
    *   **Purpose:** Records the page and vertical position of a named anchor.
    *   **Fields:**
        *   `local_page_index: usize`: The 0-based index of the page where the anchor is located.
        *   `y_pos: f32`: The Y-coordinate of the anchor relative to the top-left of the page *content area*.

*   **`IndexEntry` (`interface.rs`)**
    *   **Purpose:** Records the page and vertical position of an index term.
    *   **Fields:**
        *   `local_page_index: usize`: The 0-based index of the page where the entry is located.
        *   `y_pos: f32`: The Y-coordinate of the entry relative to the top-left of the page *content area*.

### Key Helper Types

*   **`LayoutStore` (`engine.rs`)**
    *   `pub fn new() -> Self`: Creates a new `LayoutStore`.
    *   `pub fn alloc_str(&self, s: &str) -> &str`: Allocates a string slice into the bump arena.
    *   `pub fn next_node_id(&self) -> usize`: Returns a unique ID for a layout node.
    *   `pub fn cache_style(&self, style: Arc<ComputedStyle>) -> Arc<ComputedStyle>`: Canonicalizes an `Arc<ComputedStyle>`, returning an `Arc` to an existing equivalent style if available, otherwise inserts and returns the new one.
    *   `pub fn canonicalize_style(&self, style: Arc<ComputedStyle>) -> Arc<ComputedStyle>`: Alias for `cache_style`.

## 6. Feature Specification

The `petty::core::layout` module supports the following features:

*   **Document Structure Layout:**
    *   Generic Block-level element layout (`BlockNode`).
    *   Paragraph layout (`ParagraphNode`) with advanced text handling.
    *   Heading layout (`HeadingNode`) as specialized paragraphs.
    *   Image inclusion (`ImageNode`), both block-level and inline.
    *   Flexible Box Layout (`FlexNode`) using the `taffy` library, supporting `flex-direction`, `flex-wrap`, `justify-content`, `align-items`, `align-self`, `flex-grow`, `flex-shrink`, `flex-basis`, and `order`.
    *   Ordered and Unordered Lists (`ListNode`, `ListItemNode`) with configurable marker types (disc, circle, square, decimal, alpha, roman), marker positioning (inside/outside), and nested list numbering.
    *   Table Layout (`TableNode`) with complex column width resolution (fixed, percentage, auto), colspan, rowspan, and optional repeating headers across pages.
    *   Explicit Page Breaks (`PageBreakNode`) with optional specification of a new page master.
    *   Invisible Index Markers (`IndexMarkerNode`) to collect index entries.

*   **Text Processing:**
    *   Text Shaping: Advanced text layout using `rustybuzz` for accurate glyph positioning, ligatures, and kerning.
    *   Line Breaking: Algorithms to wrap text within given constraints, handling spaces and newlines.
    *   Text Alignment: Left, Right, Center, and Justify alignment for lines of text.
    *   Font Resolution: Dynamic font selection based on `font-family`, `font-weight`, and `font-style` via `fontdb`.
    *   Hyperlinks: Support for inline text hyperlinks.
    *   Cross-references: Support for page number references (`PageReference`).

*   **Box Model Implementation:**
    *   Margins: Top, Right, Bottom, Left margins for block-level elements. Vertical margin collapsing between adjacent blocks.
    *   Padding: Top, Right, Bottom, Left padding, creating space between content and borders.
    *   Borders: Solid borders with configurable width and color.
    *   Backgrounds: Solid background colors.

*   **Pagination & Flow Control:**
    *   Automatic page breaking for all block-level content when it exceeds page height.
    *   Manual page breaks using `IRNode::PageBreak`.
    *   Widow and Orphan Control: Ensures a minimum number of lines for a paragraph remain on the current page or are moved to the next page to prevent single lines from being stranded.
    *   Table Pagination: Supports automatic splitting of tables across pages, with the option to repeat table headers on subsequent pages.
    *   List Item Pagination: Handles splitting of complex list items across pages, ensuring marker is only drawn once.

*   **Metadata Generation:**
    *   Anchor Registration: Collects `id` attributes from `IRNode`s and records their page-local positions, useful for internal document navigation.
    *   Index Entry Collection: Gathers terms from `IRNode::IndexMarker` nodes with their page-local positions for generating an index.

*   **Performance Optimizations:**
    *   Caching: Global and thread-local caches for font data, text shaping results, and layout measurements to minimize redundant computations.
    *   Bump Allocation (`bumpalo`): Efficient memory management for layout-time data structures, reducing overhead and improving performance.
    *   Profiling: Optional debug profiler (`DebugProfiler`) to measure performance hotspots and cache hit/miss rates.

## 7. Error Handling Strategy

The `petty::core::layout` module uses a dedicated `LayoutError` enum, based on the `thiserror` crate, for robust and explicit error reporting. This ensures that errors arising from layout calculations are clearly distinguishable and provide contextual information.

### `LayoutError` Enum (`mod.rs`)

```rust
#[derive(Error, Debug)]
pub enum LayoutError {
    #[error("Node has a height of {0:.2} which exceeds the total page content height of {1:.2}.")]
    ElementTooLarge(f32, f32),
    #[error("Builder mismatch: Expected {0} node, got {1}.")]
    BuilderMismatch(&'static str, &'static str),
    #[error("State mismatch: Expected state for {0}, got {1}.")]
    StateMismatch(&'static str, &'static str),
    #[error("Generic layout error: {0}")]
    Generic(String),
}
```

### Strategy:

1.  **Specific Error Variants:**
    *   `ElementTooLarge`: Signifies that a node's intrinsic height (or required height after constraints) is greater than the available page content height, indicating an un-renderable state.
    *   `BuilderMismatch`: Occurs when the `build_node_tree` function attempts to use an incorrect `NodeBuilder` for a given `IRNode` type (e.g., trying to build a `ParagraphNode` from an `IRNode::Table`). This indicates a logical inconsistency in the IR node processing.
    *   `StateMismatch`: Arises during the `layout` pass if a `LayoutNode` receives a `NodeState` that is incompatible with its expected type (e.g., a `ParagraphNode` receiving a `TableState`). This guards against errors in the pagination state machine.

2.  **`Generic(String)`:** Used for less specific or catch-all errors, often wrapping errors from external libraries (like `taffy` or `rustybuzz`) or unexpected internal conditions that don't fit into more specific categories. This variant provides flexibility for immediate error reporting.

3.  **Propagation via `Result`:** All fallible functions within the layout module (e.g., `measure`, `layout`, `build_render_tree`, `paginate`) return `Result<T, LayoutError>`. This enforces explicit error handling at each step of the layout process.

4.  **Early Exit:** When an error occurs, the computation typically stops, and the error is immediately propagated up the call stack. This prevents cascading errors and ensures that invalid layout states are not carried forward.

5.  **Contextual Information:** Each `LayoutError` variant is designed to provide as much contextual information as possible (e.g., actual vs. expected node kinds, problematic dimensions, custom messages). The `thiserror` macro automatically generates `Display` implementations for these errors, making them human-readable.

By employing this strategy, the `layout` module aims for predictable and transparent error handling, which is crucial for debugging and maintaining a complex layout engine.