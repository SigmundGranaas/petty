//! End-to-end pipeline throughput benchmarks
//!
//! Measures the complete PDF generation pipeline with varying:
//! - Record counts (1, 10, 100, 1000)
//! - Worker configurations (2, 4, 8, 16)
//! - Processing modes (Standard, WithMetrics)
//!
//! Run benchmarks: `cargo bench --bench pipeline_throughput`
//!
//! Compare specific modes:
//! ```
//! cargo bench --bench pipeline_throughput -- "processing_mode"
//! cargo bench --bench pipeline_throughput -- "worker_scaling"
//! ```

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use petty::{PipelineBuilder, ProcessingMode};
use serde_json::json;
use std::io::Cursor;
use tokio::runtime::Runtime;

/// Simple template for benchmarking - minimal overhead
fn simple_template() -> &'static str {
    r#"{
        "_stylesheet": {
            "defaultPageMaster": "default",
            "pageMasters": { "default": { "size": "A4", "margins": "1cm" } },
            "styles": { "default": { "font-family": "Helvetica" } }
        },
        "_template": {
            "type": "Block",
            "children": [
                { "type": "Paragraph", "children": [
                    { "type": "Text", "content": "Record: {{id}}" }
                ]},
                { "type": "Paragraph", "children": [
                    { "type": "Text", "content": "Name: {{name}}" }
                ]},
                { "type": "Paragraph", "children": [
                    { "type": "Text", "content": "Value: {{value}}" }
                ]}
            ]
        }
    }"#
}

/// Generate test data records
fn generate_records(count: usize) -> Vec<serde_json::Value> {
    (0..count)
        .map(|i| {
            json!({
                "id": i,
                "name": format!("Record {}", i),
                "value": i * 100
            })
        })
        .collect()
}

/// Benchmark pipeline throughput with varying record counts
fn benchmark_pipeline_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("pipeline_throughput");
    let rt = Runtime::new().expect("Failed to create Tokio runtime");

    // Test with different record counts
    for count in [1, 10, 100, 1000] {
        group.throughput(Throughput::Elements(count as u64));
        let records = generate_records(count);
        let template = simple_template();

        group.bench_with_input(BenchmarkId::new("records", count), &count, |b, _| {
            b.iter(|| {
                let pipeline = PipelineBuilder::new()
                    .with_template_source(template, "json")
                    .expect("Failed to parse template")
                    .build()
                    .expect("Failed to build pipeline");

                let writer = Cursor::new(Vec::new());
                rt.block_on(async {
                    pipeline
                        .generate(records.clone().into_iter(), writer)
                        .await
                        .expect("Failed to generate PDF")
                })
            });
        });
    }

    group.finish();
}

/// Benchmark worker scaling with fixed record count
fn benchmark_worker_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("worker_scaling");
    let rt = Runtime::new().expect("Failed to create Tokio runtime");

    // Fixed record count, varying worker counts
    let record_count = 100;
    let records = generate_records(record_count);
    let template = simple_template();

    for worker_count in [2, 4, 8, 16] {
        group.throughput(Throughput::Elements(record_count as u64));

        group.bench_with_input(
            BenchmarkId::new("workers", worker_count),
            &worker_count,
            |b, &worker_count| {
                b.iter(|| {
                    let pipeline = PipelineBuilder::new()
                        .with_template_source(template, "json")
                        .expect("Failed to parse template")
                        .with_worker_count(worker_count)
                        .build()
                        .expect("Failed to build pipeline");

                    let writer = Cursor::new(Vec::new());
                    rt.block_on(async {
                        pipeline
                            .generate(records.clone().into_iter(), writer)
                            .await
                            .expect("Failed to generate PDF")
                    })
                });
            },
        );
    }

    group.finish();
}

/// Benchmark different processing modes
fn benchmark_processing_modes(c: &mut Criterion) {
    let mut group = c.benchmark_group("processing_mode");
    let rt = Runtime::new().expect("Failed to create Tokio runtime");

    let record_count = 100;
    let records = generate_records(record_count);
    let template = simple_template();

    // Standard mode (baseline)
    group.throughput(Throughput::Elements(record_count as u64));
    group.bench_function("standard", |b| {
        b.iter(|| {
            let pipeline = PipelineBuilder::new()
                .with_template_source(template, "json")
                .expect("Failed to parse template")
                .with_processing_mode(ProcessingMode::Standard)
                .build()
                .expect("Failed to build pipeline");

            let writer = Cursor::new(Vec::new());
            rt.block_on(async {
                pipeline
                    .generate(records.clone().into_iter(), writer)
                    .await
                    .expect("Failed to generate PDF")
            })
        });
    });

    // WithMetrics mode (measures overhead of metrics collection)
    group.bench_function("with_metrics", |b| {
        b.iter(|| {
            let pipeline = PipelineBuilder::new()
                .with_template_source(template, "json")
                .expect("Failed to parse template")
                .with_processing_mode(ProcessingMode::WithMetrics)
                .build()
                .expect("Failed to build pipeline");

            let writer = Cursor::new(Vec::new());
            let result = rt.block_on(async {
                pipeline
                    .generate(records.clone().into_iter(), writer)
                    .await
                    .expect("Failed to generate PDF")
            });

            // Verify metrics are available
            if let Some(metrics) = pipeline.metrics() {
                assert!(metrics.items_processed > 0);
            }

            result
        });
    });

    group.finish();
}

/// Benchmark to compare metrics collection overhead
fn benchmark_metrics_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("metrics_overhead");
    let rt = Runtime::new().expect("Failed to create Tokio runtime");

    let template = simple_template();

    // Compare at different scales to measure overhead
    for record_count in [10, 100, 500] {
        let records = generate_records(record_count);
        group.throughput(Throughput::Elements(record_count as u64));

        group.bench_with_input(
            BenchmarkId::new("standard", record_count),
            &record_count,
            |b, _| {
                b.iter(|| {
                    let pipeline = PipelineBuilder::new()
                        .with_template_source(template, "json")
                        .expect("Failed to parse template")
                        .with_processing_mode(ProcessingMode::Standard)
                        .build()
                        .expect("Failed to build pipeline");

                    let writer = Cursor::new(Vec::new());
                    rt.block_on(async {
                        pipeline
                            .generate(records.clone().into_iter(), writer)
                            .await
                            .expect("Failed to generate PDF")
                    })
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("with_metrics", record_count),
            &record_count,
            |b, _| {
                b.iter(|| {
                    let pipeline = PipelineBuilder::new()
                        .with_template_source(template, "json")
                        .expect("Failed to parse template")
                        .with_processing_mode(ProcessingMode::WithMetrics)
                        .build()
                        .expect("Failed to build pipeline");

                    let writer = Cursor::new(Vec::new());
                    rt.block_on(async {
                        pipeline
                            .generate(records.clone().into_iter(), writer)
                            .await
                            .expect("Failed to generate PDF")
                    })
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    benchmark_pipeline_throughput,
    benchmark_worker_scaling,
    benchmark_processing_modes,
    benchmark_metrics_overhead
);
criterion_main!(benches);
