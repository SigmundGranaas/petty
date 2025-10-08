use petty::core::style::color::Color;
use petty::core::style::dimension::{Dimension, Margins};
use petty::core::style::font::FontWeight;
use petty::core::style::stylesheet::{ElementStyle, PageLayout};
use petty::core::style::text::TextAlign;
use petty::templating::builders::*;
use petty::templating::Template;
use petty::{PdfBackend, PipelineBuilder, PipelineError};
use rand::prelude::*;
use rand::rngs::StdRng;
use rand::SeedableRng;
use serde_json::{json, Value};
use std::env;
use std::time::Instant;

#[cfg(feature = "dhat-heap")]
#[global_allocator]
static ALLOCATOR: dhat::Alloc = dhat::Alloc;

const MOCK_USERS: &[&str] = &["Alice", "Bob", "Charlie", "Diana", "Eve", "Frank"];
const MOCK_ITEMS: &[&str] = &[
    "Standard Service Fee", "Premium Support Package", "Data Processing Unit",
    "Cloud Storage (1TB)", "API Access Key", "Consulting Hour", "Hardware Rental",
];

/// Generates a lazy iterator that produces one complex JSON record on demand.
fn generate_perf_test_data_iter(
    num_records: usize,
    max_items_per_record: usize,
) -> impl Iterator<Item = Value> {
    println!("Generating {} records via an iterator...", num_records);
    let mut rng = StdRng::from_seed(Default::default());
    (0..num_records).map(move |i| {
        let num_items = rng.random_range(2..=max_items_per_record);
        let mut total = 0.0;
        let items: Vec<Value> = (0..num_items)
            .map(|_| {
                let quantity = rng.random_range(1..=10);
                let price: f64 = rng.random_range(10.0..500.0);
                // Round price to 2 decimal places
                let price_rounded = (price * 100.0).round() / 100.0;
                // Calculate and round line_total to be safe from floating point inaccuracies
                let line_total = (quantity as f64 * price_rounded * 100.0).round() / 100.0;
                total += line_total;
                json!({
                    "description": MOCK_ITEMS.choose(&mut rng).unwrap_or(&""),
                    "quantity": quantity,
                    "price": format!("{:.2}", price_rounded),
                    "line_total": format!("{:.2}", line_total)
                })
            })
            .collect();

        // Round all summary values to 2 decimal places for consistent financial data
        let subtotal = (total * 100.0).round() / 100.0;
        let tax = (subtotal * 0.08 * 100.0).round() / 100.0;
        let grand_total = subtotal + tax;

        json!({
            "id": 10000 + i,
            "user": { "name": MOCK_USERS.choose(&mut rng).unwrap_or(&""), "account": format!("ACC-{}", rng.random_range(100000..999999)) },
            "items": items,
            "summary": {
                "total": format!("{:.2}", subtotal),
                "tax": format!("{:.2}", tax),
                "grand_total": format!("{:.2}", grand_total)
            }
        })
    })
}

