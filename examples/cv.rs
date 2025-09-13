use petty::{PipelineBuilder, PipelineError};
use serde_json::from_str;
use std::env;
use std::fs;

fn main() -> Result<(), PipelineError> {
    if env::var("RUST_LOG").is_err() {
        unsafe { env::set_var("RUST_LOG", "petty=info"); }
    }
    env_logger::init();

    println!("Running CV/Resume Example (XSLT)...");

    let template_path = "templates/cv_template.xsl";
    println!("✓ Using template: {}", template_path);

    let data_json_str = fs::read_to_string("data/cv_data.json")?;
    let data_json = from_str(&data_json_str)?;
    println!("✓ Data loaded.");

    // Build the pipeline from the XSLT template file.
    let pipeline = PipelineBuilder::new()
        .with_xslt_template_file(template_path)?
        .build()?;
    println!("✓ Pipeline built with XSLT engine.");

    // The <page-sequence> tag will generate a single document sequence from the root of the data.
    let output_path = "cv.pdf";
    pipeline.generate_to_file(&data_json, output_path)?;

    println!("\nSuccess! Generated {}", output_path);
    Ok(())
}