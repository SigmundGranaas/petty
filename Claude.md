# Petty PDF Generator - Development Guide

## Project Overview

**Petty** is a high-performance, platform-agnostic PDF generation engine written in Rust. It transforms structured data (JSON, XML) into professional PDF documents using declarative templates (XSLT 1.0 or custom JSON templates).

### What Petty Does

Petty implements a **compiler-executor-renderer pipeline** that processes data through five distinct stages:

```
Data (JSON/XML) → Template (XSLT/JSON) → Layout (IDF) → Render (lopdf) → PDF Output
```

**Primary Use Cases:**
- Business documents (invoices, receipts, contracts)
- High-volume batch generation (1000s of personalized documents)
- Reports and publications (financial reports, manuals, books)
- Real-time document generation services

**Key Characteristics:**
- **Platform Support**: Native (Linux, macOS, Windows) + WASM (browser)
- **Performance**: Parallel processing with work-stealing thread pool (150-200 simple invoices/sec)
- **Memory Efficiency**: Bounded memory usage via sequence-based processing and streaming output
- **Architecture**: Two-crate workspace (`petty-core` for platform-agnostic logic, `petty` for platform-specific orchestration)

---

## Project Goals & Requirements

### Core Goals

1. **Platform Agnosticism**
   - `petty-core` must compile to WASM without modification
   - No filesystem, threading, or system dependencies in core
   - All platform-specific code isolated to `petty` crate

2. **High Performance**
   - Linear throughput scaling up to 8+ cores
   - Bounded memory usage: O(sequence_size), not O(document_size)
   - Streaming output to avoid holding massive PDFs in RAM
   - Zero-copy operations where possible

3. **Production-Grade Reliability**
   - No panics in library code (fail with Result)
   - Comprehensive error handling with context
   - Graceful degradation (missing images, fonts)
   - Deterministic behavior for reproducible PDFs

4. **Maintainability**
   - Clear separation of concerns (parser → layout → render)
   - Trait-based abstractions for extensibility
   - Comprehensive documentation and examples
   - Minimal unsafe code with documented invariants

5. **Feature Completeness**
   - Full XSLT 1.0 and XPath 1.0 support
   - CSS Flexbox layout with Taffy
   - Complex text shaping (ligatures, kerning, complex scripts)
   - Advanced PDF features (TOC, hyperlinks, outlines, index generation)

### Technical Requirements

