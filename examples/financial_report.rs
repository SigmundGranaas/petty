use petty::{PipelineBuilder, PipelineError};
use serde_json::from_str;
use std::fs;

fn main() -> Result<(), PipelineError> {
    println!("Running Financial Report Example...");

    // 1. Load the stylesheet.
    // This stylesheet defines a single page sequence that uses a complex template.
    // It includes styles for different table row types (item, subtotal, total)
    // and a page footer definition.
    let stylesheet_json = fs::read_to_string("templates/financial_report_stylesheet.json")?;
    println!("✓ Stylesheet loaded.");

    // 2. Load the data file.
    // The data is structured with nested arrays for sections and tables, which the
    // template will iterate over to build the report. Each row has a "type" field
    // used for dynamic styling.
    let data_json_str = fs::read_to_string("data/financial_report_data.json")?;
    let data_json = from_str(&data_json_str)?;
    println!("✓ Data loaded.");

    // 3. Build the document pipeline.
    // This registers a "formatCurrency" Handlebars helper which the parser
    // will use when processing templates.
    let pipeline = PipelineBuilder::new()
        .with_stylesheet_json(&stylesheet_json)?
        .build()?;
    println!("✓ Pipeline built.");

    // 4. Generate the PDF.
    // The layout processor will calculate positions for all elements, including
    // complex tables, breaking content across pages as needed. The PDF renderer
    // then draws the elements and adds the templated footer to each physical page.
    let output_path = "financial_report.pdf";
    pipeline.generate_to_file(&data_json, output_path)?;

    println!("\nSuccess! Generated {}", output_path);
    Ok(())
}