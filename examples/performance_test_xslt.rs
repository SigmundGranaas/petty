use petty::{PipelineBuilder, PipelineError};
use rand::prelude::*;
use rand::rngs::StdRng;
use rand::SeedableRng;
use serde_json::{json, Value};
use std::env;
use std::path::PathBuf;
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
    let mut rng = StdRng::from_os_rng();
    (0..num_records).map(move |i| {
        let num_items = rng.random_range(2..=max_items_per_record);
        let mut total = 0.0;
        let items: Vec<Value> = (0..num_items)
            .map(|_| {
                let quantity = rng.random_range(1..=10);
                let price: f64 = rng.random_range(10.0..500.0);
                let line_total = quantity as f64 * price;
                total += line_total;
                json!({
                    "description": MOCK_ITEMS.choose(&mut rng).unwrap_or(&""),
                    "quantity": quantity, "price": price, "line_total": line_total
                })
            })
            .collect();
        json!({
            "id": 10000 + i,
            "user": { "name": MOCK_USERS.choose(&mut rng).unwrap_or(&""), "account": format!("ACC-{}", rng.random_range(100000..999999)) },
            "items": items,
            "summary": { "total": total, "tax": total * 0.08, "grand_total": total * 1.08 }
        })
    })
}

fn main() -> Result<(), PipelineError> {
    #[cfg(feature = "dhat-heap")]
    let _profiler = dhat::Profiler::new_heap();

    // Initialize the logger to enable debug messages.
    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "petty=warn");
    }
    env_logger::init();

    if cfg!(debug_assertions) {
        println!("\nWARNING: Running in debug build. For accurate results, run with `--release`.\n");
    }
    println!("Running XSLT-based Performance Test Example...");

    let args: Vec<String> = env::args().collect();
    let num_records = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(500);
    let max_items = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(15);
    println!("Configuration: {} pages, up to {} table rows per page.", num_records, max_items);

    // --- FIX: Create an absolute path to the template file ---
    // This makes the example runnable from any directory.
    let mut template_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    template_path.push("templates/perf_test_template.xsl");

    println!("✓ Loading XSLT template from '{}'.", template_path.display());
    let data_iterator = generate_perf_test_data_iter(num_records, max_items);
    println!("✓ Data iterator created.");

    // The builder now receives a guaranteed valid path.
    let pipeline = PipelineBuilder::new()
        .with_xslt_template_file(&template_path)?
        .build()?;
    println!("✓ Pipeline built with XSLT engine.");

    // --- FIX: Create an absolute path for the output file ---
    let mut output_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    output_path.push("target/performance_test_xslt_output.pdf");

    println!("Starting PDF generation for {} records to {}...", num_records, output_path.display());
    let start_time = Instant::now();

    pipeline.generate_to_file(data_iterator, &output_path)?;

    let duration = start_time.elapsed();
    println!("\nSuccess! Generated {}", output_path.display());
    println!("Total time taken: {:.2} seconds for {} records.", duration.as_secs_f64(), num_records);
    if num_records > 0 {
        println!("Average time per record: {:.2} ms.", duration.as_millis() as f64 / num_records as f64);
    }
    Ok(())
}