- **Rust 2024 Edition** (embrace modern idioms)
- **Minimal external dependencies** for core functionality
- **Thread-safe** where applicable (Arc, Mutex for shared state)
- **Memory-safe** (leverage Rust's ownership model, minimize unsafe)
- **Well-tested** (unit tests, integration tests, property tests)

---

## Architecture & Design Principles

### Workspace Structure

**petty-core/** - Platform-agnostic core
- `core/`: Primitives, layout engine, intermediate representation
- `parser/`: XSLT/JSON template parsing and execution
- `render/`: PDF generation backends (lopdf, printpdf)

**petty/** - Platform-specific orchestration
- `pipeline/`: High-level API, concurrency management
- `resource/`: Filesystem and asset loading
- `executor/`: Parallelism strategies (sync, rayon)

### Key Design Patterns

1. **Intermediate Representation (IRNode)**
   - Decouples parsers from layout engine
   - Platform-agnostic semantic tree

2. **Trait-Based Polymorphism**
   - `LayoutNode`: Layout algorithm dispatch
   - `DocumentRenderer`: Backend selection
   - `ResourceProvider`: Asset loading abstraction
   - `Executor`: Parallelism strategy

3. **Two-Pass Rendering**
   - Pass 1: Layout + metadata collection (TOC, anchors)
   - Pass 2: Re-layout with metadata for final PDF

4. **Arena Allocation**
   - `bumpalo` for LayoutNode trees
   - Reduces allocation overhead during hot path

5. **Bounded Concurrency**
   - Semaphores limit in-flight work
   - Backpressure prevents unbounded queue growth
   - Ordered streaming maintains output sequence

---

## Rust Development Guidelines for Petty

You are working on **Petty**, a production-grade PDF generation engine. Follow these Rust 2024 best practices adapted specifically for this project.

### Core Principles

1. **Idiomatic Rust 2024**: Embrace the 2024 edition's lifetime capture in `impl Trait` and temporary scoping in `if let`
2. **Safety First**: Leverage ownership, borrowing, and the type system to prevent bugs
3. **Performance-Conscious**: This is a throughput-oriented system—optimize hot paths
4. **Maintainability**: Design for future contributors and architectural evolution

---

## Strict Guidelines

### 1. Platform Agnosticism (Critical for petty-core)

**Rule**: Code in `petty-core/` MUST NOT use:
- `std::fs` (filesystem)
- `std::thread` or threading primitives
- `std::time::Instant` or system time
- Platform-specific APIs

**Instead**:
- Accept data as `&[u8]` or `String`, not file paths
- Use trait abstractions (`ResourceProvider`, `FontProvider`)
- Accept generic executors for parallel work
- Use platform-agnostic crates (`image`, `rustybuzz`, `taffy`)

**Example**:
```rust
// ❌ WRONG in petty-core
pub fn load_template(path: &Path) -> Result<Template> {
    std::fs::read_to_string(path)?
}

// ✅ CORRECT in petty-core
pub fn parse_template(content: &str) -> Result<Template> {
    // Parse from string, caller handles I/O
}
```

### 2. Memory Management & Allocation

#### Ownership & Borrowing
- **IRNode Trees**: Own data, use `String` not `&str` for content
- **LayoutNode Trees**: Allocate in `bumpalo::Bump` arena for fast deallocation
- **PositionedElements**: Use `Vec` owned by layout engine, lifetime = layout pass

**Arena Allocation Pattern**:
```rust
// Layout nodes have short lifetimes—use arena allocation
let arena = bumpalo::Bump::new();
let layout_tree = arena.alloc(create_layout_tree());
// All nodes freed at once when arena drops
```

#### Smart Pointers
- **Arc<PipelineContext>**: Shared across workers (templates, fonts, providers)
- **Rc<ComputedStyle>**: Shared within single-threaded layout pass
- **Box<dyn LayoutNode>**: Dynamic dispatch for polymorphic layout

**Example**:
```rust
// Shared context across threads
#[derive(Clone)]
pub struct PipelineContext {
    pub fonts: Arc<FontLibrary>,
    pub resources: Arc<dyn ResourceProvider>,
    pub templates: Arc<TemplateCache>,
}
```

### 3. Error Handling

#### Result for Recoverable Errors
All public APIs return `Result<T, E>` with structured error types using `thiserror`.

**Petty Error Hierarchy**:
```rust
// Level 1: Domain-specific
pub enum ParseError {
    #[error("Invalid XSLT syntax at {location}")]
    InvalidSyntax { location: String },
    // ...
}

// Level 2: Aggregation
pub enum PipelineError {
    #[error("Parse failed: {0}")]
    Parse(#[from] ParseError),
    #[error("Layout failed: {0}")]
    Layout(#[from] LayoutError),
    // ...
}
```

#### Panics are Programming Errors
- **Never panic** in library code (`petty-core`, `petty` lib)
- **Assert invariants** in tests and examples
- Use `expect()` with detailed messages only when logic guarantees success

**Example**:
```rust
// ❌ WRONG
pub fn get_font(&self, id: FontId) -> &Font {
    self.fonts.get(&id).unwrap() // Can panic!
}

// ✅ CORRECT
pub fn get_font(&self, id: FontId) -> Result<&Font, LayoutError> {
    self.fonts.get(&id)
        .ok_or(LayoutError::FontNotFound { id })
}
```

### 4. Concurrency & Parallelism

#### Bounded Concurrency Pattern (Critical)
Petty uses **bounded channels** and **semaphores** to prevent unbounded memory growth.

```rust
// Limit in-flight work items
let semaphore = Arc::new(tokio::sync::Semaphore::new(max_workers));

for data in input {
    let permit = semaphore.clone().acquire_owned().await?;
    tokio::spawn(async move {
        process(data).await;
        drop(permit); // Release permit when done
    });
}
```

#### Thread Safety
- **Arc** for shared state across threads (fonts, templates, providers)
- **Mutex/RwLock** for mutable shared state (rare in Petty)
- **Message passing** via `async_channel` for worker communication

**Pattern**:
```rust
// ✅ Shared immutable state
pub struct PipelineContext {
    pub fonts: Arc<FontLibrary>, // Thread-safe, immutable
}

// ✅ Per-worker mutable state
async fn worker(ctx: Arc<PipelineContext>, data: Data) -> Result<Output> {
    let mut layout_ctx = LayoutContext::new(); // Owned, mutable
    // ...
}
```

### 5. Generics & Traits

#### Trait Objects for Polymorphism
Petty uses dynamic dispatch for extensibility where performance impact is minimal.

**Key Traits**:
```rust
pub trait LayoutNode {
    fn compute_layout(&mut self, ctx: &mut LayoutContext) -> Result<()>;
}

pub trait ResourceProvider: Send + Sync {
    fn load_resource(&self, path: &str) -> Result<Vec<u8>>;
}

pub trait Executor {
    fn execute<F>(&self, tasks: Vec<F>) -> Vec<F::Output>
    where F: FnOnce() -> F::Output + Send;
}
```

**When to Use**:
- **Static Dispatch** (generics): Hot paths (layout algorithms, rendering)
- **Dynamic Dispatch** (trait objects): Plugin points (executors, providers, renderers)

#### Associated Types & GATs
Use associated types to reduce generic complexity.

```rust
// ✅ GOOD
pub trait Parser {
    type Output;
    fn parse(&self, input: &str) -> Result<Self::Output>;
}

// ❌ AVOID (overly generic)
pub trait Parser<T> {
    fn parse(&self, input: &str) -> Result<T>;
}
```

### 6. Performance Optimization

#### Hot Path Optimization
**Identify** hot paths with profiling (`cargo flamegraph`, `dhat`).

**Critical Hot Paths in Petty**:
1. Text shaping (`rustybuzz::shape`)
2. Flexbox layout (`taffy::compute_layout`)
3. PDF command generation (`lopdf` calls)
4. Style resolution & caching

**Optimization Techniques**:
- **Inline frequently called small functions**: `#[inline]`
- **Avoid allocations in loops**: Reuse `Vec` with `clear()`
- **Cache computed styles**: Use `Rc<ComputedStyle>` with hash-based dedup
- **Lazy evaluation**: Only compute layout when needed (pagination)

**Example**:
```rust
// Cache style to avoid recomputation
let style_cache: HashMap<u64, Rc<ComputedStyle>> = HashMap::new();
let hash = compute_style_hash(&properties);
let style = style_cache.entry(hash)
    .or_insert_with(|| Rc::new(ComputedStyle::from(properties)));
```

#### Zero-Copy Operations
Avoid copying when possible:
```rust
// ✅ GOOD: Borrow data
pub fn render_text(text: &str, font: &Font) -> Vec<GlyphId> { /*...*/ }

