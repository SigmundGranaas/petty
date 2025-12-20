//! PDF content stream generation benchmarks
//!
//! Measures isolated PDF content generation performance.

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use petty::PipelineBuilder;
use serde_json::json;
use std::io::Cursor;
use tokio::runtime::Runtime;

/// Template for PDF generation benchmarks with varied content
/// Uses Block/Paragraph instead of Heading to avoid triggering metadata pipeline
fn multi_element_template() -> &'static str {
    r#"{
        "_stylesheet": {
            "defaultPageMaster": "default",
            "pageMasters": { "default": { "size": "A4", "margins": "1cm" } },
            "styles": {
                "default": { "font-family": "Helvetica" },
                "header": { "font-family": "Helvetica", "font-size": "18pt", "font-weight": "bold" },
                "body": { "font-family": "Helvetica", "font-size": "12pt" }
            }
        },
        "_template": {
            "type": "Block",
            "children": [
                { "type": "Paragraph", "style": "header", "children": [
                    { "type": "Text", "content": "{{title}}" }
                ]},
                { "type": "Paragraph", "style": "body", "children": [
                    { "type": "Text", "content": "{{body}}" }
                ]},
                { "type": "Block", "children": [
                    { "type": "Paragraph", "children": [
                        { "type": "Text", "content": "{{footer}}" }
                    ]}
                ]}
            ]
        }
    }"#
}

/// Template with multiple pages per record
fn multi_page_template() -> &'static str {
    r#"{
        "_stylesheet": {
            "defaultPageMaster": "default",
            "pageMasters": { "default": { "size": "A4", "margins": "1cm" } },
            "styles": { "default": { "font-family": "Helvetica" } }
        },
        "_template": {
            "type": "Block",
            "children": [
                { "type": "Paragraph", "children": [{ "type": "Text", "content": "{{content}}" }]},
                { "type": "PageBreak" },
                { "type": "Paragraph", "children": [{ "type": "Text", "content": "Page 2: {{content}}" }]},
                { "type": "PageBreak" },
                { "type": "Paragraph", "children": [{ "type": "Text", "content": "Page 3: {{content}}" }]}
            ]
        }
    }"#
}

fn benchmark_pdf_content_generation(c: &mut Criterion) {
    let mut group = c.benchmark_group("pdf_content_generation");
    let template = multi_element_template();
    let rt = Runtime::new().expect("Failed to create Tokio runtime");

    for record_count in [1, 10, 100] {
        let data: Vec<_> = (0..record_count)
            .map(|i| {
                json!({
                    "title": format!("Document Title {}", i),
                    "body": format!("This is the body content for document {}.", i),
                    "footer": format!("Footer for document {}", i)
                })
            })
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

fn benchmark_multi_page_generation(c: &mut Criterion) {
    let mut group = c.benchmark_group("multi_page_generation");
    let template = multi_page_template();
    let rt = Runtime::new().expect("Failed to create Tokio runtime");

    for record_count in [1, 10, 50] {
        // Each record creates 3 pages
        let total_pages = record_count * 3;
        let data: Vec<_> = (0..record_count)
            .map(|i| json!({ "content": format!("Record {} content", i) }))
            .collect();

        group.bench_with_input(
            BenchmarkId::new("pages", total_pages),
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

fn benchmark_pdf_output_size(c: &mut Criterion) {
    let mut group = c.benchmark_group("pdf_output_size");
    let template = multi_element_template();
    let rt = Runtime::new().expect("Failed to create Tokio runtime");

    // Vary content size per record
    for content_size in [100, 1000, 5000] {
        let body_content = "x".repeat(content_size);
        let data = vec![json!({
            "title": "Test Document",
            "body": body_content,
            "footer": "Document Footer"
        })];

        group.bench_with_input(
            BenchmarkId::new("content_bytes", content_size),
            &content_size,
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

criterion_group!(
    benches,
    benchmark_pdf_content_generation,
    benchmark_multi_page_generation,
    benchmark_pdf_output_size
);
criterion_main!(benches);