/// Creates the entire invoice template programmatically using the builder API.
fn create_invoice_template() -> Template {
    // This function defines the layout and styling for a single invoice.
    // It will be instantiated for each data record from the iterator.
    let root = Block::new()
        .style_name("page")
        // --- Header ---
        .child(
            Flex::new()
                .style_name("header")
                .child(
                    Block::new()
                        .child(Paragraph::new().text("PETTY INC.").style_name("h1"))
                        .child(Paragraph::new().text("Document Generation Services").style_name("subtitle")),
                )
                .child(
                    Block::new()
                        .style_name("align-right")
                        .child(Paragraph::new().text("123 Rust Lane"))
                        .child(Paragraph::new().text("Ferris City, RS 10101"))
                        .child(Paragraph::new().text("contact@petty.rs")),
                ),
        )
        // --- Invoice Details ---
        .child(
            Flex::new()
                .style_name("details-section")
                .child(
                    Block::new()
                        .child(Paragraph::new().text("BILL TO").style_name("h3"))
                        .child(Paragraph::new().text("{{user.name}}"))
                        .child(Paragraph::new().text("Account: {{user.account}}")),
                )
                .child(
                    Block::new()
                        .style_name("align-right")
                        .child(
                            Paragraph::new()
                                .style_name("h2")
                                .child(Span::new().text("Invoice #{{id}}")),
                        )
                        .child(Paragraph::new().text("Date: 2023-10-27")),
                ),
        )
        // --- Items Table ---
        .child(
            Table::new()
                .style_name("items-table")
                .column(Column::new().width(Dimension::Percent(55.0))) // Description
                .column(Column::new().width(Dimension::Percent(15.0))) // Quantity
                .column(Column::new().width(Dimension::Percent(15.0))) // Unit Price
                .column(Column::new().width(Dimension::Percent(15.0))) // Line Total
                .header_row(
                    Row::new()
                        .cell(Cell::new().style_name("th-cell").child(Paragraph::new().text("Description").style_name("th-text")))
                        .cell(Cell::new().style_name("th-cell").child(Paragraph::new().text("Quantity").style_name("th-text-right")))
                        .cell(Cell::new().style_name("th-cell").child(Paragraph::new().text("Unit Price").style_name("th-text-right")))
                        .cell(Cell::new().style_name("th-cell").child(Paragraph::new().text("Total").style_name("th-text-right"))),
                )
                // Use `Each` to iterate over the `items` array from the JSON data
                .child(
                    Each::new(
                        "items",
                        Row::new()
                            .cell(Cell::new().child(Paragraph::new().text("{{this.description}}")))
                            .cell(Cell::new().child(Paragraph::new().text("{{this.quantity}}").style_name("align-right")))
                            .cell(Cell::new().child(Paragraph::new().text("${{this.price}}").style_name("align-right")))
                            .cell(Cell::new().child(Paragraph::new().text("${{this.line_total}}").style_name("align-right"))),
                    ),
                ),
        )
        // --- Summary Section ---
        .child(
            Flex::new() // This Flex pushes the summary box to the right
                .child(Block::new()) // Spacer
                .child(
                    Block::new()
                        .style_name("summary-box")
                        .child(
                            Flex::new()
                                .child(Paragraph::new().text("Subtotal:"))
                                .child(
                                    Paragraph::new()
                                        .style_name("align-right")
                                        // FIX: Separate static text and dynamic content
                                        .child(Text::new("$"))
                                        .child(Text::new("{{summary.total}}"))
                                ),
                        )
                        .child(
                            Flex::new()
                                .child(Paragraph::new().text("Tax (8%):"))
                                .child(
                                    Paragraph::new()
                                        .style_name("align-right")
                                        .child(Text::new("$"))
                                        .child(Text::new("{{summary.tax}}"))
                                ),
                        )
                        .child(
                            Flex::new()
                                .style_name("grand-total")
                                .child(Paragraph::new().text("Grand Total:"))
                                .child(
                                    Paragraph::new()
                                        .style_name("align-right")
                                        .child(Text::new("$"))
                                        .child(Text::new("{{summary.grand_total}}"))
                                ),
                        ),
                ),
        )
        // --- Footer ---
        .child(
            Paragraph::new()
                .text("Thank you for your business! Payment is due within 30 days.")
                .style_name("footer"),
        );

    Template::new(root)
        .add_page_master("default", PageLayout {
            margins: Some(Margins::all(36.0)), // 0.5 inch
            ..Default::default()
        })
        .add_style("page", ElementStyle { font_size: Some(10.0), color: Some(Color::gray(34)), ..Default::default() })
        .add_style("header", ElementStyle { padding: Some(Margins { bottom: 20.0, ..Default::default() }), border_bottom: Some((1.0, "solid", Color::gray(221)).into()), margin: Some(Margins { bottom: 20.0, ..Default::default() }), ..Default::default() })
        .add_style("h1", ElementStyle { font_size: Some(28.0), font_weight: Some(FontWeight::Bold), color: Some(Color::gray(34)), ..Default::default() })
        .add_style("subtitle", ElementStyle { font_size: Some(10.0), color: Some(Color::gray(102)), ..Default::default() })
        .add_style("h2", ElementStyle { font_size: Some(18.0), font_weight: Some(FontWeight::Bold), ..Default::default() })
        .add_style("h3", ElementStyle { font_size: Some(9.0), font_weight: Some(FontWeight::Bold), color: Some(Color::gray(136)), ..Default::default() })
        .add_style("align-right", ElementStyle { text_align: Some(TextAlign::Right), ..Default::default() })
        .add_style("details-section", ElementStyle { margin: Some(Margins { bottom: 30.0, ..Default::default() }), ..Default::default() })
        .add_style("items-table", ElementStyle { margin: Some(Margins { bottom: 20.0, ..Default::default() }), ..Default::default() })

        .add_style("th-cell", ElementStyle {
            padding: Some(Margins::y(6.0)),
            ..Default::default()
        })
        .add_style("th-text", ElementStyle {
            font_weight: Some(FontWeight::Bold),
            color: Some(Color::gray(136)),
            ..Default::default()
        })
        .add_style("th-text-right", ElementStyle {
            text_align: Some(TextAlign::Right),
            font_weight: Some(FontWeight::Bold),
            color: Some(Color::gray(136)),
            ..Default::default()
        })

        .add_style("summary-box", ElementStyle {
            width: Some(Dimension::Pt(250.0)),
            color: Some(Color::gray(34)),
            padding: Some(Margins::y(10.0)),
            border: Some((1.0, "solid", Color::gray(238)).into()),
            ..Default::default()
        })
        .add_style("grand-total", ElementStyle { font_weight: Some(FontWeight::Bold), margin: Some(Margins { top: 10.0, left: 10.0, ..Default::default() }), padding: Some(Margins { top: 10.0, ..Default::default() }), border_top: Some((1.0, "solid", Color::gray(238)).into()), ..Default::default() })

        .add_style("footer", ElementStyle {
            text_align: Some(TextAlign::Center),
            color: Some(Color::gray(106)),
            margin: Some(Margins { top: 20.0, ..Default::default() }),
            ..Default::default()
        })
}

