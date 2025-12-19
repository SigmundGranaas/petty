# Petty Usage Guide

Complete guide to using Petty for PDF generation from structured data.

## Table of Contents

- [Quick Start](#quick-start)
- [Core Concepts](#core-concepts)
- [API Reference](#api-reference)
- [Template Formats](#template-formats)
- [Configuration Options](#configuration-options)
- [Advanced Features](#advanced-features)
- [Error Handling](#error-handling)
- [Performance Tuning](#performance-tuning)

---

## Quick Start

### Basic Example

```rust
use petty::{PipelineBuilder, PipelineError};
use serde_json::json;

fn main() -> Result<(), PipelineError> {
    // 1. Build a pipeline with a template
    let pipeline = PipelineBuilder::new()
        .with_template_file("template.xsl")?
        .build()?;

    // 2. Prepare your data
    let data = vec![json!({
        "title": "My Document",
        "content": "Hello, PDF!"
    })];

    // 3. Generate PDF
    pipeline.generate_to_file(data, "output.pdf")?;

    Ok(())
}
```

### Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
petty = "0.1"
serde_json = "1.0"
```

---

## Core Concepts

### 1. Sequences

A **sequence** is a self-contained unit of document processing. Think of it as:
- One invoice in a batch of invoices
- One chapter in a book
- One report in a series

Each sequence is processed independently, giving you control over memory usage.

```rust
// Multiple sequences = multiple invoices
let invoices = vec![
    json!({"customer": "Alice", "amount": 100}),
    json!({"customer": "Bob", "amount": 200}),
];
pipeline.generate_to_file(invoices, "invoices.pdf")?;
```

### 2. Pipeline Architecture

```
Data → Template → Layout → Render → PDF
  ↓        ↓         ↓        ↓
JSON    XSLT/    Taffy   lopdf
        JSON     Engine  Writer
```

The pipeline processes data through these stages:
1. **Parsing**: Template is parsed and compiled
2. **Binding**: Data is merged with template (Handlebars)
3. **Layout**: Taffy computes positions and dimensions
4. **Rendering**: PDF commands are generated

### 3. Templates

Templates define document structure and styling. Two formats are supported:

- **XSLT**: XML-based, powerful transformation language
- **JSON**: Declarative JSON structure

See [TEMPLATES.md](TEMPLATES.md) for detailed syntax guide.

---

## API Reference

### PipelineBuilder

The main entry point for creating document generation pipelines.

#### Constructor

```rust
let builder = PipelineBuilder::new();
```

Creates a builder with defaults:
- Fallback fonts loaded
- Filesystem resource provider (current directory)
- Rayon executor (parallel, if feature enabled)
- Single-pass streaming mode

#### Methods

##### `with_template_file(path: impl AsRef<Path>) -> Result<Self, PipelineError>`

Load a template from a file. Format is detected by extension (`.xsl`, `.json`).

```rust
let pipeline = PipelineBuilder::new()
    .with_template_file("templates/invoice.xsl")?
    .build()?;
```

##### `with_template_string(source: &str, format: &str) -> Result<Self, PipelineError>`

Load a template from a string.

```rust
let template = r#"
    <xsl:stylesheet version="1.0" ...>
        <!-- template content -->
    </xsl:stylesheet>
"#;

let pipeline = PipelineBuilder::new()
    .with_template_string(template, "xsl")?
    .build()?;
```

##### `with_resource_provider(provider: Arc<dyn ResourceProvider>) -> Self`

Set a custom resource provider for loading images, fonts, etc.

```rust
use petty::resource::InMemoryResourceProvider;

let resources = InMemoryResourceProvider::new();
resources.add("logo.png", logo_bytes)?;

let pipeline = PipelineBuilder::new()
    .with_template_file("template.xsl")?
    .with_resource_provider(Arc::new(resources))
    .build()?;
```

##### `with_executor(executor: ExecutorImpl) -> Self`

Set a custom executor for controlling parallelism.

```rust
use petty::executor::{SyncExecutor, ExecutorImpl};

// Single-threaded execution
let pipeline = PipelineBuilder::new()
    .with_template_file("template.xsl")?
    .with_executor(ExecutorImpl::Sync(SyncExecutor::new()))
    .build()?;
```

##### `with_generation_mode(mode: GenerationMode) -> Self`

Choose between rendering strategies:
- `GenerationMode::SinglePassStreaming` (default): Fast, no cross-references
- `GenerationMode::MetadataGenerating`: Two-pass, supports TOC and page numbers

```rust
use petty::pipeline::GenerationMode;

let pipeline = PipelineBuilder::new()
    .with_template_file("template.xsl")?
    .with_generation_mode(GenerationMode::MetadataGenerating)
    .build()?;
```

##### `with_system_fonts() -> Result<Self, PipelineError>`

Load system fonts (native platforms only).

```rust
let pipeline = PipelineBuilder::new()
    .with_template_file("template.xsl")?
    .with_system_fonts()?
    .build()?;
```

##### `with_font_file(path: impl AsRef<Path>) -> Result<Self, PipelineError>`

Add a custom font from a file.

```rust
let pipeline = PipelineBuilder::new()
    .with_template_file("template.xsl")?
    .with_font_file("fonts/CustomFont.ttf")?
    .build()?;
```

##### `with_debug(debug: bool) -> Self`

Enable debug logging.

```rust
let pipeline = PipelineBuilder::new()
    .with_template_file("template.xsl")?
    .with_debug(true)
    .build()?;
```

##### `build() -> Result<DocumentPipeline, PipelineError>`

Finalize the builder and create the pipeline.

```rust
let pipeline = PipelineBuilder::new()
    .with_template_file("template.xsl")?
    .build()?;
```

### DocumentPipeline

The compiled pipeline ready for PDF generation.

#### Methods

##### `generate_to_file<I>(data: I, path: impl AsRef<Path>) -> Result<(), PipelineError>`

Generate PDF to a file.

```rust
let data = vec![json!({"title": "Document"})];
pipeline.generate_to_file(data, "output.pdf")?;
```

##### `generate_to_writer<I, W>(data: I, writer: W) -> Result<W, PipelineError>`

Generate PDF to any `Write + Seek` writer.

```rust
use std::io::Cursor;

let data = vec![json!({"title": "Document"})];
let buffer = Cursor::new(Vec::new());
let result = pipeline.generate_to_writer(data, buffer)?;
let pdf_bytes = result.into_inner();
```

---

## Template Formats

### XSLT Templates

XSLT templates use an XSL-FO-like syntax for PDF generation.

**Basic structure:**

```xml
<xsl:stylesheet version="1.0"
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
                xmlns:fo="http://www.w3.org/1999/XSL/Format">

    <!-- Page setup -->
    <fo:simple-page-master master-name="A4"
                           page-width="210mm" page-height="297mm"
                           margin="20mm"/>

    <!-- Template -->
    <xsl:template match="/">
        <fo:block>
            <p use-attribute-sets="title">
                <xsl:value-of select="title"/>
            </p>
        </fo:block>
    </xsl:template>

    <!-- Styles -->
    <xsl:attribute-set name="title">
        <xsl:attribute name="font-size">24pt</xsl:attribute>
        <xsl:attribute name="font-weight">bold</xsl:attribute>
    </xsl:attribute-set>

</xsl:stylesheet>
```

**Key features:**
- `<xsl:value-of>` for data binding
- `<xsl:for-each>` for iteration
- `<xsl:if>` for conditionals
- `use-attribute-sets` for styling
- Handlebars expressions: `{{variable}}`

### JSON Templates

JSON templates use a declarative structure.

**Basic structure:**

```json
{
  "_stylesheet": {
    "page": {
      "size": "A4",
      "margins": "20mm"
    },
    "styles": {
      "title": {
        "fontSize": "24pt",
        "fontWeight": "bold"
      }
    }
  },
  "_template": {
    "type": "Block",
    "children": [
      {
        "type": "Paragraph",
        "styleNames": ["title"],
        "children": [
          {"type": "Text", "content": "{{title}}"}
        ]
      }
    ]
  }
}
```

**Key features:**
- `_stylesheet` for page setup and styles
- `_template` for document structure
- Handlebars expressions: `{{variable}}`
- `"each"` property for iteration
- Conditional styling with Handlebars helpers

See [TEMPLATES.md](TEMPLATES.md) for complete syntax reference.

---

## Configuration Options

### Resource Providers

Control how assets (images, fonts) are loaded.

#### FilesystemResourceProvider

Default provider, loads from disk.

```rust
use petty::resource::FilesystemResourceProvider;

let resources = FilesystemResourceProvider::new("./assets");
let pipeline = PipelineBuilder::new()
    .with_template_file("template.xsl")?
    .with_resource_provider(Arc::new(resources))
    .build()?;
```

#### InMemoryResourceProvider

Loads from memory (WASM-compatible).

```rust
use petty::resource::InMemoryResourceProvider;

let resources = InMemoryResourceProvider::new();
resources.add("logo.png", include_bytes!("logo.png").to_vec())?;
resources.add("banner.jpg", banner_bytes)?;

let pipeline = PipelineBuilder::new()
    .with_template_file("template.xsl")?
    .with_resource_provider(Arc::new(resources))
    .build()?;
```

### Executors

Control parallelism and threading.

#### SyncExecutor

Single-threaded, sequential execution.

```rust
use petty::executor::{SyncExecutor, ExecutorImpl};

let pipeline = PipelineBuilder::new()
    .with_template_file("template.xsl")?
    .with_executor(ExecutorImpl::Sync(SyncExecutor::new()))
    .build()?;
```

**Use cases:**
- WASM/browser environments
- Debugging
- Low-memory systems

#### RayonExecutor (feature: `rayon-executor`)

Parallel execution using work-stealing thread pool.

```rust
#[cfg(feature = "rayon-executor")]
use petty::executor::{RayonExecutor, ExecutorImpl};

let pipeline = PipelineBuilder::new()
    .with_template_file("template.xsl")?
    .with_executor(ExecutorImpl::Rayon(RayonExecutor::new()))
    .build()?;
```

**Use cases:**
- Batch processing (hundreds of invoices)
- Server environments
- Maximum throughput

### Generation Modes

#### SinglePassStreaming (Default)

Fast, memory-efficient, single-pass rendering.

```rust
use petty::pipeline::GenerationMode;

let pipeline = PipelineBuilder::new()
    .with_template_file("template.xsl")?
    .with_generation_mode(GenerationMode::SinglePassStreaming)
    .build()?;
```

**Features:**
- ✅ Fast rendering
- ✅ Low memory usage
- ✅ Streams to output immediately
- ❌ No table of contents
- ❌ No page number references
- ❌ No cross-references

**Use cases:**
- Simple documents
- High-volume generation
- Real-time generation

#### MetadataGenerating

Two-pass rendering with metadata support.

```rust
use petty::pipeline::GenerationMode;

let pipeline = PipelineBuilder::new()
    .with_template_file("template.xsl")?
    .with_generation_mode(GenerationMode::MetadataGenerating)
    .build()?;
```

**Features:**
- ✅ Table of contents
- ✅ Page number references
- ✅ Index generation
- ✅ Internal hyperlinks
- ⚠️ Higher memory usage (two passes)
- ⚠️ Slower (renders twice)

**Use cases:**
- Books with TOC
- Reports with index
- Documents with cross-references

---

## Advanced Features

### Table of Contents

Requires `MetadataGenerating` mode.

**Template (XSLT):**

```xml
<xsl:template match="/">
    <!-- Table of contents -->
    <toc-entries/>

    <page-break/>

    <!-- Headings with anchors -->
    <heading level="1" id="section1">
        <xsl:value-of select="section1/title"/>
    </heading>

    <heading level="2" id="subsection1">
        <xsl:value-of select="section1/subsection/title"/>
    </heading>
</xsl:template>
```

**Template (JSON):**

```json
{
  "_template": {
    "type": "Block",
    "children": [
      {"type": "TocEntries"},
      {"type": "PageBreak"},
      {
        "type": "Heading",
        "level": 1,
        "id": "section1",
        "children": [{"type": "Text", "content": "{{title}}"}]
      }
    ]
  }
}
```

### Page Numbers

Use special variables in templates:

- `{{page_num}}` - Current page number
- `{{total_pages}}` - Total page count (MetadataGenerating only)

**Example:**

```xml
<fo:simple-page-master master-name="A4"
                       page-width="210mm" page-height="297mm"
                       margin="20mm">
    <fo:footer>
        <p>Page {{page_num}} of {{total_pages}}</p>
    </fo:footer>
</fo:simple-page-master>
```

### Internal Links

Link to anchors within the document.

```xml
<!-- Create anchor -->
<heading level="1" id="chapter2">Chapter 2</heading>

<!-- Link to anchor -->
<hyperlink target-id="chapter2">
    <p>See Chapter 2</p>
</hyperlink>
```

### Index Generation

Mark terms for indexing:

```xml
<p>
    This is about <index-marker term="Rust">Rust</index-marker>.
</p>

<!-- Generate index at end -->
<index-entries/>
```

### Custom Fonts

Add fonts from files:

```rust
let pipeline = PipelineBuilder::new()
    .with_template_file("template.xsl")?
    .with_font_file("fonts/Roboto-Regular.ttf")?
    .with_font_file("fonts/Roboto-Bold.ttf")?
    .build()?;
```

Or in-memory (WASM):

```rust
use petty_core::traits::InMemoryFontProvider;
use petty_core::core::style::{FontWeight, FontStyle};

let fonts = InMemoryFontProvider::new();
fonts.add_font(
    "Roboto",
    FontWeight::Normal,
    FontStyle::Normal,
    include_bytes!("fonts/Roboto-Regular.ttf").to_vec()
)?;

// Use in pipeline builder...
```

---

## Error Handling

Petty uses a comprehensive error type: `PipelineError`.

```rust
use petty::PipelineError;

match pipeline.generate_to_file(data, "output.pdf") {
    Ok(()) => println!("Success!"),
    Err(PipelineError::TemplateNotSet) => {
        eprintln!("No template configured");
    }
    Err(PipelineError::Io(e)) => {
        eprintln!("I/O error: {}", e);
    }
    Err(PipelineError::ParseError(e)) => {
        eprintln!("Template parse error: {}", e);
    }
    Err(e) => {
        eprintln!("Error: {}", e);
    }
}
```

### Error Types

| Error | Description |
|-------|-------------|
| `TemplateNotSet` | Builder called without setting template |
| `Io(io::Error)` | File I/O error |
| `ParseError(String)` | Template parsing error |
| `RenderError(String)` | PDF rendering error |
| `LayoutError(String)` | Layout computation error |
| `FontError` | Font loading/resolution error |
| `ResourceError` | Asset loading error |

---

## Performance Tuning

### Batch Processing

For high-volume generation, use the Rayon executor:

```rust
use petty::executor::{RayonExecutor, ExecutorImpl};

let pipeline = PipelineBuilder::new()
    .with_template_file("invoice.xsl")?
    .with_executor(ExecutorImpl::Rayon(RayonExecutor::new()))
    .build()?;

// Process thousands of invoices
let invoices: Vec<Value> = load_invoices()?;
pipeline.generate_to_file(invoices, "all_invoices.pdf")?;
```

### Memory Management

**Sequences control memory usage:**

```rust
// BAD: Single giant sequence = high memory
let data = vec![json!({
    "items": huge_array_of_10000_items
})];

// GOOD: Many small sequences = bounded memory
let data: Vec<Value> = huge_array_of_10000_items
    .into_iter()
    .map(|item| json!({"item": item}))
    .collect();
```

Each sequence is:
1. Loaded
2. Processed
3. Rendered
4. Freed

This keeps memory usage predictable regardless of document size.

### Caching

Font resolution and style computation are cached automatically:

```rust
use petty::pipeline::PipelineCacheConfig;

let cache_config = PipelineCacheConfig {
    enable_font_cache: true,  // Cache font lookups
    enable_style_cache: true, // Cache computed styles
};

let pipeline = PipelineBuilder::new()
    .with_template_file("template.xsl")?
    .with_cache_config(cache_config)
    .build()?;
```

### Streaming Output

Use `generate_to_writer` with a buffered writer:

```rust
use std::io::BufWriter;
use std::fs::File;

let file = File::create("output.pdf")?;
let writer = BufWriter::new(file);

pipeline.generate_to_writer(data, writer)?;
```

### Profiling

Enable profiling features for detailed timing:

```toml
[dependencies]
petty = { version = "0.1", features = ["profiling"] }
```

```rust
let pipeline = PipelineBuilder::new()
    .with_template_file("template.xsl")?
    .with_debug(true)  // Enable debug logging
    .build()?;
```

---

## Best Practices

### 1. Reuse Pipelines

Building a pipeline is expensive (template parsing, font loading). Reuse it:

```rust
// GOOD: Build once, use many times
let pipeline = PipelineBuilder::new()
    .with_template_file("invoice.xsl")?
    .build()?;

for batch in invoice_batches {
    pipeline.generate_to_file(batch, format!("batch_{}.pdf", i))?;
}
```

### 2. Right-Size Sequences

Find the sweet spot for your use case:

```rust
// Too small: High overhead
let data = vec![json!({"line": "one"}), json!({"line": "two"}), ...];

// Too large: High memory
let data = vec![json!({"lines": [1, 2, 3, ..., 10000]})];

// Just right: Balanced
let data = invoices.chunks(100)
    .map(|chunk| json!({"invoices": chunk}))
    .collect();
```

### 3. Choose the Right Mode

- Simple documents → `SinglePassStreaming`
- Complex documents with TOC → `MetadataGenerating`

### 4. Use System Fonts When Available

```rust
let pipeline = PipelineBuilder::new()
    .with_template_file("template.xsl")?
    .with_system_fonts()?  // Discover all system fonts
    .build()?;
```

### 5. Handle Errors Gracefully

```rust
match pipeline.generate_to_file(data, path) {
    Ok(()) => {},
    Err(e) => {
        log::error!("Failed to generate PDF: {}", e);
        // Fallback or retry logic
    }
}
```

---

## Platform-Specific Notes

### Native (Desktop/Server)

Full feature set:
- ✅ System fonts
- ✅ Filesystem access
- ✅ Multi-threading (Rayon)
- ✅ Tempfiles for efficiency

### WASM (Browser)

Restricted environment:
- ❌ No system fonts (provide fonts manually)
- ❌ No filesystem (use InMemoryResourceProvider)
- ❌ Single-threaded only
- ⚠️ Higher memory usage (no tempfiles)

See [WASM.md](WASM.md) for details.

---

## Troubleshooting

### Font not found

```
Error: FontError: Font 'Arial' not found
```

**Solution:** Add the font explicitly:

```rust
pipeline = PipelineBuilder::new()
    .with_font_file("path/to/arial.ttf")?
    .build()?;
```

Or use system fonts:

```rust
pipeline = PipelineBuilder::new()
    .with_system_fonts()?
    .build()?;
```

### Template parse error

```
Error: ParseError: unexpected token at line 5
```

**Solution:** Check template syntax. Enable debug mode for details:

```rust
RUST_LOG=petty=debug cargo run
```

### Out of memory

```
Error: allocation failed
```

**Solution:** Reduce sequence size or switch to streaming mode:

```rust
.with_generation_mode(GenerationMode::SinglePassStreaming)
```

---

## Next Steps

- [TEMPLATES.md](TEMPLATES.md) - Complete template syntax reference
- [EXAMPLES.md](EXAMPLES.md) - Common patterns and recipes
- [ARCHITECTURE.md](ARCHITECTURE.md) - How Petty works internally
- [WASM.md](WASM.md) - Using Petty in browser environments
