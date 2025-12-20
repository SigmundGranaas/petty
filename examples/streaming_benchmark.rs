//! Streaming renderer benchmark.
//!
//! Run with:
//!   cargo run --release --example streaming_benchmark -- 2000

use clap::Parser;
use petty::pipeline::config::PdfBackend;
use petty::pipeline::context::PipelineContext;
use petty::pipeline::provider::passthrough::PassThroughProvider;
use petty::pipeline::provider::DataSourceProvider;
use petty::pipeline::renderer::streaming::SinglePassStreamingRenderer;
use petty::pipeline::renderer::RenderingStrategy;
use petty::pipeline::adapters::TemplateParserAdapter;
use petty_core::layout::fonts::SharedFontLibrary;
use petty_core::parser::processor::TemplateParser;
use petty_json_template::JsonParser;
use rand::prelude::*;
use rand::rngs::StdRng;
use rand::SeedableRng;
use serde_json::{json, Value};
use std::io::Cursor;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

#[derive(Parser, Debug)]
#[command(version, about = "Benchmark streaming renderer")]
struct Args {
    /// Number of records to generate
    #[arg(default_value_t = 1000)]
    num_records: usize,

    /// Number of iterations for averaging
    #[arg(short, long, default_value_t = 3)]
    iterations: usize,
}

const MOCK_USERS: &[&str] = &["Alice", "Bob", "Charlie", "Diana", "Eve", "Frank"];

fn generate_test_data(num_records: usize) -> Vec<Value> {
    let mut rng = StdRng::from_seed(Default::default());
    (0..num_records)
        .map(|i| {
            let total: f64 = rng.random_range(100.0..1000.0);
            json!({
                "id": 10000 + i,
                "user": { "name": MOCK_USERS.choose(&mut rng).unwrap_or(&"") },
                "total": (total * 100.0).round() / 100.0
            })
        })
        .collect()
}

fn create_context() -> PipelineContext {
    // Simple template that produces 1-2 pages per record
    let template_json = json!({
        "_stylesheet": {
            "defaultPageMaster": "default",
            "pageMasters": { "default": { "size": "A4", "margins": "1cm" } },
            "styles": { "default": { "font-family": "Helvetica", "font-size": "10pt" } }
        },
        "_template": {
            "type": "Block",
            "children": [
                { "type": "Paragraph", "children": [{ "type": "Text", "content": "Invoice #{{id}}" }] },
                { "type": "Paragraph", "children": [{ "type": "Text", "content": "Customer: {{user.name}}" }] },
                { "type": "Paragraph", "children": [{ "type": "Text", "content": "Item 1" }] },
                { "type": "Paragraph", "children": [{ "type": "Text", "content": "Item 2" }] },
                { "type": "Paragraph", "children": [{ "type": "Text", "content": "Item 3" }] },
                { "type": "Paragraph", "children": [{ "type": "Text", "content": "Total: ${{total}}" }] }
            ]
        }
    });

    let template_str = serde_json::to_string(&template_json).unwrap();
    let parser = TemplateParserAdapter::new(JsonParser);
    let features = parser.parse(&template_str, PathBuf::new()).unwrap();
    let library = SharedFontLibrary::new();
    library.load_fallback_font();

    PipelineContext {
        compiled_template: features.main_template,
        role_templates: Arc::new(features.role_templates),
        font_library: Arc::new(library),
        resource_provider: Arc::new(petty_resource::InMemoryResourceProvider::new()),
        cache_config: Default::default(),
        adaptive: None,
    }
}

fn run_streaming_benchmark(context: &PipelineContext, data: Vec<Value>) -> f64 {
    let provider = PassThroughProvider;
    let renderer = SinglePassStreamingRenderer::with_config(PdfBackend::Lopdf, None, 16);

    let start = Instant::now();
    let prepared = provider.provide(context, data.into_iter()).unwrap();
    let writer = Cursor::new(Vec::new());

    // Clone context for the blocking task
    let context_clone = context.clone();

    // Run in blocking context since streaming uses tokio
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        tokio::task::spawn_blocking(move || {
            renderer.render(&context_clone, prepared, writer)
        }).await.unwrap().unwrap()
    });

    start.elapsed().as_secs_f64()
}

fn main() {
    env_logger::init();
    let args = Args::parse();

    println!("=== Streaming Renderer Benchmark ===");
    println!("Records: {}", args.num_records);
    println!("Iterations: {}", args.iterations);
    println!();

    let context = create_context();

    let mut times = Vec::new();
    for i in 1..=args.iterations {
        let data = generate_test_data(args.num_records);
        let elapsed = run_streaming_benchmark(&context, data);
        let rec_per_sec = args.num_records as f64 / elapsed;
        println!("  Run {}: {:.3}s ({:.1} rec/s)", i, elapsed, rec_per_sec);
        times.push(elapsed);
    }

    let avg_time = times.iter().sum::<f64>() / times.len() as f64;
    let avg_throughput = args.num_records as f64 / avg_time;

    println!();
    println!("Average: {:.3}s ({:.1} rec/s)", avg_time, avg_throughput);
}
