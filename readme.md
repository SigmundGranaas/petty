# Petty PDF Generator

Petty is a Rust-based document generation engine inspired by Apache FOP. It transforms structured JSON data into PDFs using a stylesheet that defines page layout, styling, and data-driven templates.

## Key Concepts

- **Stylesheet**: A JSON file that defines everything about the document: page size, margins, styles, and templates.
- **Data File**: A JSON file containing the content to be populated into the document.
- **Page Sequences**: A core concept where a template is applied to each item in a data array, allowing for the generation of multi-page documents like reports, invoices, or catalogs where each major section (e.g., each invoice) starts on a new page.

## How to Run

### Command-Line Interface

The main binary acts as a simple CLI tool for PDF generation.

```bash
cargo run --release -- \
  templates/invoice_stylesheet.json \
  data/invoice_data.json \
  output/invoices.pdf



### Running Examples

The project includes several examples to demonstrate its capabilities. You can run them using Cargo:

```bash
# Generate a multi-page report from a single block of text (JSON)
cargo run --example simple_report

# Generate a separate invoice for each customer in the data file (JSON)
cargo run --example invoice_per_customer

# Generate a complex financial statement with dynamic row styling (JSON)
cargo run --example financial_report

# Generate invoices using the XSLT templating engine
cargo run --example xslt_invoice

# --- Performance Testing ---

# Run a performance test with a large, generated dataset (JSON Engine).
# You can pass the number of pages as an argument.
cargo run --release --example performance_test -- 1000

# Run the same performance test using the XSLT Engine for comparison.
cargo run --release --example performance_test_xslt -- 1000
```