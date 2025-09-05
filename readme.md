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