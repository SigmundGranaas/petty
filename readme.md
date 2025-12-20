# Petty PDF Generator

**High-performance PDF generation engine for Rust**

Transform structured data (JSON, XML) into PDF documents using declarative templates.

---

## Quick Start

### Installation

```toml
[dependencies]
petty = "0.1"
serde_json = "1.0"
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

### Minimal Example

```rust
use petty::PipelineBuilder;
use serde_json::json;
use std::io::Cursor;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let template = r#"{
        "_stylesheet": {
            "defaultPageMaster": "default",
            "pageMasters": { "default": { "size": "A4", "margins": "2cm" } },
            "styles": { "default": { "font-family": "Helvetica", "font-size": "12pt" } }
        },
        "_template": {
            "type": "Block",
            "children": [{
                "type": "Paragraph",
                "children": [{ "type": "Text", "content": "Hello, {{name}}!" }]
            }]
        }
    }"#;

    let pipeline = PipelineBuilder::new()
        .with_template_source(template, "json")?
        .build()?;

    let data = vec![json!({"name": "World"})];
    let output = Cursor::new(Vec::new());

    let result = pipeline.generate(data.into_iter(), output).await?;
    std::fs::write("hello.pdf", result.into_inner())?;

    println!("Generated hello.pdf");
    Ok(())
}
```

---

## Performance

Tested on a 20-core machine (AMD Ryzen, Ubuntu Linux):

```bash
cargo run --release --example performance_test 10000
```

| Records | Time | Throughput |
|---------|------|------------|
| 500 | 0.05s | 9,517 records/sec |
| 1,000 | 0.07s | 14,330 records/sec |
| 5,000 | 0.33s | 14,944 records/sec |
| 10,000 | 0.65s | 15,424 records/sec |

Worker threads auto-scale based on CPU cores (19 workers on 20-core machine).

**To reproduce:**
```bash
cargo run --release --example performance_test 5000
```

---

## Template Formats

### JSON Templates

```json
{
    "_stylesheet": {
        "defaultPageMaster": "default",
        "pageMasters": { "default": { "size": "A4", "margins": "2cm" } },
        "styles": { "heading": { "font-size": "18pt", "font-weight": "bold" } }
    },
    "_template": {
        "type": "Block",
        "children": [
            {
                "type": "Paragraph",
                "style": "heading",
                "children": [{ "type": "Text", "content": "{{title}}" }]
            }
        ]
    }
}
```

### XSLT Templates

```xml
<?xml version="1.0" encoding="UTF-8"?>
<xsl:stylesheet version="1.0"
    xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
    xmlns:fo="http://www.w3.org/1999/XSL/Format">

    <fo:simple-page-master page-width="210mm" page-height="297mm" margin="2cm"/>

    <xsl:template match="/">
        <fo:block font-size="18pt" font-weight="bold">
            <xsl:value-of select="document/title"/>
        </fo:block>
    </xsl:template>
</xsl:stylesheet>
```

---

## API Reference

### PipelineBuilder

```rust
use petty::{PipelineBuilder, GenerationMode, ProcessingMode, PdfBackend};

let pipeline = PipelineBuilder::new()
    // Template (required - choose one)
    .with_template_file("template.xsl")?       // Load from file
    .with_template_source(source, "json")?     // Load from string

    // Fonts
    .with_system_fonts(true)                   // Load system fonts
    .with_font_dir("./fonts")?                 // Add font directory

    // Configuration
    .with_generation_mode(GenerationMode::Auto)           // Auto or ForceStreaming
    .with_processing_mode(ProcessingMode::Standard)       // Standard, WithMetrics, or Adaptive
    .with_pdf_backend(PdfBackend::Lopdf)                  // Lopdf or LopdfParallel
    .with_worker_count(4)                                 // Manual worker count

    .build()?;
```

### Generation Modes

| Mode | Description |
|------|-------------|
| `GenerationMode::Auto` | Automatically selects best pipeline (default) |
| `GenerationMode::ForceStreaming` | Force single-pass streaming (faster, no TOC support) |

### Processing Modes

| Mode | Description |
|------|-------------|
| `ProcessingMode::Standard` | No metrics, fixed workers (default) |
| `ProcessingMode::WithMetrics` | Collect throughput/queue metrics |
| `ProcessingMode::Adaptive` | Dynamic worker scaling based on load |

---

## Examples

The `examples/` directory contains working examples:

```bash
# CV/Resume generation
cargo run --release --example cv

# Invoice generation
cargo run --release --example xslt_invoice

# JSON template report
cargo run --release --example json_report

# Performance benchmark
cargo run --release --example performance_test 1000

# Table of contents
cargo run --release --example toc
```

### Available Templates

| File | Description |
|------|-------------|
| `templates/cv_template.xsl` | Resume/CV |
| `templates/invoice_template.xsl` | Invoice |
| `templates/report_template.json` | JSON template example |
| `templates/toc_template.xsl` | Document with table of contents |
| `templates/perf_test_template.xsl` | Performance testing |

---

## Features

- **Parallel Processing**: Auto-scales workers based on CPU cores
- **Template Formats**: XSLT and JSON with Handlebars data binding
- **Streaming Output**: Write directly to files or buffers
- **Table of Contents**: Auto-generated TOC with page numbers
- **Internal Links**: Cross-references within documents

---

## Documentation

- **[USAGE.md](USAGE.md)** - API reference and configuration
- **[TEMPLATES.md](TEMPLATES.md)** - Template syntax guide
- **[EXAMPLES.md](EXAMPLES.md)** - Practical examples
- **[ARCHITECTURE.md](ARCHITECTURE.md)** - Internal design
- **[WASM.md](WASM.md)** - WebAssembly support

---

## Project Structure

```
petty/
├── src/pipeline/           # Core pipeline orchestration
├── crates/
│   ├── core/              # Layout, fonts, error handling
│   ├── json-template/     # JSON template parser
│   ├── xslt/              # XSLT template parser
│   ├── render-lopdf/      # PDF rendering
│   └── wasm/              # WebAssembly bindings
├── templates/             # Example templates
└── examples/              # Working examples
```

---

## License

MIT License

---

## Acknowledgments

Built on:
- [Taffy](https://github.com/DioxusLabs/taffy) - CSS layout engine
- [lopdf](https://github.com/J-F-Liu/lopdf) - PDF generation
- [rustybuzz](https://github.com/RazrFalcon/rustybuzz) - Text shaping
- [Handlebars](https://github.com/sunng87/handlebars-rust) - Template engine
