//! Configuration benchmark to find optimal worker count and buffer size.
//!
//! Run with: cargo run --release --example bench_config

use petty::{PdfBackend, PipelineBuilder, PipelineError};
use rand::prelude::*;
use rand::rngs::StdRng;
use rand::SeedableRng;
use serde_json::{json, Value};
use std::io::Cursor;
use std::time::Instant;

const RECORDS: usize = 3000;
const ITERATIONS: usize = 5;
const MOCK_USERS: &[&str] = &["Alice", "Bob", "Charlie", "Diana", "Eve", "Frank"];
const MOCK_ITEMS: &[&str] = &[
    "Standard Service Fee",
    "Premium Support Package",
    "Data Processing Unit",
    "Cloud Storage (1TB)",
];

fn generate_data(num_records: usize) -> Vec<Value> {
    let mut rng = StdRng::from_seed(Default::default());
    (0..num_records)
        .map(|i| {
            let num_items = rng.random_range(2..=8);
            let mut total = 0.0;
            let items: Vec<Value> = (0..num_items)
                .map(|_| {
                    let qty = rng.random_range(1..=5);
                    let price: f64 = rng.random_range(10.0..200.0);
                    let line = (qty as f64 * price * 100.0).round() / 100.0;
                    total += line;
                    json!({"description": MOCK_ITEMS.choose(&mut rng).unwrap(), "quantity": qty, "price": price, "line_total": line})
                })
                .collect();
            json!({
                "id": 10000 + i,
                "user": {"name": MOCK_USERS.choose(&mut rng).unwrap()},
                "items": items,
                "summary": {"total": total, "tax": total * 0.08, "grand_total": total * 1.08}
            })
        })
        .collect()
}

fn run_benchmark(workers: usize, buffer: usize) -> Result<f64, PipelineError> {
    let mut builder = PipelineBuilder::new()
        .with_template_file("templates/perf_test_template.xsl")?
        .with_pdf_backend(PdfBackend::LopdfParallel)
        .with_render_buffer_size(buffer);

    if workers > 0 {
        builder = builder.with_worker_count(workers);
    }

    let pipeline = builder.build()?;
    let data = generate_data(RECORDS);

    let rt = tokio::runtime::Runtime::new().unwrap();
    let start = Instant::now();
    rt.block_on(pipeline.generate(data.into_iter(), Cursor::new(Vec::new())))?;
    Ok(start.elapsed().as_secs_f64())
}

fn main() -> Result<(), PipelineError> {
    println!("=== Configuration Benchmark ===");
    println!("Records: {}, Iterations: {}", RECORDS, ITERATIONS);
    println!();

    let physical_cores = num_cpus::get_physical();
    let logical_cores = num_cpus::get();
    println!("Physical cores: {}, Logical cores: {}", physical_cores, logical_cores);
    println!();

    // Test combined configurations (workers x buffer)
    println!("=== Combined Configuration Tests ===");
    let configs = [
        (8, 16), (8, 32), (8, 64),
        (12, 16), (12, 32), (12, 64),
        (14, 16), (14, 32), (14, 64),
        (16, 16), (16, 32), (16, 64),
    ];

    let mut results: Vec<(usize, usize, f64, f64)> = Vec::new();
    for &(workers, buffer) in &configs {
        let mut times = Vec::new();
        for _ in 0..ITERATIONS {
            times.push(run_benchmark(workers, buffer)?);
        }
        times.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let avg = times.iter().sum::<f64>() / ITERATIONS as f64;
        let throughput = RECORDS as f64 / avg;
        let peak = RECORDS as f64 / times.first().unwrap();
        results.push((workers, buffer, throughput, peak));
        println!("W={:>2} B={:>3}: {:.0} rec/s (peak: {:.0})", workers, buffer, throughput, peak);
    }

    // Find best configuration
    results.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap());
    let best = &results[0];
    println!();
    println!(">>> BEST: Workers={}, Buffer={} -> {:.0} rec/s <<<", best.0, best.1, best.2);

    println!();
    println!("Done!");
    Ok(())
}
