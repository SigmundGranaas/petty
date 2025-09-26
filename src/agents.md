# AI Agent Rules: `src` Directory

These rules apply to the overall structure and interaction of modules within the `src` directory.

## 1. Module Responsibilities
The `src` directory is organized into distinct stages of the document generation pipeline and foundational modules. Respect these boundaries:

- **`lib.rs`:** The public API of the crate. It should only `pub use` the necessary types for library consumers, primarily the `PipelineBuilder`.
- **`main.rs`:** A simple CLI wrapper around the `lib.rs` public API. It is for demonstration and basic usage, not for core library logic.
- **`core/`:** Foundational, primitive data types (e.g., `Color`, `Dimension`). Has zero dependencies on other `petty` modules.
- **`idf/`:** Defines the `IRNode` tree, the geometry-agnostic intermediate representation that connects the `parser` and `layout` stages.
- **`parser/`:** Converts template files and data into `IRNode` trees.
- **`layout/`:** Converts an `IRNode` tree into pages of positioned elements.
- **`render/`:** Converts positioned elements into a final output format (PDF).
- **`pipeline/`:** Orchestrates the concurrent execution of the `parser`, `layout`, and `render` stages.
- **`xpath/`:** A utility for data selection within JSON, used by the `parser`.

## 2. Error Handling
- **`error.rs`:** All top-level pipeline errors are defined in `PipelineError`.
- **Module-Specific Errors:** Each major module (`parser`, `layout`, `render`) defines its own specific error enum.
- **Error Propagation:** Use `thiserror` and the `?` operator to cleanly propagate errors up to the `PipelineError` level. Avoid using `panic!` or `unwrap()` except in tests or for truly unrecoverable states.

## 3. Public vs. Private API
- **Minimal Public API:** Keep the crate's public API in `lib.rs` as small and focused as possible. The primary entry point for users is the `PipelineBuilder`.
- **Internal Visibility:** Use `pub(crate)` for items that need to be shared between modules but should not be part of the public API. Default to private visibility (`pub(super)` or no modifier) for items used only within a single module.
  ==== END FILE ====

==== FILE: /home/sigmund/RustroverProjects/petty/src/core/agents.md ====
# AI Agent Rules: `core` Module

This module is the foundational layer of the project, defining primitive data types. Adherence to these rules is critical for project stability.

## 1. Purpose & Scope
- **Mission:** The `core` module defines stable, self-contained, primitive data types for styling and layout (e.g., `Color`, `Dimension`, `Border`, `Margins`).
- **The Styling "Language":** These types form the basic vocabulary used by the `parser`, `stylesheet`, `layout`, and `render` modules.

## 2. Core Rules
- **Rule 1: No Internal Dependencies:** The `core` module **must not** have any dependencies on other modules within the `petty` crate (e.g., `parser`, `layout`, `idf`). It sits at the bottom of the dependency graph.
- **Rule 2: Data-Oriented:** Types in this module should primarily be data containers. They should contain minimal logic. Complex parsing or computation logic belongs in other modules.
- **Rule 3: Serialization is Key:** All public types in this module **must** implement `serde::Serialize` and `serde::Deserialize`.
- **Rule 4: Support Shorthands:** Use custom `Deserialize` implementations to provide ergonomic, string-based shorthands for complex types, such as `"1pt solid #000"` for `Border` or `"10pt 20pt"` for `Margins`. The actual parsing logic for these shorthands should be delegated to functions in `src/parser/style.rs`.
- **Rule 5: Stability:** These types should be changed infrequently. Any modification has a cascading effect on the entire codebase.

