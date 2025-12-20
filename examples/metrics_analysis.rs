use petty::{PipelineBuilder, ProcessingMode};
use serde_json::json;
use std::time::Instant;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let count: usize = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(5000);
    let workers: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(19);

    println!("Generating {} records with {} workers (metrics enabled)...", count, workers);

    let pipeline = PipelineBuilder::new()
        .with_template_file("templates/perf_test_template.xsl")?
        .with_processing_mode(ProcessingMode::WithMetrics)
        .with_worker_count(workers)
        .build()?;

    let data: Vec<_> = (0..count)
        .map(|i| json!({
            "id": i,
            "name": format!("Record {}", i),
            "value": i * 100,
        }))
        .collect();

    let start = Instant::now();
    pipeline.generate_to_file(data, "metrics_test.pdf")?;
    let elapsed = start.elapsed();

    println!("\n=== Results ===");
    println!("Records: {}", count);
    println!("Workers: {}", workers);
    println!("Time: {:.3}s", elapsed.as_secs_f64());
    println!("Throughput: {:.1} records/sec", count as f64 / elapsed.as_secs_f64());

    if let Some(metrics) = pipeline.metrics() {
        println!("\n=== Pipeline Metrics ===");
        println!("Items processed: {}", metrics.items_processed);
        println!("Current workers: {}", metrics.current_workers);
        println!("Queue depth (final): {}", metrics.queue_depth);
        println!("Queue high water: {}", metrics.queue_high_water);
        println!("Throughput (internal): {:.1} items/sec", metrics.throughput);
        if let Some(avg_time) = metrics.avg_item_time {
            println!("Avg item time: {:.3}ms", avg_time.as_secs_f64() * 1000.0);

            // Calculate efficiency metrics
            let theoretical_max = workers as f64 / avg_time.as_secs_f64();
            let efficiency = (metrics.throughput / theoretical_max) * 100.0;
            println!("\n=== Efficiency Analysis ===");
            println!("Theoretical max throughput: {:.1} items/sec", theoretical_max);
            println!("Actual throughput: {:.1} items/sec", metrics.throughput);
            println!("Parallel efficiency: {:.1}%", efficiency);
        }
        println!("Pipeline elapsed: {:.3}s", metrics.elapsed.as_secs_f64());
    }

    Ok(())
}
