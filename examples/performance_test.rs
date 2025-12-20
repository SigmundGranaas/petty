use clap::Parser;
use petty::{PdfBackend, PipelineBuilder, PipelineError, ProcessingMode};
use rand::SeedableRng;
use rand::prelude::*;
use rand::rngs::StdRng;
use serde_json::{Value, json};
use std::env;
use std::time::Instant;

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

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Number of records to generate
    #[arg(default_value_t = 500)]
    num_records: usize,

    /// Maximum items per record
    #[arg(default_value_t = 15)]
    max_items: usize,

    /// Number of worker threads (0 = auto-detect based on CPU count)
    #[arg(short, long, default_value_t = 0)]
    workers: usize,

    /// Enable metrics collection for throughput analysis
    #[arg(short, long)]
    metrics: bool,

    /// Enable adaptive scaling (requires --features adaptive-scaling)
    /// Workers will dynamically scale up/down based on queue depth
    #[arg(short, long)]
    adaptive: bool,
}

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
                let price_rounded = (price * 100.0).round() / 100.0;
                let line_total = (quantity as f64 * price_rounded * 100.0).round() / 100.0;
                total += line_total;
                json!({
                    "description": MOCK_ITEMS.choose(&mut rng).unwrap_or(&""),
                    "quantity": quantity, "price": price_rounded, "line_total": line_total
                })
            })
            .collect();

        let subtotal = (total * 100.0).round() / 100.0;
        let tax = (subtotal * 0.08 * 100.0).round() / 100.0;
        let grand_total = subtotal + tax;

        json!({
            "id": 10000 + i,
            "user": { "name": MOCK_USERS.choose(&mut rng).unwrap_or(&""), "account": format!("ACC-{}", rng.random_range(100000..999999)) },
            "items": items,
            "summary": { "total": subtotal, "tax": tax, "grand_total": grand_total }
        })
    })
}

fn main() -> Result<(), PipelineError> {
    // 3. Initialize the Profiler
    // The `_profiler` variable must remain in scope for the duration you want to track.
    // When it is dropped at the end of main, the report is generated.

    // Initialize the logger.
    if env::var("RUST_LOG").is_err() {
        unsafe {
            env::set_var("RUST_LOG", "petty=info");
        }
    }
    env_logger::init();

    if cfg!(debug_assertions) {
        println!(
            "\nWARNING: Running in debug build. For accurate results, run with `--release`.\n"
        );
    }

    let args = Args::parse();
    let num_records = args.num_records;
    let max_items = args.max_items;

    let template_path = "templates/perf_test_template.xsl";
    let data_iterator = generate_perf_test_data_iter(num_records, max_items);
    let output_path = "performance_test_output.pdf";

    println!("Starting Pipeline...");
    println!("  Records: {}", num_records);
    println!("  Template: {}", template_path);
    if args.workers > 0 {
        println!("  Workers: {}", args.workers);
    } else {
        println!("  Workers: auto (based on CPU count)");
    }
    println!("  Metrics: {}", if args.metrics || args.adaptive { "enabled" } else { "disabled" });
    println!("  Adaptive scaling: {}", if args.adaptive { "enabled" } else { "disabled" });

    // Build pipeline with configurable options
    let mut builder = PipelineBuilder::new()
        .with_template_file(template_path)?
        .with_pdf_backend(PdfBackend::LopdfParallel);

    // Set worker count if specified
    if args.workers > 0 {
        builder = builder.with_worker_count(args.workers);
    }

    // Enable metrics collection if requested (or if adaptive is enabled)
    if args.metrics || args.adaptive {
        builder = builder.with_processing_mode(ProcessingMode::WithMetrics);
    }

    // Enable adaptive scaling if requested
    if args.adaptive {
        builder = builder.with_adaptive_scaling(true);
    }

    let pipeline = builder.build()?;

    let start_time = Instant::now();
    pipeline.generate_to_file(data_iterator, output_path)?;
    let duration = start_time.elapsed();

    println!("\nSuccess! Generated {}", output_path);
    println!("Total time: {:.2}s", duration.as_secs_f64());
    println!("Records/sec: {:.1}", num_records as f64 / duration.as_secs_f64());

    // Display metrics if available
    if let Some(metrics) = pipeline.metrics() {
        println!("\n=== Pipeline Metrics ===");
        println!("  Items processed: {}", metrics.items_processed);
        println!("  Active workers: {}", metrics.current_workers);
        println!("  Throughput: {:.1} items/sec", metrics.throughput);
        if let Some(avg_time) = metrics.avg_item_time {
            println!("  Avg item time: {:?}", avg_time);
        }
        println!("  Queue high water: {}", metrics.queue_high_water);
        println!("  Pipeline health: {}", if metrics.is_healthy() { "healthy" } else { "backlogged" });
        println!("  Utilization: {:.1}%", metrics.utilization() * 100.0);
    }

    // Display adaptive scaling metrics if available (requires --features adaptive-scaling)
    #[cfg(feature = "adaptive-scaling")]
    if args.adaptive {
        if let Some(pending) = pipeline.pending_shutdowns() {
            println!("  Pending shutdowns: {}", pending);
        }
        if let Some(wm_metrics) = pipeline.worker_manager_metrics() {
            println!("\n=== Worker Manager Metrics ===");
            println!("  Final worker count: {}", wm_metrics.current_workers);
            println!("  Queue depth (final): {}", wm_metrics.queue_depth);
        }
    }

    Ok(())
}
