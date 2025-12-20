// Demonstrates CSS-like dimension parsing in JSON templates

use petty::{PipelineBuilder, PipelineError};
use serde_json::json;
use std::env;

fn main() -> Result<(), PipelineError> {
    if env::var("RUST_LOG").is_err() {
        unsafe {
            env::set_var("RUST_LOG", "petty=info");
        }
    }
    env_logger::init();

    println!("Running JSON CSS Dimensions Example...");
    println!("This demonstrates flexible dimension parsing:");
    println!("  - String dimensions: \"fontSize\": \"24pt\"");
    println!("  - Hex colors: \"color\": \"#2a4d69\"");
    println!("  - Named colors: \"color\": \"blue\"");
    println!("  - Border specs: \"border\": \"2pt solid #cccccc\"");
    println!("  - Shorthand padding: \"padding\": \"10pt\"\n");

    // Create a JSON template with CSS-like string dimensions
    let template = json!({
        "_stylesheet": {
            "defaultPageMaster": "default",
            "pageMasters": {
                "default": {
                    "size": "A4",
                    "margins": "2cm"
                }
            },
            "styles": {
                "title": {
                    "fontSize": "28pt",
                    "color": "#1a3d5c",
                    "fontWeight": "bold",
                    "marginBottom": "12pt",
                    "textAlign": "center"
                },
                "section-header": {
                    "fontSize": "18pt",
                    "color": "#2a4d69",
                    "fontWeight": "bold",
                    "marginTop": "15pt",
                    "marginBottom": "8pt",
                    "borderBottom": "2pt solid #cccccc",
                    "paddingBottom": "4pt"
                },
                "body-text": {
                    "fontSize": "12pt",
                    "lineHeight": "18pt",
                    "color": "#333333",
                    "marginBottom": "6pt"
                },
                "highlight-box": {
                    "fontSize": "11pt",
                    "backgroundColor": "#f0f8ff",
                    "border": "1pt solid #4682b4",
                    "padding": "10pt",
                    "marginTop": "8pt",
                    "marginBottom": "8pt"
                },
                "code-block": {
                    "fontFamily": "Courier",
                    "fontSize": "10pt",
                    "backgroundColor": "#f5f5f5",
                    "border": "1pt solid #dddddd",
                    "padding": "8pt",
                    "marginTop": "6pt",
                    "marginBottom": "6pt"
                }
            }
        },
        "_template": {
            "type": "Block",
            "children": [
                {
                    "type": "Paragraph",
                    "styleNames": ["title"],
                    "children": [
                        { "type": "Text", "content": "CSS-Like Dimension Parsing Demo" }
                    ]
                },
                {
                    "type": "Paragraph",
                    "styleNames": ["section-header"],
                    "children": [
                        { "type": "Text", "content": "String Dimensions" }
                    ]
                },
                {
                    "type": "Paragraph",
                    "styleNames": ["body-text"],
                    "children": [
                        {
                            "type": "Text",
                            "content": "JSON templates now support CSS-like string dimensions. Instead of requiring numeric values, you can use intuitive strings like \"24pt\", \"1.5cm\", or \"18pt\" for sizes."
                        }
                    ]
                },
                {
                    "type": "Paragraph",
                    "styleNames": ["highlight-box"],
                    "children": [
                        {
                            "type": "StyledSpan",
                            "styleOverride": { "fontWeight": "bold" },
                            "children": [
                                { "type": "Text", "content": "Note: " }
                            ]
                        },
                        {
                            "type": "Text",
                            "content": "This paragraph uses backgroundColor, border, and padding all specified with string dimensions in the stylesheet."
                        }
                    ]
                },
                {
                    "type": "Paragraph",
                    "styleNames": ["section-header"],
                    "children": [
                        { "type": "Text", "content": "Color Parsing" }
                    ]
                },
                {
                    "type": "Paragraph",
                    "styleNames": ["body-text"],
                    "children": [
                        {
                            "type": "Text",
                            "content": "Colors can be specified as hex values (#RGB or #RRGGBB) or named colors (black, white, red, green, blue). The title uses #1a3d5c, section headers use #2a4d69."
                        }
                    ]
                },
                {
                    "type": "Paragraph",
                    "styleNames": ["section-header"],
                    "children": [
                        { "type": "Text", "content": "Border Specifications" }
                    ]
                },
                {
                    "type": "Paragraph",
                    "styleNames": ["body-text"],
                    "children": [
                        {
                            "type": "Text",
                            "content": "Borders can use CSS-like syntax: \"2pt solid #cccccc\" combines width, style, and color in one string. Section headers use this feature for their bottom border."
                        }
                    ]
                },
                {
                    "type": "Paragraph",
                    "styleNames": ["code-block"],
                    "children": [
                        {
                            "type": "Text",
                            "content": "Example stylesheet:\n  \"fontSize\": \"12pt\",\n  \"color\": \"#2a4d69\",\n  \"border\": \"1pt solid #dddddd\",\n  \"padding\": \"8pt\""
                        }
                    ]
                },
                {
                    "type": "Paragraph",
                    "styleNames": ["section-header"],
                    "children": [
                        { "type": "Text", "content": "Flexible Field Naming" }
                    ]
                },
                {
                    "type": "Paragraph",
                    "styleNames": ["body-text"],
                    "children": [
                        {
                            "type": "Text",
                            "content": "The deserializer supports both camelCase (fontSize, marginTop) and kebab-case (font-size, margin-top) field names, making it easy to migrate from existing CSS or match your preferred coding style."
                        }
                    ]
                }
            ]
        }
    });

    // Sample data (not used in this template, but required by pipeline)
    let data = json!({});

    // Write template to a temporary file
    let template_str = serde_json::to_string_pretty(&template)?;
    std::fs::write("css_dimensions_template.json", &template_str)?;
    println!("✓ Template with CSS-like dimensions created");

    // Build and run the pipeline
    let pipeline = PipelineBuilder::new()
        .with_template_file("css_dimensions_template.json")?
        .build()?;
    println!("✓ Pipeline built with JSON engine");

    let output_path = "test_dimensions_output.pdf";
    pipeline.generate_to_file(vec![data], output_path)?;

    println!("\n✓ Success! Generated {}", output_path);
    println!("\nOpen the PDF to see:");
    println!("  • Different font sizes from string dimensions");
    println!("  • Hex and named colors in action");
    println!("  • Borders with compound specifications");
    println!("  • Padding and margins from string values");

    Ok(())
}
