use petty::{PipelineBuilder, PipelineError};
use rand::prelude::*;
use serde_json::{json, Value};
use std::env;
use std::fs;
use std::time::Instant;

#[cfg(feature = "dhat-heap")]
#[global_allocator]
static ALLOCATOR: dhat::Alloc = dhat::Alloc;

const MOCK_USERS: &[&str] = &["Alice", "Bob", "Charlie", "Diana", "Eve", "Frank"];
const MOCK_ITEMS: &[&str] = &[
    "Standard Service Fee", "Premium Support Package", "Data Processing Unit",
    "Cloud Storage (1TB)", "API Access Key", "Consulting Hour", "Hardware Rental",
];

/// Generates a large, complex JSON dataset in memory.
fn generate_perf_test_data(num_records: usize, max_items_per_record: usize) -> Value {
    println!("Generating {} records in memory...", num_records);
    let mut rng = rand::rng();
    let records: Vec<Value> = (0..num_records).map(|i| {
        let num_items = rng.random_range(2..=max_items_per_record);
        let mut total = 0.0;
        let items: Vec<Value> = (0..num_items).map(|_| {
            let quantity = rng.random_range(1..=10);
            let price: f64 = rng.random_range(10.0..500.0);
            let line_total = quantity as f64 * price;
            total += line_total;
            json!({
                "description": MOCK_ITEMS.choose(&mut rng).unwrap_or(&""),
                "quantity": quantity, "price": price, "line_total": line_total
            })
        }).collect();
        json!({
            "id": 10000 + i,
            "user": { "name": MOCK_USERS.choose(&mut rng).unwrap_or(&""), "account": format!("ACC-{}", rng.random_range(100000..999999)) },
            "items": items,
            "summary": { "total": total, "tax": total * 0.08, "grand_total": total * 1.08 }
        })
    }).collect();
    json!({ "records": records })
}

fn main() -> Result<(), PipelineError> {
    #[cfg(feature = "dhat-heap")]
    let _profiler = dhat::Profiler::new_heap();

    if cfg!(debug_assertions) {
        println!("\nWARNING: Running in debug build. For accurate results, run with `--release`.\n");
    }
    println!("Running Advanced Performance Test Example (JSON)...");

    let args: Vec<String> = env::args().collect();
    let num_records = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(500);
    let max_items = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(15);
    println!("Configuration: {} pages, up to {} table rows per page.", num_records, max_items);

    let stylesheet_json = fs::read_to_string("templates/perf_test_stylesheet.json")?;
    println!("✓ Stylesheet loaded.");
    let data_json = generate_perf_test_data(num_records, max_items);
    println!("✓ Data generated.");

    let pipeline = PipelineBuilder::new()
        .with_stylesheet_json(&stylesheet_json)?
        .build()?;
    println!("✓ Pipeline built.");

    let output_path = "performance_test_output.pdf";
    println!("Starting PDF generation for {} records...", num_records);
    let start_time = Instant::now();

    pipeline.generate_to_file(&data_json, output_path)?;

    let duration = start_time.elapsed();
    println!("\nSuccess! Generated {}", output_path);
    println!("Total time taken: {:.2} seconds for {} pages.", duration.as_secs_f64(), num_records);
    if num_records > 0 {
        println!("Average time per page: {:.2} ms.", duration.as_millis() as f64 / num_records as f64);
    }
    Ok(())
}