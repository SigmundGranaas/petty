# AI Agent Rules: Project-Wide

This file contains high-level rules and guidelines for AI agents working on the `petty` project.

## 1. Core Mission
- **Goal:** The project's primary goal is to be a high-performance, concurrent document generation engine. It transforms structured JSON data into professional PDFs using declarative templates (XSLT and JSON-based).
- **Core Principle:** Maintain a low and predictable memory footprint by processing documents in streaming "sequences".

## 2. Response Format
- **Complete Files:** Always respond with complete, compilable code files. Never use snippets, placeholders like `...`, or partial code.
- **Markdown Blocks:** Enclose all file content within Markdown code blocks, clearly indicating the file path.
- **Include Tests:** Never omit or shorten test suites from your responses. Tests are a critical part of the deliverable.
- **Small Commits:** When making changes, prefer to modify a small number of files. If a change is large, propose breaking down the logic into smaller, more manageable files first.

## 3. Architecture & Design
- **Strict Separation of Concerns:** The pipeline architecture is sacred. Adhere to the strict data flow: `parser` -> `idf` -> `layout` -> `render`. A module for one stage must not have knowledge of the internal workings of another.
- **Modularity:** Keep modules and files focused on a single responsibility. A file named `table.rs` should only contain logic for handling tables.
- **Concurrency:** The `pipeline` module orchestrates the concurrent execution of the other stages. Ensure that the `parser`, `layout`, and `render` modules remain thread-safe and (where possible) stateless to support this.

## 4. Code Quality & Style
- **Rust Best Practices:** Follow standard Rust idioms (e.g., use `Result` for error handling, leverage iterators, prefer composition over inheritance).
- **Clarity and Simplicity:** Write clear, readable code. Add comments to explain complex logic, non-obvious decisions, or `unsafe` blocks.
- **Performance:** Be mindful of performance. Avoid unnecessary allocations, clones, or computations in hot paths (e.g., layout loops, rendering). Use `Arc` for shared, read-only data.
- **Dependencies:** Do not add new third-party dependencies without explicit instruction.

## 5. Testing
- **Unit Tests:** Every logical unit (e.g., a style parser, a layout function) must be covered by unit tests within its own module.
- **Integration Tests:** Use integration tests to verify the interaction between different modules (e.g., `layout/integration_test.rs` tests how different `IRNode` types are laid out together).
- **Examples:** The `examples/` directory serves as end-to-end integration tests. Ensure they are always functional and demonstrate key features.