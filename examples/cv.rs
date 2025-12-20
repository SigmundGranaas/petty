use clap::Parser;
use petty::{PdfBackend, PipelineBuilder, PipelineError};
use serde_json::{Value, from_str};
use std::env;
use std::fs;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Enable debug mode
    #[arg(long, default_value_t = false)]
    debug: bool,
}

fn main() -> Result<(), PipelineError> {
    if env::var("RUST_LOG").is_err() {
        unsafe {
            env::set_var("RUST_LOG", "petty=info");
        }
    }
    env_logger::init();

    let args = Args::parse();

    println!("Running CV/Resume Example (XSLT)...");

    let template_path = "templates/cv_template.xsl";
    println!("✓ Using template: {}", template_path);

    let data_json_str = fs::read_to_string("data/cv_data.json")?;
    let data_json: Value = from_str(&data_json_str)?;
    println!("✓ Data loaded.");

    // Build the pipeline from the XSLT template file.
    let pipeline = PipelineBuilder::new()
        .with_template_file(template_path)?
        .with_pdf_backend(PdfBackend::Lopdf)
        .with_debug(args.debug)
        .build()?;
    println!("✓ Pipeline built with XSLT engine.");

    // The <page-sequence> tag will generate a single document sequence from the root of the data.
    let output_path = "cv.pdf";
    // Since the whole document is one sequence, we pass a Vec containing the single data object.
    pipeline.generate_to_file(vec![data_json], output_path)?;

    println!("\nSuccess! Generated {}", output_path);
    Ok(())
}