// ❌ AVOID: Unnecessary clone
pub fn render_text(text: String, font: Font) -> Vec<GlyphId> { /*...*/ }
```

#### SIMD (Future Consideration)
For bulk operations (color conversion, image processing), consider `portable-simd`.

### 7. Code Structure & Organization

#### Module Boundaries
- **Parser modules** (`parser/xslt`, `parser/json`): Input → IRNode
- **Layout modules** (`core/layout`): IRNode → PositionedElement
- **Render modules** (`render/`): PositionedElement → PDF bytes
- **Pipeline modules** (`pipeline/`): Orchestration, concurrency

**Visibility**:
- `pub` for public API (documented with `///`)
- `pub(crate)` for internal cross-module use
- `pub(super)` for parent-module-only access
- Private by default

#### Documentation Standards
```rust
/// Computes the layout for a flexbox container using Taffy.
///
/// # Arguments
/// * `container` - The flex container IRNode
/// * `ctx` - Mutable layout context for accumulating positioned elements
///
/// # Returns
/// A vector of positioned child elements with computed geometry.
///
/// # Errors
/// Returns `LayoutError::FlexLayoutFailed` if Taffy computation fails.
pub fn compute_flex_layout(
    container: &FlexContainer,
    ctx: &mut LayoutContext,
) -> Result<Vec<PositionedElement>> {
    // ...
}
```

