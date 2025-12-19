# Petty PDF Generator

**High-performance, platform-agnostic PDF generation engine for Rust**

Transform structured data (JSON, XML) into professional PDF documents using declarative templates.

[![Platform](https://img.shields.io/badge/platform-Native%20%7C%20WASM-blue)]()
[![License](https://img.shields.io/badge/license-MIT-green)]()

---

## Features

- **🚀 High Performance** - Parallel processing with Rayon, optimized for throughput
- **🌐 Platform Agnostic** - Runs on native (Linux, macOS, Windows) and WASM (browser)
- **📝 Multiple Template Formats** - XSLT and JSON template support
- **🎨 CSS-like Styling** - Familiar styling model with Flexbox layout
- **📊 Advanced Features** - TOC, page numbers, cross-references, index generation
- **💾 Predictable Memory** - Bounded memory usage through sequence-based processing
- **🔌 Pluggable Everything** - Custom executors, resource providers, data sources
- **⚡ Streaming Output** - Generate directly to files, buffers, or network streams

---

## Quick Start

### Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
petty = "0.1"
serde_json = "1.0"
```

### Hello World

**Create a template** (`hello.xsl`):

```xml
<xsl:stylesheet version="1.0"
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
                xmlns:fo="http://www.w3.org/1999/XSL/Format">

    <fo:simple-page-master page-width="8.5in" page-height="11in" margin="1in"/>

    <xsl:template match="/">
        <fo:block>
            <p font-size="24pt" font-weight="bold">Hello, {{name}}!</p>
            <p>Welcome to Petty PDF generation.</p>
        </fo:block>
    </xsl:template>

</xsl:stylesheet>
```

**Generate the PDF**:

```rust
use petty::{PipelineBuilder, PipelineError};
use serde_json::json;

fn main() -> Result<(), PipelineError> {
    // Build pipeline
    let pipeline = PipelineBuilder::new()
        .with_template_file("hello.xsl")?
        .build()?;

    // Generate PDF
    let data = vec![json!({"name": "World"})];
    pipeline.generate_to_file(data, "hello.pdf")?;

    println!("✓ Generated hello.pdf");
    Ok(())
}
```

Run it:

```bash
cargo run
```

**Output**: `hello.pdf` with "Hello, World!" styled as a heading.

---

## Use Cases

### Business Documents
- **Invoices** - Professional invoices with line items, totals, branding
- **Receipts** - Point-of-sale receipts
- **Letters** - Business correspondence with letterhead
- **Contracts** - Multi-page legal documents

### Reports
- **Financial Reports** - Quarterly/annual reports with charts and tables
- **Analytics Dashboards** - Data visualizations and metrics
- **Performance Reports** - KPIs, trends, analysis

### Publications
- **Books** - Multi-chapter books with table of contents and index
- **Manuals** - Technical documentation with cross-references
- **Newsletters** - Formatted publications

### High-Volume Generation
- **Batch Invoicing** - Thousands of invoices in parallel
- **Statement Generation** - Monthly statements for customers
- **Certificate Generation** - Personalized certificates

---

## Core Concepts

### Sequences

A **sequence** is a self-contained unit of document processing. Think of it as:
- One invoice in a batch of 1000 invoices
- One chapter in a book
- One report in a series

**Why sequences?** They provide predictable memory usage. Instead of loading an entire 1000-page document into memory, Petty processes each sequence independently.

```rust
// Process 1000 invoices - memory usage stays bounded
let invoices: Vec<Value> = (0..1000)
    .map(|i| json!({"invoice_number": i, "amount": 100.0}))
    .collect();

pipeline.generate_to_file(invoices, "all_invoices.pdf")?;
```

### Templates

Templates define document structure and styling. Petty supports two formats:

**XSLT** - XML-based, powerful transformation language:
```xml
<xsl:for-each select="items/item">
    <p>{{name}}: ${{price}}</p>
</xsl:for-each>
```

**JSON** - Declarative, easy to generate programmatically:
```json
{
  "each": "items",
  "template": {
    "type": "Paragraph",
    "children": [
      {"type": "Text", "content": "{{name}}: ${{price}}"}
    ]
  }
}
```

Both use **Handlebars** for data binding.

### Pipeline Architecture

```
┌──────────┐    ┌──────────┐    ┌────────┐    ┌────────┐    ┌─────┐
│   Data   │ →  │ Template │ →  │ Layout │ →  │ Render │ →  │ PDF │
│  (JSON)  │    │  (XSLT)  │    │(Taffy) │    │(lopdf) │    │     │
└──────────┘    └──────────┘    └────────┘    └────────┘    └─────┘
```

1. **Data** - JSON/XML input
2. **Template** - XSLT/JSON template with Handlebars
3. **Layout** - CSS Flexbox-based layout engine (Taffy)
4. **Render** - PDF generation (lopdf)

---

## Advanced Features

### Table of Contents

Automatically generate TOC with page numbers:

```xml
<toc-entries/>
<page-break/>

<heading level="1" id="chapter1">Chapter 1</heading>
<p>Content...</p>

<heading level="2" id="section1">Section 1.1</heading>
<p>More content...</p>
```

Requires `MetadataGenerating` mode:

```rust
use petty::pipeline::GenerationMode;

let pipeline = PipelineBuilder::new()
    .with_template_file("book.xsl")?
    .with_generation_mode(GenerationMode::MetadataGenerating)
    .build()?;
```

### Internal Links

Link to sections within the document:

```xml
<hyperlink target-id="chapter2">
    <p>See Chapter 2</p>
</hyperlink>

<!-- Later in document -->
<heading level="1" id="chapter2">Chapter 2</heading>
```

### Index Generation

Mark terms and generate an index:

```xml
<p>This covers <index-marker term="Rust">Rust programming</index-marker>.</p>

<!-- At end of document -->
<index-entries/>
```

### Custom Fonts

Load fonts from files or memory:

```rust
let pipeline = PipelineBuilder::new()
    .with_template_file("template.xsl")?
    .with_font_file("fonts/Roboto-Regular.ttf")?
    .with_font_file("fonts/Roboto-Bold.ttf")?
    .build()?;
```

---

## Platform Support

### Native (Desktop/Server)

Full feature set with maximum performance:

- ✅ System fonts (fontdb)
- ✅ Filesystem resources
- ✅ Multi-threading (Rayon)
- ✅ Tempfiles for memory efficiency

```rust
let pipeline = PipelineBuilder::new()
    .with_template_file("template.xsl")?
    .with_system_fonts()?  // Discover all system fonts
    .build()?;
```

### WASM (Browser)

Run in browser with some restrictions:

- ✅ Full layout and rendering
- ✅ In-memory resources
- ❌ No system fonts (provide manually)
- ❌ Single-threaded only

```rust
use petty_core::traits::InMemoryResourceProvider;

let resources = InMemoryResourceProvider::new();
resources.add("logo.png", logo_bytes)?;

let pipeline = PipelineBuilder::new()
    .with_template_string(template, "xsl")?
    .with_resource_provider(Arc::new(resources))
    .build()?;
```

See [WASM.md](WASM.md) for detailed WASM usage guide.

---

## Documentation

📚 **Complete Documentation**:

- **[USAGE.md](USAGE.md)** - Complete API reference, configuration options, best practices
- **[TEMPLATES.md](TEMPLATES.md)** - Full template syntax guide (XSLT and JSON)
- **[EXAMPLES.md](EXAMPLES.md)** - Practical examples and common patterns
- **[ARCHITECTURE.md](ARCHITECTURE.md)** - Internal design and how components work
- **[WASM.md](WASM.md)** - WebAssembly compatibility guide

---

## Examples

The `examples/` directory contains working examples:

### Running Examples

```bash
# Invoice generation
cargo run --example xslt_invoice

# Financial report
cargo run --example xslt_financial_report

# CV/Resume
cargo run --example cv

# Book with TOC
cargo run --example xslt_toc

# Performance test (1000s of documents)
cargo run --release --example performance_test
```

### Example Templates

Check out these template files:

- `templates/invoice_template.xsl` - Professional invoice
- `templates/financial_report_template.xsl` - Financial report with charts
- `templates/cv_template.xsl` - Resume/CV
- `templates/toc_template.xsl` - Book with table of contents
- `templates/report_template.json` - JSON template example

---

## Performance

Petty is designed for high-throughput PDF generation:

### Benchmarks

On an 8-core machine with Rayon executor:

| Document Type | Throughput | Notes |
|---------------|-----------|-------|
| Simple invoice (1 page) | ~150-200/sec | Single invoice per sequence |
| Complex report (10 pages) | ~20-50/sec | Multi-page with tables |
| Batch invoices | ~500-1000/sec | 1000 invoices in single PDF |

### Memory Usage

Petty uses a **bounded memory model**:

- Base (fonts, templates): ~50MB
- Per worker: ~100MB
- Per sequence (active): ~2-5MB

**Example**: Processing 10,000 invoices with 4 workers:
- Peak memory: ~500MB (bounded)
- Without sequences: Would require ~20GB (entire document in memory)

### Scaling

Throughput scales linearly with CPU cores up to ~8 cores, then plateaus due to I/O.

---

## Architecture

Petty uses a **workspace structure** to separate platform-agnostic core from platform adapters:

```
petty-core/          # Platform-agnostic (compiles to WASM)
├── core/           # Layout, styling, fonts
├── parser/         # Template parsing (XSLT, JSON)
├── render/         # PDF generation
└── traits/         # Platform abstraction traits

petty/              # Platform adapters (native)
├── executor/       # Sync, Rayon executors
├── resource/       # Filesystem, in-memory providers
├── pipeline/       # Orchestration, concurrency
└── source/         # Data source abstractions
```

See [ARCHITECTURE.md](ARCHITECTURE.md) for detailed design documentation.

---

## Configuration

### Executors

Control parallelism:

```rust
use petty::executor::{SyncExecutor, RayonExecutor, ExecutorImpl};

// Single-threaded (WASM, debugging)
let pipeline = PipelineBuilder::new()
    .with_executor(ExecutorImpl::Sync(SyncExecutor::new()))
    .build()?;

// Parallel (native, high throughput)
let pipeline = PipelineBuilder::new()
    .with_executor(ExecutorImpl::Rayon(RayonExecutor::new()))
    .build()?;
```

### Resource Providers

Control asset loading:

```rust
use petty::resource::{FilesystemResourceProvider, InMemoryResourceProvider};

// Filesystem (native)
let resources = FilesystemResourceProvider::new("./assets");

// In-memory (WASM)
let resources = InMemoryResourceProvider::new();
resources.add("logo.png", logo_bytes)?;
```

### Generation Modes

Choose rendering strategy:

```rust
use petty::pipeline::GenerationMode;

// Fast, single-pass (default)
let pipeline = PipelineBuilder::new()
    .with_generation_mode(GenerationMode::SinglePassStreaming)
    .build()?;

// Two-pass with metadata (TOC, page numbers)
let pipeline = PipelineBuilder::new()
    .with_generation_mode(GenerationMode::MetadataGenerating)
    .build()?;
```

---

## Comparison

### vs. LaTeX

**Petty**:
- ✅ Faster compilation
- ✅ JSON/XML data binding
- ✅ Programmatic generation
- ❌ Less mature typography

**LaTeX**:
- ✅ Superior typography
- ✅ Math equations
- ❌ Slower compilation
- ❌ Complex syntax

### vs. wkhtmltopdf

**Petty**:
- ✅ Native Rust, no dependencies
- ✅ Template-based
- ✅ Predictable memory
- ❌ No HTML rendering

**wkhtmltopdf**:
- ✅ HTML/CSS support
- ❌ External process
- ❌ Memory issues with large docs
- ❌ Deprecated

### vs. Typst

**Petty**:
- ✅ JSON data binding
- ✅ Parallel batch processing
- ✅ Programmatic API
- ❌ Smaller ecosystem

**Typst**:
- ✅ Modern syntax
- ✅ Fast compilation
- ❌ Less data-oriented
- ❌ Newer (smaller community)

---

## Contributing

Contributions welcome! Areas of interest:

- New template features
- Performance optimizations
- Additional platform support
- Documentation improvements
- Example templates

---

## License

MIT License - see LICENSE file for details.

---

## Resources

- **Documentation**: See docs in this repository
- **Issues**: https://github.com/yourusername/petty/issues
- **Examples**: `examples/` directory

---

## Roadmap

- [ ] SVG rendering support
- [ ] HTML output backend
- [ ] Plugin system for custom nodes
- [ ] Incremental template compilation
- [ ] Enhanced WASM bindings (wasm-bindgen)
- [ ] More built-in templates

---

## Acknowledgments

Petty is built on excellent Rust libraries:

- [Taffy](https://github.com/DioxusLabs/taffy) - CSS layout engine
- [lopdf](https://github.com/J-F-Liu/lopdf) - PDF generation
- [rustybuzz](https://github.com/RazrFalcon/rustybuzz) - Text shaping
- [Handlebars](https://github.com/sunng87/handlebars-rust) - Template engine
- [Rayon](https://github.com/rayon-rs/rayon) - Parallel processing

---

**Built with ❤️ in Rust**
