# Petty Architecture

This document explains Petty's internal architecture, design decisions, and how the components work together.

## Table of Contents

- [Overview](#overview)
- [Workspace Structure](#workspace-structure)
- [Core Pipeline](#core-pipeline)
- [Component Details](#component-details)
- [Platform Abstraction](#platform-abstraction)
- [Execution Models](#execution-models)
- [Memory Model](#memory-model)
- [Rendering Strategies](#rendering-strategies)
- [Design Decisions](#design-decisions)

---

## Overview

Petty is a **platform-agnostic PDF generation engine** that transforms structured data into professional PDFs using declarative templates.

**Key architectural principles:**

1. **Platform Portability** - Core engine has no system dependencies
2. **Pluggable Everything** - Executors, resource providers, renderers are all abstracted
3. **Bounded Memory** - Memory usage is predictable and configurable
4. **Parallel by Default** - Multi-core processing for high throughput

### High-Level Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    User Application                         │
└──────────────────────┬──────────────────────────────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────────────────────┐
│                  PipelineBuilder (petty)                    │
│  ┌─────────────┐  ┌──────────────┐  ┌──────────────────┐  │
│  │  Template   │  │  Executors   │  │   Resources      │  │
│  │  Loaders    │  │  (Sync/Rayon)│  │   (FS/Memory)    │  │
│  └─────────────┘  └──────────────┘  └──────────────────┘  │
└──────────────────────┬──────────────────────────────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────────────────────┐
│              DocumentPipeline (petty)                       │
│  ┌────────────┐  ┌───────────┐  ┌──────────────────────┐  │
│  │ Providers  │  │ Renderers │  │  Concurrency Control │  │
│  │(Pass-thru, │  │(Streaming,│  │  (Channels, Sema)    │  │
│  │ Metadata)  │  │ Composing)│  │                      │  │
│  └────────────┘  └───────────┘  └──────────────────────┘  │
└──────────────────────┬──────────────────────────────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────────────────────┐
│                   petty-core                                │
│  ┌──────────┐  ┌─────────┐  ┌────────┐  ┌──────────────┐  │
│  │ Parsers  │  │ Layout  │  │ Render │  │   Traits     │  │
│  │(XSLT,    │  │ Engine  │  │ (lopdf)│  │ (Executor,   │  │
│  │ JSON)    │  │(Taffy)  │  │        │  │  Resources)  │  │
│  └──────────┘  └─────────┘  └────────┘  └──────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

---

## Workspace Structure

Petty uses a **workspace** to separate platform-agnostic core from platform adapters:

```
petty/                      # Workspace root
├── petty-core/            # Platform-agnostic PDF engine
│   ├── src/
│   │   ├── core/         # Layout, styling, fonts
│   │   ├── parser/       # Template parsing
│   │   ├── render/       # PDF generation
│   │   └── traits/       # Platform abstraction traits
│   └── Cargo.toml        # No system dependencies
│
└── petty/                 # Platform adapters + high-level API
    ├── src/
    │   ├── executor/     # Sync, Rayon executors
    │   ├── resource/     # Filesystem, in-memory providers
    │   ├── pipeline/     # Orchestration, concurrency
    │   ├── source/       # Data source abstractions
    │   └── templating/   # Fluent builder API
    └── Cargo.toml        # Platform-specific deps
```

### Why This Split?

**petty-core:**
- ✅ Compiles to WASM
- ✅ Pure Rust, no syscalls
- ✅ Suitable for embedded, WASM, WASI
- ✅ Can be used standalone with custom adapters

**petty:**
- Uses `petty-core` as a library
- Adds platform-specific implementations
- Provides high-level convenience APIs
- Includes native optimizations (tempfiles, threading)

---

## Core Pipeline

The document generation pipeline has several stages:

### 1. Template Compilation

```
Template File → Parser → IRNode Tree → CompiledTemplate
```

**Components:**
- `XsltParser` - Parses XSLT templates
- `JsonParser` - Parses JSON templates
- `IRNode` - Intermediate representation
- `CompiledTemplate` - Optimized, ready-to-execute template

**Key insight:** Templates are parsed once, reused many times.

### 2. Data Binding

```
Data (JSON) + Template → Handlebars → Bound IRNode Tree
```

**Process:**
- Data is fed through Handlebars expressions
- `{{variable}}` placeholders are resolved
- Conditional blocks are evaluated
- Loops are expanded

### 3. Layout Computation

```
IRNode Tree → LayoutEngine (Taffy) → Positioned Elements
```

**Layout engine:**
- Based on [Taffy](https://github.com/DioxusLabs/taffy) (CSS Flexbox/Grid)
- Computes positions, dimensions, line breaks
- Handles text shaping (rustybuzz)
- Manages page breaks

**Key types:**
- `LayoutEngine` - Main layout coordinator
- `LayoutNode` - Trait for layout algorithm dispatch
- `ParagraphLayoutNode` - Text layout with line breaking
- `FlexLayoutNode` - Container layout

### 4. PDF Rendering

```
Positioned Elements → Renderer → PDF Commands → lopdf Writer
```

**Renderers:**
- `LopdfRenderer` - PDF generation via lopdf
- Handles coordinate transformations
- Generates PDF objects (pages, fonts, images)
- Writes to `Write + Seek` trait

---

## Component Details

### Parsing Layer (petty-core)

Located in `petty-core/src/parser/`:

#### XsltParser

Parses XSL-FO-like templates:

```rust
pub struct XsltParser;

impl TemplateParser for XsltParser {
    fn parse(&self, source: &str, base_path: PathBuf)
        -> Result<TemplateFeatures, ParseError>;
}
```

**Features:**
- XPath-like selectors (`<xsl:value-of select="customer/name"/>`)
- Iteration (`<xsl:for-each>`)
- Conditionals (`<xsl:if>`, `<xsl:choose>`)
- Attribute sets for styling
- Handlebars expressions in text

#### JsonParser

Parses declarative JSON templates:

```rust
pub struct JsonParser;

impl TemplateParser for JsonParser {
    fn parse(&self, source: &str, base_path: PathBuf)
        -> Result<TemplateFeatures, ParseError>;
}
```

**Features:**
- Declarative node structure
- `"each"` for iteration
- Handlebars in `"content"` fields
- Style composition

### Layout Layer (petty-core)

Located in `petty-core/src/core/layout/`:

#### LayoutEngine

Main coordinator for layout computation:

```rust
pub struct LayoutEngine {
    font_library: Arc<SharedFontLibrary>,
    cache_config: PipelineCacheConfig,
    // ... caches
}

impl LayoutEngine {
    pub fn layout_sequence(
        &mut self,
        ir_node: &IRNode,
        page_width: f32,
        page_height: f32,
    ) -> Result<LaidOutSequence, LayoutError>;
}
```

**Responsibilities:**
- Font resolution and caching
- Style computation and caching
- Delegating to LayoutNode implementations
- Managing page breaks

#### LayoutNode Trait

Polymorphic layout algorithm:

```rust
pub trait LayoutNode: Send + Sync {
    fn layout(
        &self,
        available_width: f32,
        available_height: f32,
        engine: &mut LayoutEngine,
    ) -> Result<LayoutResult, LayoutError>;
}
```

**Implementations:**
- `ParagraphLayoutNode` - Text with line breaking
- `FlexLayoutNode` - Flexbox containers
- `TableLayoutNode` - Table layout
- `ImageLayoutNode` - Image embedding
- `HeadingLayoutNode` - TOC-aware headings

### Rendering Layer (petty-core)

Located in `petty-core/src/render/`:

#### LopdfRenderer

Generates PDF via lopdf:

```rust
pub struct LopdfRenderer<W: Write + Seek> {
    writer: W,
    layout_engine: LayoutEngine,
    stylesheet: Arc<Stylesheet>,
    // ... PDF state
}

impl<W: Write + Seek> DocumentRenderer<W> for LopdfRenderer<W> {
    fn begin_document(&mut self, writer: W) -> Result<(), RenderError>;
    fn render_sequence(&mut self, sequence: &LaidOutSequence)
        -> Result<(), RenderError>;
    fn finish(self, page_ids: Vec<u32>) -> Result<W, RenderError>;
}
```

**Features:**
- Two-pass rendering (metadata support)
- Font embedding
- Image embedding
- Hyperlink support
- TOC generation

---

## Platform Abstraction

Petty uses traits to abstract platform-specific operations.

### ResourceProvider Trait

Abstracts asset loading:

```rust
pub trait ResourceProvider: Send + Sync {
    fn load(&self, uri: &str) -> Result<Vec<u8>, ResourceError>;
    fn exists(&self, uri: &str) -> bool;
}
```

**Implementations:**
- `FilesystemResourceProvider` - Loads from disk (native)
- `InMemoryResourceProvider` - Loads from HashMap (WASM)

### FontProvider Trait

Abstracts font loading:

```rust
pub trait FontProvider: Send + Sync {
    fn load_font(&self, family: &str, weight: FontWeight, style: FontStyle)
        -> Result<FontData, FontError>;
    fn list_families(&self) -> Vec<String>;
}
```

**Implementations:**
- System fonts via fontdb (native)
- InMemoryFontProvider (WASM)
- Embedded fonts (feature-gated)

### Executor Trait

Abstracts parallelism:

```rust
pub trait Executor: Send + Sync {
    fn execute_all<T, R, F>(&self, items: Vec<T>, f: F) -> Vec<R>
    where
        T: Send + 'static,
        R: Send + 'static,
        F: Fn(T) -> R + Send + Sync + Clone + 'static;

    fn parallelism(&self) -> usize;
}
```

**Implementations:**
- `SyncExecutor` - Sequential (single-threaded)
- `RayonExecutor` - Parallel (work-stealing pool)

---

## Execution Models

### Sequential Execution (SyncExecutor)

```
Item 1 → Process → Result 1
Item 2 → Process → Result 2
Item 3 → Process → Result 3
```

**Characteristics:**
- Single-threaded
- Deterministic order
- Low memory overhead
- WASM-compatible

### Parallel Execution (RayonExecutor)

```
Item 1 ─┐
Item 2 ─┼─→ Thread Pool → Results (unordered)
Item 3 ─┘
```

**Characteristics:**
- Multi-threaded (work-stealing)
- Non-deterministic order
- Higher throughput
- Native only

### Concurrency Control

The pipeline uses channels and semaphores for backpressure:

```rust
let (tx, rx) = async_channel::bounded(buffer_size);
let semaphore = Arc::new(Semaphore::new(max_in_flight));

// Producer task
spawn(producer_task(data_iterator, tx, semaphore));

// Worker pool
spawn_workers(num_workers, context, rx, tx_output);

// Consumer task
run_in_order_streaming_consumer(rx_output, renderer);
```

**Key properties:**
- Bounded channels prevent unbounded queuing
- Semaphore limits in-flight work items
- Backpressure propagates through the pipeline

---

## Memory Model

Petty uses a **sequence-based memory model** for predictable memory usage.

### Sequence Processing

```
┌─────────────────────────────────────────┐
│ Sequence 1: Invoice for Customer A     │
│  - Load data                            │
│  - Bind template                        │
│  - Layout (IRNode → positioned)         │
│  - Render (positioned → PDF)            │
│  - [Memory freed]                       │
└─────────────────────────────────────────┘
┌─────────────────────────────────────────┐
│ Sequence 2: Invoice for Customer B     │
│  - Load data                            │
│  - ...                                  │
└─────────────────────────────────────────┘
```

**Benefits:**
- Memory usage = O(sequence_size), not O(document_size)
- Large documents don't cause OOM
- Users control memory via sequence granularity

### Memory Usage Breakdown

Per sequence:
1. **Data** - JSON value (~KB to MB depending on data)
2. **IRNode Tree** - Template structure (~100KB typical)
3. **Layout Result** - Positioned elements (~500KB typical)
4. **PDF Buffer** - Rendered output (varies, ~1MB per 10 pages)

**Total active memory** ≈ `num_workers * sequence_size * ~2MB`

With 4 workers processing 100-item sequences:
- ~800MB peak memory usage
- Predictable and configurable

### Tempfiles (Native Only)

On native platforms, the metadata analysis pass uses tempfiles:

```rust
#[cfg(feature = "tempfile")]
let buf_writer = {
    let temp_file = tempfile::tempfile()?;  // Writes to disk
    BufWriter::new(temp_file)
};

#[cfg(not(feature = "tempfile"))]
let buf_writer = {
    let memory_buffer = Cursor::new(Vec::new());  // Writes to RAM
    BufWriter::new(memory_buffer)
};
```

**Why:** MetadataGenerating mode renders the document twice. Using a tempfile avoids holding the entire PDF in memory.

---

## Rendering Strategies

Petty has two rendering strategies (providers + renderers):

### 1. SinglePassStreaming (Default)

**Provider:** `PassThroughProvider`
**Renderer:** `SinglePassStreamingRenderer`

```
Data → Workers → Render → Write (once)
```

**Characteristics:**
- Single-pass rendering
- Streams output immediately
- No metadata collection
- Memory-efficient
- Fast

**Limitations:**
- ❌ No TOC
- ❌ No page number references
- ❌ No total page count

**Use case:** Simple documents, high volume

### 2. MetadataGenerating

**Provider:** `MetadataGeneratingProvider`
**Renderer:** `ComposingRenderer`

```
Pass 1: Data → Workers → Render → Tempfile + Metadata
Pass 2: Data + Metadata → Workers → Render → Final PDF
```

**Characteristics:**
- Two-pass rendering
- Collects metadata (anchors, TOC entries)
- Injects metadata into second pass
- Higher memory/CPU usage

**Features:**
- ✅ Table of contents
- ✅ Page number references ({{page_num}}, {{total_pages}})
- ✅ Internal hyperlinks
- ✅ Index generation

**Use case:** Complex documents with cross-references

---

## Design Decisions

### 1. Why Taffy for Layout?

Taffy implements CSS Flexbox/Grid in Rust.

**Benefits:**
- Well-tested, production-ready
- Familiar model (CSS-like)
- Handles complex layouts
- No unsafe code

**Tradeoffs:**
- Flexbox isn't perfect for PDF (lacks page breaks)
- Added custom page-break handling on top

### 2. Why lopdf for Rendering?

lopdf is a pure-Rust PDF library.

**Benefits:**
- No C dependencies
- Works in WASM
- Full PDF spec support
- Active maintenance

**Alternatives considered:**
- printpdf - Higher-level but less flexible
- pdf-writer - Lower-level, more control

### 3. Why Two Template Formats?

**XSLT:**
- Familiar to XML/XSLT developers
- Powerful transformation language
- Good for complex data structures

**JSON:**
- Easier for beginners
- More compact syntax
- Better for programmatic generation

Both compile to the same `IRNode` representation.

### 4. Why Workspace Split?

**Problem:** Initial design had system dependencies (fontdb, tempfile) mixed with core logic.

**Solution:** Split into:
- petty-core: Pure logic, WASM-compatible
- petty: Platform adapters

**Benefits:**
- WASM support without #[cfg] everywhere
- Clear separation of concerns
- Users can depend on petty-core directly for custom integrations

### 5. Why Channels Instead of Direct Calls?

**Problem:** In parallel execution, need backpressure and ordering.

**Solution:** Bounded channels + semaphores.

**Benefits:**
- Backpressure prevents memory blowup
- Channels decouple producers/consumers
- Semaphore limits in-flight work

**Tradeoff:**
- Added complexity
- Requires Tokio (for now, could use crossbeam)

### 6. Why ExecutorImpl Enum Instead of dyn Executor?

**Problem:** Executor trait has generic methods, can't be made into trait object.

**Solution:** Use enum wrapper:

```rust
pub enum ExecutorImpl {
    Sync(SyncExecutor),
    Rayon(RayonExecutor),
}

impl Executor for ExecutorImpl {
    fn execute_all<T, R, F>(&self, items: Vec<T>, f: F) -> Vec<R> {
        match self {
            ExecutorImpl::Sync(e) => e.execute_all(items, f),
            ExecutorImpl::Rayon(e) => e.execute_all(items, f),
        }
    }
}
```

**Tradeoffs:**
- Must update enum when adding executors
- But: Zero-cost abstraction (static dispatch)
- And: No heap allocation for trait object

---

## Future Architecture Evolution

### Planned Improvements

1. **Remove Tokio Dependency**
   - Replace async channels with crossbeam
   - Make sync-only builds smaller

2. **Plugin System**
   - Dynamic template loaders
   - Custom layout algorithms
   - User-defined IRNode types

3. **Streaming Data Sources**
   - Kafka integration
   - Database cursors
   - Async iterators

4. **Multi-Backend Rendering**
   - SVG output
   - HTML output
   - Image output (PNG/JPEG)

5. **Incremental Compilation**
   - Cache compiled templates
   - Faster startup for long-running services

---

## Performance Characteristics

### Throughput

With Rayon executor on 8-core machine:
- ~100-200 invoices/second (single page)
- ~20-50 reports/second (multi-page)
- Scales linearly with cores up to ~8

### Latency

Single-threaded generation:
- Simple invoice: ~10-20ms
- Complex report (10 pages): ~100-200ms

### Memory

Typical usage:
- Base (fonts, templates): ~50MB
- Per worker: ~100MB
- Per sequence (active): ~2-5MB

### Disk I/O

SinglePassStreaming:
- Write-only, sequential
- No temp files

MetadataGenerating:
- Native: 1 tempfile (disk I/O)
- WASM: In-memory buffer

---

## Debugging Architecture

### Logging

Use `RUST_LOG` environment variable:

```bash
RUST_LOG=petty=debug cargo run
```

Log levels:
- `error` - Failures only
- `info` - Pipeline stages
- `debug` - Detailed operations
- `trace` - Everything (verbose)

### Profiling

Enable profiling feature:

```toml
[dependencies]
petty = { version = "0.1", features = ["profiling"] }
```

### Heap Profiling

Use dhat:

```toml
[dependencies]
dhat = "0.3"

[features]
dhat-heap = []
```

```rust
#[cfg(feature = "dhat-heap")]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

fn main() {
    #[cfg(feature = "dhat-heap")]
    let _profiler = dhat::Profiler::new_heap();

    // Your code
}
```

---

## Summary

Petty's architecture is designed for:

✅ **Portability** - Works on native, WASM, embedded
✅ **Performance** - Parallel processing, memory efficiency
✅ **Flexibility** - Pluggable components, multiple strategies
✅ **Reliability** - Bounded memory, comprehensive error handling

The workspace split and trait abstraction enable this without compromising on performance or ergonomics.
