## Module: `core::idf`

### 1. Overview

The `idf` (Intermediate Document Format) module defines the central, geometry-agnostic data structure, `IRNode`. This tree structure serves as the canonical representation of a document's semantic content and styling, acting as the exclusive contract between the parsing stage and the layout engine.

### 2. Core Principles

-   **Geometry Agnostic**: `IRNode` and its components represent *what* to render, not *where*. They contain no X/Y coordinates, absolute positions, or page numbers. The layout engine is solely responsible for calculating this information.
-   **Strict Block vs. Inline Separation**: The architecture maintains a clear distinction between block-level elements (`IRNode`) which control vertical flow, and inline-level elements (`InlineNode`) which flow and wrap within lines.
-   **Uncomputed Styles**: Styles are passed through this representation, not computed. Each node carries references to named styles (`style_sets`) and an optional inline style (`style_override`). The layout engine resolves these into a final `ComputedStyle`.

### 3. Public API & Definitions

#### **Main Enums**

-   `pub enum IRNode`: Represents all possible block-level elements.
    -   `Root(Vec<IRNode>)`: The root of a document sequence.
    -   `Block { meta, children }`: A generic block container (like `<div>`).
    -   `FlexContainer { meta, children }`: A container for flexbox layout.
    -   `Paragraph { meta, children: Vec<InlineNode> }`: A block containing only inline content, serving as the context for text wrapping.
    -   `Image { meta, src }`: A block-level image.
    -   `List { meta, start, children }`: An ordered or unordered list container.
    -   `ListItem { meta, children }`: An item within a `List`.
    -   `Table { meta, columns, header, body }`: A structured table with a defined header and body.
    -   `Heading { meta, level, children: Vec<InlineNode> }`: A semantic heading.
    -   `TableOfContents { meta }`: A placeholder for a generated table of contents.
    -   `PageBreak { master_name }`: An explicit hard page break, optionally switching to a new page master.

-   `pub enum InlineNode`: Represents content that flows within a line.
    -   `Text(String)`: A raw string of text.
    -   `StyledSpan { meta, children }`: A styled text container (like `<span>`).
    -   `Hyperlink { href, meta, children }`: An interactive hyperlink.
    -   `Image { meta, src }`: An image placed inline with text.
    -   `PageReference { target_id, meta, children }`: An inline placeholder for a cross-reference or page number.
    -   `LineBreak`: A hard line break (like `<br>`).

#### **Metadata & Table Structs**

-   `pub struct NodeMetadata`: Common metadata for all `IRNode` variants.
    -   `id: Option<String>`: A unique identifier for anchor targeting.
    -   `style_sets: Vec<Arc<ElementStyle>>`: Pre-resolved, shared pointers to named styles.
    -   `style_override: Option<ElementStyle>`: Parsed inline style override.

-   `pub struct InlineMetadata`: Similar to `NodeMetadata` but for `InlineNode` variants (lacks the `id` field).

-   **Table Components**:
    -   `pub struct TableColumnDefinition`: Defines column properties like `width`.
    -   `pub struct TableHeader`: Container for header `TableRow`s.
    -   `pub struct TableBody`: Container for body `TableRow`s.
    -   `pub struct TableRow`: A single row containing `TableCell`s.
    -   `pub struct TableCell`: A cell that can contain any block-level `IRNode`, allowing for complex nested content. Includes `colspan` and `rowspan` properties.

---

## Module: `core::style`

### 1. Overview

This module is the foundational layer for all styling in the project. It defines primitive, self-contained data types that form the "vocabulary" of the styling system. These types are used by the parser to interpret style definitions and by the layout engine to render elements.

### 2. Core Principles

-   **Data-Oriented**: Types are primarily data containers with minimal logic.
-   **Serialization-First**: All public types implement `serde::Serialize` and `serde::Deserialize`, with custom implementations to support ergonomic string shorthands (e.g., parsing `"10pt 20pt"` into a `Margins` struct).
-   **Zero Internal Dependencies**: This module sits at the bottom of the dependency graph and does not depend on any other project modules.

### 3. Public API & Definitions

#### **Top-Level Structures**

-   `pub struct Stylesheet`: The root container for all styling information.
    -   `page_masters: HashMap<String, PageLayout>`: A map of named page layouts.
    -   `default_page_master_name: Option<String>`: The master to use for the first page.
    -   `styles: HashMap<String, Arc<ElementStyle>>`: A map of all named styles that can be applied to elements.

