# AI Agent Rules: `idf` Module

This module defines the Intermediate Document Format (`IRNode`), the pivotal data structure connecting the parsing and layout stages.

## 1. Purpose & Scope
- **Mission:** The `idf` module defines the canonical, geometry-agnostic layout tree (`IRNode`) that represents a single document "sequence".
- **Single Source of Truth:** This is the exclusive data structure passed from the `parser` to the `layout` engine.

## 2. Core Rules
- **Rule 1: Strictly Geometry-Agnostic:** `IRNode` and its components represent the *semantic* structure and styling of a document. They **must not** contain any X/Y coordinates, absolute positions, page numbers, or other geometry-specific data. The layout engine is solely responsible for calculating this information.
- **Rule 2: Block vs. Inline:** Maintain the strict architectural distinction between block-level (`IRNode`) and inline-level (`InlineNode`) content.
    - Only `IRNode::Paragraph` can contain `InlineNode` children.
    - `InlineNode` represents content that flows and wraps within a line.
- **Rule 3: Style Representation:** Styles are passed through this representation, not computed.
    - `style_sets`: A `Vec<Arc<ElementStyle>>` of pre-resolved, shared pointers to named styles.
    - `style_override`: An `Option<ElementStyle>` for inline styles defined directly on an element.
    - The `idf` module itself does not merge or compute styles.
- **Rule 4: Layout Annotations:** The `layout` engine is permitted to add annotations to the `IRNode` tree during its measurement pass (e.g., `calculated_widths` in `IRNode::Table`). These are the only geometry-related fields allowed.
