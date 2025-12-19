use petty::{PipelineBuilder, PipelineError};
use serde_json::{from_str, Value};
use std::env;
use std::fs;

fn main() -> Result<(), PipelineError> {
    if env::var("RUST_LOG").is_err() {
        unsafe { env::set_var("RUST_LOG", "petty=info"); }
    }
    env_logger::init();

    println!("Running JSON Report Example...");

    let template_path = "templates/report_template.json";
    println!("✓ Using template: {}", template_path);

    // CORRECTED: Use the financial report data, which matches the template's structure and variables.
    let data_json_str = fs::read_to_string("data/financial_report_data.json").unwrap();
    let data_json: Value = from_str(&data_json_str).unwrap();
    println!("✓ Data loaded.");

    // Build the pipeline from the JSON template file.
    let pipeline = PipelineBuilder::new()
        .with_template_file(template_path)?
        .build()?;
    println!("✓ Pipeline built with JSON engine.");

    // A single JSON template produces a single document sequence from the root of the data.
    let output_path = "json_report.pdf";
    // Pass a Vec containing the single data object.
    pipeline.generate_to_file(vec![data_json], output_path)?;

    println!("\nSuccess! Generated {}", output_path);
    Ok(())
}