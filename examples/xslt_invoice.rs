use petty::{PipelineBuilder, PipelineError};
use serde_json::from_str;
use std::env;
use std::fs;

fn main() -> Result<(), PipelineError> {
    // Initialize logging to see the parser's trace.
    // Run with `RUST_LOG=debug cargo run ...` for more detail.
    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "petty=info");
    }
    env_logger::init();

    println!("Running XSLT Invoice Example...");

    // 1. Define the path to the self-contained XSLT template.
    let template_path = "templates/invoice_template.xsl";
    println!("✓ Using template: {}", template_path);

    // 2. Load the data file.
    let data_json_str = fs::read_to_string("data/invoice_data.json")?;
    let data_json = from_str(&data_json_str)?;
    println!("✓ Data loaded.");

    // 3. Build the document pipeline directly from the XSLT template file.
    // The builder automatically discovers and parses the <petty:page-layout> and
    // <xsl:attribute-set> blocks within the XSLT file to configure the document.
    let pipeline = PipelineBuilder::new()
        .with_xslt_template_file(template_path)?
        .build()?;
    println!("✓ Pipeline built with XSLT engine.");

    // 4. Generate the PDF.
    // The <page-sequence> tag in the template will create a new page for each customer.
    let output_path = "xslt_invoices.pdf";
    pipeline.generate_to_file(&data_json, output_path)?;

    println!("\nSuccess! Generated {}", output_path);
    Ok(())
}