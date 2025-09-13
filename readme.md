# Petty PDF Generator

Petty is a high-performance document generation engine designed to transform structured data (e.g., JSON) into professional-quality PDF documents using declarative templates. It is built on a concurrent pipeline architecture that processes a stream of self-contained document chunks, enabling sophisticated layout features while maintaining a low and predictable memory footprint.

## Key Concepts

- **Sequence**: A logical, self-contained part of a document (like an invoice or a chapter) that is processed in memory as a single unit. This gives the user explicit control over the memory-for-features trade-off.
- **Templates**: Document structure can be defined using a simple JSON format or a more powerful XSLT-like syntax.
- **Styling**: A simple, CSS-like styling model is used to control the appearance of elements. Styles can be defined in a JSON stylesheet or extracted from an XSLT file.
- **Intermediate Representation (IR)**: The engine uses a rich, semantic tree (`IRNode`) as its canonical representation, decoupling input syntax from layout logic.

## How to Run

### Command-Line Interface

The main binary acts as a simple CLI tool for PDF generation using the XSLT engine.

```bash
# This command is for the XSLT engine.
cargo run --release -- \
  templates/invoice_template.xsl \
  data/invoice_data.json \
  output/xslt_invoices.pdf