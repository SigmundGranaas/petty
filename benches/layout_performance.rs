//! Layout engine micro-benchmarks
//!
//! Measures layout computation performance for various element types and complexities.

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use petty::PipelineBuilder;
use serde_json::json;
use std::io::Cursor;
use tokio::runtime::Runtime;

/// Template with simple text content
fn simple_text_template() -> &'static str {
    r#"{
        "_stylesheet": {
            "defaultPageMaster": "default",
            "pageMasters": { "default": { "size": "A4", "margins": "1cm" } },
            "styles": { "default": { "font-family": "Helvetica" } }
        },
        "_template": {
            "type": "Paragraph",
            "children": [{ "type": "Text", "content": "{{content}}" }]
        }
    }"#
}

/// Template with nested blocks
fn nested_blocks_template() -> &'static str {
    r#"{
        "_stylesheet": {
            "defaultPageMaster": "default",
            "pageMasters": { "default": { "size": "A4", "margins": "1cm" } },
            "styles": { "default": { "font-family": "Helvetica" } }
        },
        "_template": {
            "type": "Block",
            "children": [
                { "type": "Block", "children": [
                    { "type": "Block", "children": [
                        { "type": "Paragraph", "children": [
                            { "type": "Text", "content": "{{content}}" }
                        ]}
                    ]}
                ]}
            ]
        }
    }"#
}

/// Template with table layout
fn table_template() -> &'static str {
    r#"{
        "_stylesheet": {
            "defaultPageMaster": "default",
            "pageMasters": { "default": { "size": "A4", "margins": "1cm" } },
            "styles": { "default": { "font-family": "Helvetica" } }
        },
        "_template": {
            "type": "Table",
            "columns": [
                { "width": { "percent": 33.33 } },
                { "width": { "percent": 33.33 } },
                { "width": { "percent": 33.34 } }
            ],
            "header": {
                "rows": [
                    {
                        "type": "Block",
                        "children": [
                            { "type": "Block", "children": [{ "type": "Paragraph", "children": [{ "type": "Text", "content": "Col 1" }] }] },
                            { "type": "Block", "children": [{ "type": "Paragraph", "children": [{ "type": "Text", "content": "Col 2" }] }] },
                            { "type": "Block", "children": [{ "type": "Paragraph", "children": [{ "type": "Text", "content": "Col 3" }] }] }
                        ]
                    }
                ]
            },
            "body": {
                "rows": [
                    {
                        "type": "Block",
                        "children": [
                            { "type": "Block", "children": [{ "type": "Paragraph", "children": [{ "type": "Text", "content": "{{col1}}" }] }] },
                            { "type": "Block", "children": [{ "type": "Paragraph", "children": [{ "type": "Text", "content": "{{col2}}" }] }] },
                            { "type": "Block", "children": [{ "type": "Paragraph", "children": [{ "type": "Text", "content": "{{col3}}" }] }] }
                        ]
                    }
                ]
            }
        }
    }"#
}

fn benchmark_layout_simple_text(c: &mut Criterion) {
    let mut group = c.benchmark_group("layout_simple_text");
    let template = simple_text_template();
    let rt = Runtime::new().expect("Failed to create Tokio runtime");

    for text_length in [10, 100, 1000] {
        let content = "x".repeat(text_length);
        let data = vec![json!({ "content": content })];

        group.bench_with_input(
            BenchmarkId::new("chars", text_length),
            &text_length,
            |b, _| {
                b.iter(|| {
                    let pipeline = PipelineBuilder::new()
                        .with_template_source(template, "json")
                        .expect("Failed to parse template")
                        .build()
                        .expect("Failed to build pipeline");

                    let writer = Cursor::new(Vec::new());
                    rt.block_on(async {
                        pipeline
                            .generate(data.clone().into_iter(), writer)
                            .await
                            .expect("Failed to generate PDF")
                    })
                });
            },
        );
    }

    group.finish();
}

fn benchmark_layout_nested_blocks(c: &mut Criterion) {
    let mut group = c.benchmark_group("layout_nested_blocks");
    let template = nested_blocks_template();
    let rt = Runtime::new().expect("Failed to create Tokio runtime");

    for record_count in [1, 10, 50] {
        let data: Vec<_> = (0..record_count)
            .map(|i| json!({ "content": format!("Content {}", i) }))
            .collect();

        group.bench_with_input(
            BenchmarkId::new("records", record_count),
            &record_count,
            |b, _| {
                b.iter(|| {
                    let pipeline = PipelineBuilder::new()
                        .with_template_source(template, "json")
                        .expect("Failed to parse template")
                        .build()
                        .expect("Failed to build pipeline");

                    let writer = Cursor::new(Vec::new());
                    rt.block_on(async {
                        pipeline
                            .generate(data.clone().into_iter(), writer)
                            .await
                            .expect("Failed to generate PDF")
                    })
                });
            },
        );
    }

    group.finish();
}

fn benchmark_layout_tables(c: &mut Criterion) {
    let mut group = c.benchmark_group("layout_tables");
    let template = table_template();
    let rt = Runtime::new().expect("Failed to create Tokio runtime");

    for row_count in [1, 10, 50] {
        let data: Vec<_> = (0..row_count)
            .map(|i| {
                json!({
                    "col1": format!("Row {} Col 1", i),
                    "col2": format!("Row {} Col 2", i),
                    "col3": format!("Row {} Col 3", i)
                })
            })
            .collect();

        group.bench_with_input(BenchmarkId::new("rows", row_count), &row_count, |b, _| {
            b.iter(|| {
                let pipeline = PipelineBuilder::new()
                    .with_template_source(template, "json")
                    .expect("Failed to parse template")
                    .build()
                    .expect("Failed to build pipeline");

                let writer = Cursor::new(Vec::new());
                rt.block_on(async {
                    pipeline
                        .generate(data.clone().into_iter(), writer)
                        .await
                        .expect("Failed to generate PDF")
                })
            });
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    benchmark_layout_simple_text,
    benchmark_layout_nested_blocks,
    benchmark_layout_tables
);
criterion_main!(benches);
