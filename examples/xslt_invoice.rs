// FILE: examples/xslt_invoice.rs
use petty::{PipelineBuilder, PipelineError};
use serde_json::{from_str, Value};
use std::env;
use std::fs;

fn main() -> Result<(), PipelineError> {
    if env::var("RUST_LOG").is_err() {
        unsafe { env::set_var("RUST_LOG", "petty=info"); }
    }
    env_logger::init();

    println!("Running XSLT Invoice Example...");

    let template_path = "templates/invoice_template.xsl";
    println!("✓ Using template: {}", template_path);
    let data_json_str = fs::read_to_string("data/invoice_data.json")?;
    let data_json: Value = from_str(&data_json_str)?;
    println!("✓ Data loaded.");

    // Build the pipeline from the XSLT file.
    let pipeline = PipelineBuilder::new()
        .with_xslt_template_file(template_path)?
        .build()?;
    println!("✓ Pipeline built with XSLT engine.");

    // Extract the array of customers, which defines our sequences.
    let customers = if let Value::Object(mut map) = data_json {
        map.remove("customers")
            .and_then(|v| v.as_array().cloned())
            .unwrap_or_default()
    } else {
        vec![]
    };

    // Generate the PDF. The pipeline will create one IRNode tree per customer,
    // demonstrating the "sequence" processing model.
    let output_path = "xslt_invoices.pdf";
    pipeline.generate_to_file(customers.into_iter(), output_path)?;

    println!("\nSuccess! Generated {}", output_path);
    Ok(())
}