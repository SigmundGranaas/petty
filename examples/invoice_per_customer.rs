use petty::{PipelineBuilder, PipelineError};
use serde_json::from_str;
use std::fs;

fn main() -> Result<(), PipelineError> {
    println!("Running Invoice per Customer Example...");

    // 1. Load the stylesheet definition from a file.
    // This stylesheet defines a "page_sequence" that iterates over the "/customers"
    // array in the data file. For each customer, it applies the "invoicePage" template.
    let stylesheet_json = fs::read_to_string("templates/invoice_stylesheet.json")?;
    println!("✓ Stylesheet loaded.");

    // 2. Load the data file.
    // This file contains a top-level "customers" array. Each object in the array
    // has customer details and a nested array of "items" for the invoice table.
    let data_json_str = fs::read_to_string("data/invoice_data.json")?;
    let data_json = from_str(&data_json_str)?;
    println!("✓ Data loaded.");

    // 3. Build the document pipeline.
    let pipeline = PipelineBuilder::new()
        .with_stylesheet_json(&stylesheet_json)?
        .build()?;
    println!("✓ Pipeline built.");

    // 4. Generate the PDF.
    // The parser will create a BeginPageSequenceItem event for each customer,
    // which the layout processor uses to start a new page.
    let output_path = "invoices.pdf";
    pipeline.generate_to_file(&data_json, output_path)?;

    println!("\nSuccess! Generated {}", output_path);
    Ok(())
}