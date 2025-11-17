# AI Agent Rules: `pipeline` Module

This module is the main public API and orchestrator. It connects the `parser`, `layout`, and `render` stages into a high-performance, concurrent pipeline.

## 1. Purpose & Scope
- **Mission:** To provide a simple public API (`PipelineBuilder`) for configuring a document generation job and to delegate the execution to a selected pipeline configuration.
- **Role:** This module is the "conductor" of the orchestra. It sets up the shared `PipelineContext`, selects a `DataSourceProvider` and `RenderingStrategy` based on user configuration and template features, and then hands off control to the `DocumentPipeline` orchestrator.

## 2. Core Rules
- **Rule 1: Uphold the Provider/Renderer Pattern:** The core architecture is based on a two-stage composition pattern.
  1.  **`DataSourceProvider` Trait:** Defines the interface for components that prepare data for rendering. This might involve a simple pass-through or a full analysis pass that consumes the data to generate metadata.
  2.  **`RenderingStrategy` Trait:** Defines the interface for components that consume the prepared data sources and produce the final document. This might be a simple streaming renderer or a complex composing renderer that merges multiple artifacts.
  3.  **`DocumentPipeline`:** A simple facade that holds the chosen provider and renderer and executes them in sequence.
- **Rule 2: The Builder is the Entry Point:** The `PipelineBuilder` is the sole public-facing entry point for configuration. It is responsible for:
  - Loading and compiling the template.
  - Discovering and loading fonts.
  - Selecting the PDF rendering backend and `GenerationMode`.
  - **Intelligently selecting** the correct provider and renderer based on template features and user settings.
  - Instantiating and creating the `DocumentPipeline`.
- **Rule 3: Decoupling via Context:** The `PipelineContext` struct holds shared, read-only resources (like the compiled template and font manager) that are passed to the chosen components, decoupling their logic from the initial setup.
- **Rule 4: Explicit Trade-offs via Auto-Configuration:** The architecture makes the performance and feature trade-offs explicit but handles the choice automatically.
  - **Fast Path (`PassThroughProvider` + `SinglePassStreamingRenderer`):** Automatically chosen for simple templates. Fast and low-memory, but cannot handle forward references.
  - **Advanced Path (`MetadataGeneratingProvider` + `ComposingRenderer`):** Automatically chosen for templates with features like Tables of Contents or role templates. Slower and uses temporary storage, but handles all features correctly for any data source.