### 8. Unsafe Code (Minimal Use)

**Current unsafe usage in Petty**:
- Font data FFI (`ttf-parser`, `rustybuzz` internals)
- PDF binary generation (lopdf byte manipulation)

**Guidelines**:
- **Encapsulate**: Wrap unsafe in safe abstractions
- **Document invariants**: Explain why it's safe
- **Test thoroughly**: Fuzz test unsafe boundaries

**Example**:
```rust
/// SAFETY: `data` must be valid UTF-8 (pre-validated by caller)
pub unsafe fn string_from_utf8_unchecked(data: Vec<u8>) -> String {
    String::from_utf8_unchecked(data)
}
```

### 9. Testing Strategy

#### Unit Tests
Test individual components in isolation.

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flex_layout_row() {
        let container = FlexContainer { /* ... */ };
        let mut ctx = LayoutContext::new();
        let result = compute_flex_layout(&container, &mut ctx).unwrap();
        assert_eq!(result.len(), 3);
    }
}
```

#### Integration Tests
Test full pipeline in `tests/` directory.

```rust
// tests/invoice_generation.rs
#[test]
fn test_generate_invoice_pdf() {
    let pipeline = PipelineBuilder::new()
        .with_template(include_str!("../templates/invoice.xslt"))
        .build().unwrap();

    let data = json!({"invoice_number": "INV-001"});
    let pdf = pipeline.generate(data).unwrap();

    assert!(pdf.len() > 1000); // PDF is non-trivial
}
```

#### Property Tests (Future)
Use `proptest` or `quickcheck` for layout algorithm correctness.

### 10. Modern Rust Idioms (2024 Edition)

#### Prefer `if let` and `while let`
```rust
// ✅ GOOD
if let Some(value) = optional {
    process(value);
}

// ❌ AVOID
match optional {
    Some(value) => process(value),
    None => {}
}
```

#### Use `?` for Error Propagation
```rust
// ✅ GOOD
pub fn parse_and_layout(template: &str, data: &str) -> Result<Vec<PositionedElement>> {
    let ir_tree = parse_template(template)?;
    let layout = compute_layout(&ir_tree)?;
    Ok(layout)
}
```

#### Leverage Iterators
```rust
// ✅ GOOD: Functional style, composable
let total_height: f32 = elements.iter()
    .filter(|e| e.is_visible())
    .map(|e| e.height)
    .sum();

// ❌ AVOID: Imperative loops
let mut total_height = 0.0;
for e in &elements {
    if e.is_visible() {
        total_height += e.height;
    }
}
```

#### Type Conversions with From/Into
```rust
impl From<IRNode> for RenderNode {
    fn from(ir: IRNode) -> Self {
        match ir {
            IRNode::Block(b) => RenderNode::Block(b.into()),
            IRNode::Text(t) => RenderNode::Text(t.into()),
        }
    }
}

// Usage
let render_node: RenderNode = ir_node.into();
```

#### Avoid Explicit Clones When Possible
```rust
// ✅ GOOD: Borrow when you can
fn render_element(elem: &PositionedElement) -> Vec<PdfCommand> { /*...*/ }

