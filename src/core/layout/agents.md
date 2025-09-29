# AI Agent Rules: `layout` Module

This module is responsible for the complex process of converting a geometry-agnostic `IRNode` tree into pages of drawable elements with absolute positions.

## 1. Purpose & Scope
- **Mission:** To take a single `idf::IRNode` tree (representing one "sequence") and produce an iterator that yields pages of `PositionedElement`s.
- **Input:** `LayoutUnit` (an `IRNode` tree + its data context).
- **Output:** An iterator of `Vec<PositionedElement>`.

## 2. Core Rules
- **Rule 1: Adhere to the Multi-Pass Algorithm:**
  1.  **Measurement Pass (`engine.rs`):** A pre-pass that walks the `IRNode` tree to calculate and annotate size-dependent properties *before* final positioning. The prime example is calculating table column widths.
  2.  **Positioning Pass (`page.rs`):** The main pass, implemented as a stateful `PageIterator`. It consumes the annotated `IRNode` tree, places elements, handles page breaks, and yields final pages.
- **Rule 2: Distinguish Paginated vs. Subtree Layout:**
  - **Paginated Layout (`page.rs`, `block.rs`, etc.):** The main layout logic that handles page flow and breaking. It is stateful and operates on a work stack.
  - **Subtree Layout (`subtree.rs`):** A separate set of stateless, recursive functions used to lay out self-contained nodes (like flex items or table cells) that must be measured as a single unit. These functions return a total height and a `Vec` of elements with positions *relative* to the subtree's origin.
- **Rule 3: Style Computation:** The `style.rs` submodule is responsible for resolving the style cascade (parent style -> named styles -> inline override) into a final, non-optional `ComputedStyle` for each element. This `ComputedStyle` is attached to every `PositionedElement`.
- **Rule 4: Final Output:** The `PositionedElement` is the definitive output of this module. It must contain the element's absolute X/Y coordinates and dimensions on the page, along with its fully resolved `ComputedStyle`.
- **Rule 5: Font Management:** The `FontManager` is the source of truth for all font metrics. All text measurement must be delegated to it.