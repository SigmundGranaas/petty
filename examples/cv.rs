// examples/cv.rs
use petty::{PipelineBuilder, PipelineError};
use serde_json::from_str;
use std::env;
use std::fs;

fn main() -> Result<(), PipelineError> {
    // Initialize logging to see the parser's trace.
    // Run with `RUST_LOG=debug` for more detail.
    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "petty=info");
    }
    env_logger::init();

    println!("Running CV/Resume Example (XSLT)...");

    // 1. Define the path to the self-contained XSLT template.
    // This template includes both the layout/style definitions (<petty:page-layout>,
    // <xsl:attribute-set>) and the content structure.
    let template_path = "templates/cv_template.xsl";
    println!("✓ Using template: {}", template_path);

    // 2. Load the data file.
    // The data is structured with nested arrays for experience, education, etc.,
    // which the <xsl:for-each> tags in the template will iterate over.
    let data_json_str = fs::read_to_string("data/cv_data.json")?;
    let data_json = from_str(&data_json_str)?;
    println!("✓ Data loaded.");

    // 3. Build the document pipeline directly from the XSLT template file.
    // The builder automatically discovers and parses the layout and style blocks
    // within the XSLT file to configure the document.
    let pipeline = PipelineBuilder::new()
        .with_xslt_template_file(template_path)?
        .build()?;
    println!("✓ Pipeline built with XSLT engine.");

    // 4. Generate the PDF.
    // The <page-sequence> tag in the template will create the document from the root
    // of the data, laying out all specified elements.
    let output_path = "cv.pdf";
    pipeline.generate_to_file(&data_json, output_path)?;

    println!("\nSuccess! Generated {}", output_path);
    Ok(())
}