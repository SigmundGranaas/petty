# AI Agent Rules: `pipeline` Module

This module is the main public API and orchestrator. It connects the `parser`, `layout`, and `render` stages into a high-performance, concurrent pipeline.

## 1. Purpose & Scope
- **Mission:** To provide a simple public API (`PipelineBuilder`) for configuring a document generation job and to delegate the execution to a selected generation strategy.
- **Role:** This module is the "conductor" of the orchestra. It sets up the shared `PipelineContext`, selects a `GenerationStrategy` based on user configuration and template features, and then hands off control.

## 2. Core Rules
- **Rule 1: Uphold the Strategy Pattern:** The core architecture is based on the Strategy Pattern.
    1.  **`GenerationStrategy` Trait:** Defines the common interface for all high-level document assembly algorithms.
    2.  **Concrete Strategies (`TwoPassStrategy`, `SinglePassStreamingStrategy`):** Each implements a specific algorithm (e.g., buffering for forward references vs. low-memory streaming). They are responsible for managing their own concurrency model (producer/worker/consumer).
    3.  **`DocumentPipeline`:** A simple facade that holds the chosen strategy and delegates the `generate` call to it.
- **Rule 2: The Builder is the Entry Point:** The `PipelineBuilder` is the sole public-facing entry point for configuration. It is responsible for:
    - Loading and compiling the template.
    - Discovering and loading fonts.
    - Selecting the PDF rendering backend and `GenerationMode`.
    - Instantiating the correct strategy and creating the `DocumentPipeline`.
- **Rule 3: Decoupling via Context:** The `PipelineContext` struct holds shared, read-only resources (like the compiled template and font manager) that are passed to the chosen strategy, decoupling the strategy's logic from the initial setup.
- **Rule 4: Explicit Trade-offs:** The architecture makes the performance and feature trade-offs explicit.
    - `SinglePassStreamingStrategy` is fast and low-memory but fails on templates with forward references.
    - `TwoPassStrategy` handles all features correctly but is slower and requires a `Clone`-able data iterator.