use petty::style::dimension::{Dimension, Margins};
use petty::style::flex::JustifyContent;
use petty::style::font::FontWeight;
use petty::style::stylesheet::PageLayout;
use petty::style::text::TextAlign;
use petty::templating::builders::*;
use petty::templating::{h1, h2, h3, p, subtitle, Template};
use petty::{PdfBackend, PipelineBuilder, PipelineError};
use rand::prelude::*;
use rand::rngs::StdRng;
use rand::SeedableRng;
use serde_json::{json, Value};
use std::env;
use std::time::Instant;
use petty::types_base::color::Color;

#[cfg(feature = "dhat-heap")]
#[global_allocator]
static ALLOCATOR: dhat::Alloc = dhat::Alloc;

const MOCK_USERS: &[&str] = &["Alice", "Bob", "Charlie", "Diana", "Eve", "Frank"];
const MOCK_ITEMS: &[&str] = &[
    "Standard Service Fee",
    "Premium Support Package",
    "Data Processing Unit",
    "Cloud Storage (1TB)",
    "API Access Key",
    "Consulting Hour",
    "Hardware Rental",
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

/// Creates the entire invoice template programmatically using the new builder API.
fn create_invoice_template() -> Template {
    let table_header_text_style = |text: &str| {
        p(text)
            .font_weight(FontWeight::Bold)
            .color(Color::gray(136))
    };

    let summary_row = |label: &str, value: &str| {
        Flex::new()
            .justify_content(JustifyContent::SpaceBetween)
            .child(p(label))
            .child(
                Paragraph::new(
                    Span::new()
                        .child(Text::new("$"))
                        .child(Text::new(value)),
                )
                    .text_align(TextAlign::Right),
            )
    };

    let root = Block::new()
        .font_size(10.0)
        .color(Color::gray(34))
        // --- Header ---
        .child(
            Flex::new()
                .justify_content(JustifyContent::SpaceBetween)
                .padding(Margins { bottom: 20.0, ..Default::default() })
                .margin(Margins { bottom: 20.0, ..Default::default() })
                .border_bottom((1.0, "solid", Color::gray(221)).into())
                .child(
                    Block::new()
                        .child(h1("PETTY INC."))
                        .child(subtitle("Document Generation Services")),
                )
                .child(
                    Block::new()
                        .text_align(TextAlign::Right)
                        .child(p("123 Rust Lane"))
                        .child(p("Ferris City, RS 10101"))
                        .child(p("contact@petty.rs")),
                ),
        )
        // --- Invoice Details ---
        .child(
            Flex::new()
                .justify_content(JustifyContent::SpaceBetween)
                .margin(Margins { bottom: 30.0, ..Default::default() })
                .child(
                    Block::new()
                        .child(h3("BILL TO"))
                        .child(p("{{user.name}}"))
                        .child(p("Account: {{user.account}}")),
                )
                .child(
                    Block::new()
                        .text_align(TextAlign::Right)
                        .child(h2("Invoice #{{id}}"))
                        .child(p("Date: 2023-10-27")),
                ),
        )
        // --- Items Table ---
        .child(
            Table::new()
                .margin(Margins { bottom: 20.0, ..Default::default() })
                .column(Column::new().width(Dimension::Percent(55.0))) // Description
                .column(Column::new().width(Dimension::Percent(15.0))) // Quantity
                .column(Column::new().width(Dimension::Percent(15.0))) // Unit Price
                .column(Column::new().width(Dimension::Percent(15.0))) // Line Total
                .header_row(
                    Row::new()
                        .cell(Cell::new().padding(Margins::y(6.0)).child(table_header_text_style("Description")))
                        .cell(Cell::new().padding(Margins::y(6.0)).child(table_header_text_style("Quantity").text_align(TextAlign::Right)))
                        .cell(Cell::new().padding(Margins::y(6.0)).child(table_header_text_style("Unit Price").text_align(TextAlign::Right)))
                        .cell(Cell::new().padding(Margins::y(6.0)).child(table_header_text_style("Total").text_align(TextAlign::Right))),
                )
                .child(
                    Each::new(
                        "items",
                        Row::new()
                            .cell(Cell::new().child(p("{{this.description}}")))
                            .cell(Cell::new().child(p("{{this.quantity}}").text_align(TextAlign::Right)))
                            .cell(Cell::new().child(p("${{this.price}}").text_align(TextAlign::Right)))
                            .cell(Cell::new().child(p("${{this.line_total}}").text_align(TextAlign::Right))),
                    ),
                ),
        )
        // --- Summary Section ---
        .child(
            Flex::new()
                .child(Block::new().flex_grow(1.0)) // Spacer
                .child(
                    Block::new()
                        .width(Dimension::Pt(250.0))
                        .child(summary_row("Subtotal:", "{{summary.total}}"))
                        .child(summary_row("Tax (8%):", "{{summary.tax}}"))
                        .child(
                            Flex::new()
                                .justify_content(JustifyContent::SpaceBetween)
                                .margin(Margins { top: 10.0, ..Default::default() })
                                .padding(Margins { top: 10.0, ..Default::default() })
                                .border_top((1.0, "solid", Color::gray(238)).into())
                                .child(p("Grand Total:").font_weight(FontWeight::Bold))
                                .child(
                                    Paragraph::new(
                                        Span::new()
                                            .child(Text::new("$"))
                                            .child(Text::new("{{summary.grand_total}}")),
                                    )
                                        .text_align(TextAlign::Right)
                                        .font_weight(FontWeight::Bold),
                                ),
                        ),
                ),
        )
        // --- Footer ---
        .child(
            p("Payment is due within 30 days.")
                .text_align(TextAlign::Center)
                .color(Color::gray(106))
                .margin(Margins { top: 20.0, ..Default::default() }),
        );

    Template::new(root).add_page_master(
        "default",
        PageLayout {
            margins: Some(Margins::all(36.0)),
            ..Default::default()
        },
    )
}

fn main() -> Result<(), PipelineError> {
    #[cfg(feature = "dhat-heap")]
    let _profiler = dhat::Profiler::new_heap();

    // Initialize the logger to enable debug messages.
    if env::var("RUST_LOG").is_err() {
        unsafe { env::set_var("RUST_LOG", "petty=info"); }
    }
    env_logger::init();

    if cfg!(debug_assertions) {
        println!("\nWARNING: Running in debug build. For accurate results, run with `--release`.\n");
    }
    println!("Running Code-based Template Performance Test Example...");

    let args: Vec<String> = env::args().collect();
    let num_records = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(500);
    let max_items = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(15);
    println!(
        "Configuration: {} pages, up to {} table rows per page.",
        num_records, max_items
    );

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
    println!(
        "Starting PDF generation for {} records to {}...",
        num_records, output_path
    );
    let start_time = Instant::now();

    pipeline.generate_to_file(data_iterator, output_path)?;

    let duration = start_time.elapsed();
    println!("\nSuccess! Generated {}", output_path);
    println!(
        "Total time taken: {:.2} seconds for {} records.",
        duration.as_secs_f64(),
        num_records
    );
    if num_records > 0 {
        println!(
            "Average time per record: {:.2} ms.",
            duration.as_millis() as f64 / num_records as f64
        );
    }
    Ok(())
}