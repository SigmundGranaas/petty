use petty::{PdfBackend, PipelineBuilder, PipelineError};
use serde_json::{from_str, Value};
use std::env;
use std::fs;

fn main() -> Result<(), PipelineError> {
    if env::var("RUST_LOG").is_err() {
        unsafe { env::set_var("RUST_LOG", "petty=info"); }
    }
    env_logger::init();

    println!("Running XSLT Table of Contents Example...");

    let template_path = "templates/toc_template.xsl";
    println!("✓ Using template: {}", template_path);

    let data_json_str = fs::read_to_string("data/toc_data.json")?;
    let data_json: Value = from_str(&data_json_str)?;
    println!("✓ Data loaded.");

    let pipeline = PipelineBuilder::new()
        .with_template_file(template_path)?
        .with_debug(true)
        .with_pdf_backend(PdfBackend::Lopdf)
        .build()?;
    println!("✓ Pipeline built with XSLT engine.");

    let output_path = "xslt_toc.pdf";
    pipeline.generate_to_file(vec![data_json], output_path)?;

    println!("\nSuccess! Generated {}", output_path);
    Ok(())
}