# AI Agent Rules: `pipeline` Module

This module is the main public API and orchestrator. It connects the `parser`, `layout`, and `render` stages into a high-performance, concurrent pipeline.

## 1. Purpose & Scope
- **Mission:** To provide a simple public API (`PipelineBuilder`) for configuring a document generation job and to manage the concurrent execution of parsing, layout, and rendering.
- **Role:** This module is the "conductor" of the orchestra. It does not contain any specific parsing, layout, or rendering logic itself.

## 2. Core Rules
- **Rule 1: Uphold the Concurrent Pipeline Model:** The architecture is a multi-stage, channel-based system.
    1.  **Producer Stage (`orchestrator.rs`):** Takes an iterator of data items and sends them into the first channel for the workers.
    2.  **Worker Stage (`worker.rs`):** A pool of threads/tasks that consume from the first channel. Each worker performs the full parse-and-layout process for a single data item, producing a `LaidOutSequence` (pages of positioned elements + resources).
    3.  **Consumer Stage (`orchestrator.rs`):** A single task that consumes `LaidOutSequence`s from the second channel. It is responsible for re-ordering out-of-order results and feeding them sequentially to the `render` engine.
- **Rule 2: The Builder is the Entry Point:** The `PipelineBuilder` is the sole public-facing entry point for configuring a pipeline. It is responsible for all setup, including:
    - Loading and compiling the template (XSLT or JSON).
    - Discovering and loading fonts.
    - Selecting the PDF rendering backend.
- **Rule 3: Decoupling and Orchestration:** This module's responsibility is data flow and concurrency management. It connects the other modules together via traits (`TemplateProcessor`, `DocumentRenderer`) and channels but knows nothing of their internal implementation.
- **Rule 4: Support for Streaming:** The entire design must support processing an unbounded stream of input data items without holding the entire document in memory. Each "sequence" is processed and rendered, and its memory can then be freed.
