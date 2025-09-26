use petty::{PipelineBuilder, PipelineError};
use serde_json::{from_str, Value};
use std::env;
use std::fs;

fn main() -> Result<(), PipelineError> {
    if env::var("RUST_LOG").is_err() {
        unsafe { env::set_var("RUST_LOG", "petty=info"); }
    }
    env_logger::init();

    println!("Running XSLT Financial Report Example (Native Tags)...");

    let template_path = "templates/financial_report_template.xsl";
    println!("✓ Using template: {}", template_path);

    let data_json_str = fs::read_to_string("data/financial_report_data.json")?;
    let data_json: Value = from_str(&data_json_str)?;
    println!("✓ Data loaded.");

    // Build the pipeline from the XSLT template file.
    let pipeline = PipelineBuilder::new()
        .with_xslt_template_file(template_path)?
        .build()?;
    println!("✓ Pipeline built with XSLT engine.");

    // The <page-sequence> tag will generate a single document from the root of the data.
    let output_path = "financial_report.pdf";
    pipeline.generate_to_file(std::iter::once(data_json), output_path)?;

    println!("\nSuccess! Generated {}", output_path);
    Ok(())
}