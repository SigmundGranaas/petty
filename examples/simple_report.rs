use petty::{PipelineBuilder, PipelineError};
use serde_json::from_str;
use std::fs;

fn main() -> Result<(), PipelineError> {
    println!("Running Simple Report Example...");

    // 1. Load the stylesheet definition from a file.
    // This stylesheet uses a "page_sequence" that points to the root of the data ("/")
    // to generate a single document from the provided data.
    let stylesheet_json = fs::read_to_string("templates/report_stylesheet.json")?;
    println!("✓ Stylesheet loaded.");

    // 2. Load the data from a file.
    // This data contains a title and a very long string of content, which will
    // force the layout engine to create multiple pages.
    let data_json_str = fs::read_to_string("data/report_data.json")?;
    let data_json = from_str(&data_json_str)?;
    println!("✓ Data loaded.");

    // 3. Build the document pipeline with the loaded stylesheet.
    let pipeline = PipelineBuilder::new()
        .with_stylesheet_json(&stylesheet_json)?
        .build()?;
    println!("✓ Pipeline built.");

    // 4. Generate the PDF file.
    // The pipeline will now execute:
    // - Parsing the data and template into a stream of layout events.
    // - Processing the events to calculate element positions and page breaks.
    // - Rendering the final positioned elements into a PDF file.
    let output_path = "simple_report.pdf";
    pipeline.generate_to_file(&data_json, output_path)?;

    println!("\nSuccess! Generated {}", output_path);
    Ok(())
}