fn main() -> Result<(), PipelineError> {
    #[cfg(feature = "dhat-heap")]
    let _profiler = dhat::Profiler::new_heap();

    // Initialize the logger to enable debug messages.
    if env::var("RUST_LOG").is_err() {
        unsafe {
            env::set_var("RUST_LOG", "petty=info");
        }
    }
    env_logger::init();

    if cfg!(debug_assertions) {
        println!("\nWARNING: Running in debug build. For accurate results, run with `--release`.\n");
    }
    println!("Running Code-based Template Performance Test Example...");

    let args: Vec<String> = env::args().collect();
    let num_records = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(500);
    let max_items = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(15);
    println!("Configuration: {} pages, up to {} table rows per page.", num_records, max_items);

    println!("✓ Building template from Rust code...");
    let template = create_invoice_template();
    let data_iterator = generate_perf_test_data_iter(num_records, max_items);
    println!("✓ Data iterator created.");

    // The builder now receives the template object directly.
    let pipeline = PipelineBuilder::new()
        .with_template_object(template)?
        .with_pdf_backend(PdfBackend::LopdfParallel)
        .with_debug(false)
        .build()?;
    println!("✓ Pipeline built with JSON engine.");

    let output_path = "performance_test_output_from_code.pdf";
    println!("Starting PDF generation for {} records to {}...", num_records, output_path);
    let start_time = Instant::now();

    pipeline.generate_to_file(data_iterator, output_path)?;

    let duration = start_time.elapsed();
    println!("\nSuccess! Generated {}", output_path);
    println!("Total time taken: {:.2} seconds for {} records.", duration.as_secs_f64(), num_records);
    if num_records > 0 {
        println!("Average time per record: {:.2} ms.", duration.as_millis() as f64 / num_records as f64);
    }
    Ok(())
}