-   `pub struct PageLayout`: Defines the geometry and static content of a page.
    -   `size: PageSize`: The physical dimensions of the page.
    -   `margins: Option<Margins>`: The page margins, defining the content area.
    -   `header`, `footer`: Optional template nodes for page headers and footers.

-   `pub struct ElementStyle`: A struct where every field is an `Option`, representing a set of style properties that can be defined in a stylesheet or as an inline override. This is the direct deserialized representation of a style definition.

#### **Primitive Types**

-   **Dimension & Spacing**:
    -   `pub enum Dimension`: `Pt(f32)`, `Percent(f32)`, or `Auto`.
    -   `pub enum PageSize`: `A4`, `Letter`, `Legal`, or `Custom { width, height }`.
    -   `pub struct Margins`: `top`, `right`, `bottom`, `left` values.

-   **Color & Border**:
    -   `pub struct Color`: An `r, g, b, a` color representation.
    -   `pub struct Border`: Comprises `width: f32`, `style: BorderStyle`, and `color: Color`.
    -   `pub enum BorderStyle`: `None`, `Solid`, `Dashed`, `Dotted`, `Double`.

-   **Font & Text**:
    -   `pub enum FontWeight`: `Thin`, `Regular`, `Bold`, etc., plus `Numeric(u16)`.
    -   `pub enum FontStyle`: `Normal`, `Italic`, `Oblique`.
    -   `pub enum TextAlign`: `Left`, `Right`, `Center`, `Justify`.
    -   `pub enum TextDecoration`: `None`, `Underline`, `LineThrough`.

-   **List**:
    -   `pub enum ListStyleType`: `Disc`, `Circle`, `Decimal`, `LowerAlpha`, `LowerRoman`, etc.
    -   `pub enum ListStylePosition`: `Inside` or `Outside`.

-   **Flexbox**:
    -   `pub enum FlexDirection`: `Row`, `RowReverse`, `Column`, `ColumnReverse`.
    -   `pub enum FlexWrap`: `NoWrap`, `Wrap`, `WrapReverse`.
    -   `pub enum JustifyContent`: `FlexStart`, `Center`, `SpaceBetween`, etc.
    -   `pub enum AlignItems`: `Stretch`, `FlexStart`, `Center`, etc.
    -   `pub enum AlignSelf`: `Auto`, `Stretch`, `FlexStart`, etc.

---

## Module: `core::layout`

### 1. Overview

The `layout` module is the engine of the project. It consumes a geometry-agnostic `IRNode` tree and produces a series of pages, where each page is a collection of `PositionedElement`s with absolute coordinates and fully resolved styles.

### 2. Core Principles & Architecture

The layout process is a stateful, cooperative, multi-pass algorithm:

1.  **Pre-processing Pass (`engine.rs`)**: Before layout begins, the `IRNode` tree is traversed to collect metadata required by certain nodes. For example, all headings with IDs are collected so the `TableOfContentsNode` can generate its content.
2.  **Measurement Pass (`LayoutNode::measure`)**: Before a node and its children are positioned, a `measure` pass is performed. This pass calculates size-dependent properties that are necessary for positioning. The most critical example is `TableNode`, which calculates all its column widths during this phase.
3.  **Positioning/Layout Pass (`LayoutNode::layout`)**: This is the main pass. It is a stateful process that iterates down the layout tree, placing elements onto the current page. When a node does not fit, it returns a `LayoutResult::Partial` containing the remainder of itself, which triggers a page break. The layout engine then starts a new page and continues layout with the remainder.

### 3. Public API & Key Components

#### **`engine.rs` - The Orchestrator**

-   **Entrypoint**: `pub fn paginate(&self, stylesheet: &Stylesheet, ir_nodes: Vec<IRNode>) -> Result<(Vec<Vec<PositionedElement>>, HashMap<String, AnchorLocation>)>`
    -   This is the sole public entry point into the layout module.
    -   It drives the main pagination loop, managing page masters, creating `LayoutContext`s for each page, and handling page breaks signaled by `LayoutResult::Partial`.
-   **`LayoutEngine` Struct**: The main struct, holding shared resources like the `FontManager`.
-   **Tree Building**: The internal `build_layout_node_tree` function acts as a factory, recursively converting the `IRNode` tree into a tree of `LayoutNode` trait objects, which are the active participants in the layout process.

#### **`node.rs` - The Layout Trait**