// ❌ AVOID: Unnecessary clone
fn render_element(elem: PositionedElement) -> Vec<PdfCommand> { /*...*/ }
```

### 11. Petty-Specific Patterns

#### IRNode Construction
Use builder patterns or macro for complex trees:
```rust
let ir = IRNode::Block(Block {
    id: Some("header".to_string()),
    class: vec!["title".to_string()],
    children: vec![
        IRNode::Text(TextNode { content: "Invoice".into(), .. }),
    ],
    ..Default::default()
});
```

#### Layout Context Management
`LayoutContext` is mutable state for a single layout pass:
```rust
pub fn layout_document(root: &IRNode) -> Result<Vec<PositionedElement>> {
    let mut ctx = LayoutContext::new();
    ctx.set_available_space(Size::new(595.0, 842.0)); // A4

    root.compute_layout(&mut ctx)?;

    Ok(ctx.take_elements())
}
```

#### Style Resolution & Cascading
Styles cascade from parent to child:
```rust
fn resolve_style(node: &IRNode, parent_style: &ComputedStyle) -> ComputedStyle {
    let mut style = parent_style.clone();
    style.apply_overrides(&node.inline_styles);
    style
}
```

#### PDF Streaming
Use `StreamingWriter` for memory efficiency:
```rust
let mut writer = StreamingWriter::new(output_file)?;
for page in pages {
    writer.write_page(&page)?;
}
writer.finalize()?;
```

---

## Common Pitfalls & Anti-Patterns

### ❌ AVOID: Breaking Platform Agnosticism
```rust
// petty-core/src/parser/xslt/compiler.rs
use std::fs; // ❌ Breaks WASM compatibility
```

### ❌ AVOID: Unbounded Memory Growth
```rust
// ❌ Can exhaust memory with large batches
let mut results = Vec::new();
for data in huge_dataset {
    results.push(process(data)); // Unbounded!
}
```

### ❌ AVOID: Unnecessary String Allocations
```rust
// ❌ Allocates on every call
fn get_tag_name(&self) -> String {
    self.name.clone()
}

// ✅ Return borrowed data
fn get_tag_name(&self) -> &str {
    &self.name
}
```

### ❌ AVOID: Panicking in Public APIs
```rust
// ❌ Can crash user applications
pub fn get_font(&self, id: FontId) -> &Font {
    &self.fonts[id] // Panics if missing!
}

// ✅ Return Result
pub fn get_font(&self, id: FontId) -> Result<&Font, LayoutError> {
    self.fonts.get(&id).ok_or(LayoutError::FontNotFound { id })
}
```

---

## Performance Targets

### Throughput Goals
- Simple invoice (1 page): **150-200 docs/sec** (8 cores)
- Complex report (10 pages): **20-50 docs/sec** (8 cores)
- Batch invoices (100s): **500-1000 docs/sec** (8 cores)

### Memory Goals
- Base overhead: **~50MB** (fonts, templates)
- Per worker: **~100MB**
- Per sequence: **2-5MB** (bounded)
- Total with 4 workers, 10k sequences: **~500MB** (not 50GB!)

### Optimization Checklist
1. Profile with `cargo flamegraph` or `perf`
2. Check allocations with `dhat`
3. Benchmark critical paths with `criterion`
4. Minimize clone/copy in hot paths
5. Use arena allocation for short-lived objects
6. Cache expensive computations (styles, shaped text)

---

## Development Workflow

### Building
```bash
# Native build
cargo build --release

# WASM build (petty-core only)
cd petty-core
wasm-pack build --target web
```

### Testing
```bash
# Unit tests
cargo test

# Integration tests
cargo test --test '*'

# Specific module
cargo test -p petty-core layout::tests
```

### Benchmarking
```bash
# Run benchmarks (if configured with criterion)
cargo bench

# Profile with flamegraph
cargo flamegraph --bin example_invoice
```

### Linting
```bash
# Clippy (strict mode)
cargo clippy -- -D warnings

# Format check
cargo fmt --check
```

---

## Key Architecture Documents

- **ARCHITECTURE.md**: Detailed workspace structure, component boundaries
- **USAGE.md**: API reference, configuration examples
- **TEMPLATES.md**: XSLT/JSON template syntax guides
- **WASM.md**: WebAssembly compatibility guidelines
- **petty-core/src/core/readme.md**: Layout engine specification
- **petty-core/src/render/README.md**: Rendering backend specification

---

## Summary: Rules of Thumb

1. **petty-core = no platform dependencies** (WASM-first mindset)
2. **Public APIs return Result** (never panic)
3. **Bounded concurrency** (semaphores, channels with capacity)
4. **Arena allocation** for layout trees (performance)
5. **Arc for cross-thread sharing** (PipelineContext, fonts)
6. **Trait objects for extensibility** (Executor, ResourceProvider)
7. **Document with `///`** (public API), `//` (internal complexity)
8. **Profile before optimizing** (flamegraph, dhat)
9. **Test integration paths** (not just units)
10. **Follow Rust 2024 idioms** (if let, ?, iterators, From/Into)

---

**Welcome to Petty development! Build fast, safe, and elegant PDF generation.**
