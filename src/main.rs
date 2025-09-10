use petty::{PipelineBuilder, PipelineError};
use serde_json::from_str;
use std::env;
use std::fs;

/// A simple CLI to generate a PDF from an XSLT template and a data file.
fn main() -> Result<(), PipelineError> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 4 {
        eprintln!("A simple tool to generate PDFs from JSON data and an XSLT template.");
        eprintln!();
        eprintln!(
            "Usage: {} <path/to/template.xsl> <path/to/data.json> <path/to/output.pdf>",
            args[0]
        );
        eprintln!();
        eprintln!("To run examples:");
        eprintln!("  cargo run --example simple_report");
        eprintln!("  cargo run --example xslt_invoice");
        std::process::exit(1);
    }

    let template_path = &args[1];
    let data_path = &args[2];
    let output_path = &args[3];

    println!("Loading template from {}", template_path);

    println!("Loading data from {}", data_path);
    let data_json_str = fs::read_to_string(data_path)?;
    let data_json = from_str(&data_json_str)?;

    println!("Building PDF generation pipeline...");
    let pipeline = PipelineBuilder::new()
        .with_stylesheet_file(template_path)?
        .build()?;

    println!("Generating PDF to {}...", output_path);
    pipeline.generate_to_file(&data_json, output_path)?;

    println!("Successfully generated {}", output_path);
    Ok(())
}