-   **`pub trait LayoutNode`**: The central abstraction for all layoutable elements.
    -   `fn layout(&mut self, ctx: &mut LayoutContext) -> Result<LayoutResult, LayoutError>`: The core method where an element positions itself and its children.
    -   `fn measure(&mut self, engine: &LayoutEngine, available_width: f32)`: The pre-pass for size calculation.
    -   `fn measure_content_height(...) -> f32`: Calculates the total vertical space required by the node.
    -   `fn measure_intrinsic_width(...) -> f32`: Calculates the "max-content" width, used for `flex-basis: auto`.
-   **`pub struct LayoutContext<'a>`**: The "canvas" provided to a `layout` call, containing the available bounds, current cursor position, and references to the page's element list and the document's anchor map.
-   **`pub enum LayoutResult`**: The mechanism for cooperative pagination.
    -   `Full`: The node was laid out completely.
    -   `Partial(Box<dyn LayoutNode>)`: The node only partially fit. The box contains the remainder to be laid out on the next page.

#### **`elements.rs` - The Output**

-   **`pub struct PositionedElement`**: The definitive, drawable output of the layout process.
    -   `x, y, width, height: f32`: Absolute coordinates and dimensions on the page.
    -   `element: LayoutElement`: The type of element to draw.
    -   `style: Arc<ComputedStyle>`: The fully resolved, non-optional style for this element.
-   **`pub enum LayoutElement`**: The concrete drawable primitives: `Text`, `Rectangle`, `Image`, `PageNumberPlaceholder`.

#### **`style.rs` - Style Computation**

-   **`pub struct ComputedStyle`**: A fully resolved style struct where **no fields are optional**. This is the result of the style cascade.
-   **`pub fn compute_style(...)`**: The function that implements the style cascade: it inherits properties from a parent `ComputedStyle`, merges in any named `ElementStyle` definitions, and finally applies an inline `ElementStyle` override. It correctly handles the distinction between inherited and non-inherited properties.

#### **`text.rs` & `fonts.rs` - Text & Font Handling**

-   **`pub struct FontManager`**: Manages loading, querying (`fontdb`), parsing, and caching (`fontdue`) fonts. It is the single source of truth for all text measurement.
-   **`enum LayoutAtom`**: An internal representation for the smallest units of inline content (`Word`, `Space`, `Image`, etc.). The `atomize_inlines` function converts `InlineNode`s into a stream of these atoms, which is then consumed by the paragraph line-breaker.

#### **`nodes/` - Concrete `LayoutNode` Implementations**

Each file in this directory implements the `LayoutNode` trait for a specific `IRNode` variant.

-   **`block.rs` (`BlockNode`)**: The fundamental container. Stacks children vertically, handles its own box model (margins, padding, borders, background), and manages vertical margin collapsing with its siblings.
-   **`paragraph.rs` (`ParagraphNode`)**: Handles complex text layout. Its `layout` method performs line-breaking on its `LayoutAtom` stream, respects `text-align`, and implements widow/orphan control to prevent awkward page breaks.
-   **`flex.rs` (`FlexNode`)**: Implements the Flexbox layout algorithm. The `measure` pass resolves children into `FlexLine`s. The `layout` pass positions items within these lines according to flex properties. It can break pages *between* flex lines.
-   **`table.rs` (`TableNode`)**: Implements table layout. Its `measure` pass is crucial, as it calculates all column widths before positioning begins. The `layout` pass places cells and handles page breaks *between* rows, correctly repeating the table header on subsequent pages. It also manages `colspan` and `rowspan` logic.
-   **`list.rs` & `list_item.rs` (`ListNode`, `ListItemNode`)**: `ListNode` orchestrates the creation of its `ListItemNode` children, providing them with their index and nesting depth. `ListItemNode` is responsible for rendering the list marker (e.g., "•", "1.") and indenting its content.
-   **`image.rs` (`ImageNode`)**: A simple node that places a block-level image. If it doesn't fit in the remaining page space, it requests a page break.
-   **`heading.rs` (`HeadingNode`)**: Behaves identically to a `ParagraphNode` but also registers its `id` as an anchor in the `LayoutContext` during the `layout` pass.
-   **`table_of_contents.rs` (`TableOfContentsNode`)**: A "generator" node. In its constructor, it consumes a list of all headings in the document (provided by the `LayoutEngine`) and dynamically generates an `IRNode` tree representing the TOC entries. It then delegates the layout of this generated tree to an internal `BlockNode`.
-   **`page_break.rs` (`PageBreakNode`)**: A simple marker node. Its `layout` method forces a page break by returning `LayoutResult::Partial` with itself as the remainder, unless it's already at the top of a fresh page.