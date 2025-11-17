use petty::{ PdfBackend, PipelineBuilder, PipelineError};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use serde_json::{json, Value};
use std::env;
use std::time::Instant;

/// A simple Lorem Ipsum generator for content.
fn lorem_ipsum(rng: &mut StdRng, num_paragraphs: usize) -> String {
    const WORDS: &[&str] = &[
        "lorem", "ipsum", "dolor", "sit", "amet", "consectetur", "adipiscing", "elit", "sed",
        "do", "eiusmod", "tempor", "incididunt", "ut", "labore", "et", "dolore", "magna", "aliqua",
    ];
    let mut paragraphs = Vec::new();
    for _ in 0..num_paragraphs {
        let mut paragraph_content = String::new();
        let num_sentences = rng.gen_range(3..=7);
        for s_idx in 0..num_sentences {
            let mut sentence = String::new();
            let num_words = rng.gen_range(8..=20);
            for w_idx in 0..num_words {
                let word = WORDS[rng.gen_range(0..WORDS.len())];
                if w_idx == 0 {
                    let mut c = word.chars();
                    if let Some(first) = c.next() {
                        sentence.push_str(&first.to_uppercase().to_string());
                        sentence.push_str(c.as_str());
                    }
                } else {
                    sentence.push_str(word);
                }
                if w_idx < num_words - 1 {
                    sentence.push(' ');
                }
            }
            sentence.push('.');
            if s_idx < num_sentences - 1 {
                sentence.push(' ');
            }
            paragraph_content.push_str(&sentence);
        }
        paragraphs.push(paragraph_content);
    }
    paragraphs.join("\n\n")
}

/// Generates a structured document with sections and subsections.
fn generate_structured_data(
    num_sections: usize,
    subsections_per_section: usize,
) -> Vec<Value> {
    println!(
        "Generating {} sections, each with up to {} subsections...",
        num_sections, subsections_per_section
    );
    let mut rng = StdRng::from_seed(Default::default());
    let mut sections = Vec::new();

    for i in 1..=num_sections {
        let mut subsections = Vec::new();
        for j in 1..=subsections_per_section {
            let subsection_title = format!("Subsection {}.{}", i, j);
            subsections.push(json!({
                "id": format!("sub-{}-{}", i, j),
                "title": subsection_title,
                "content": lorem_ipsum(&mut rng, 2),
            }));
        }

        let section_title = format!("Main Section {}", i);
        sections.push(json!({
            "id": format!("sec-{}", i),
            "title": section_title,
            "content": lorem_ipsum(&mut rng, 3),
            "subsections": subsections,
        }));
    }

    vec![json!({ "documentTitle": "Performance Test with Table of Contents", "sections": sections })]
}

fn main() -> Result<(), PipelineError> {
    if env::var("RUST_LOG").is_err() {
        unsafe { env::set_var("RUST_LOG", "petty=info"); }
    }
    env_logger::init();

    if cfg!(debug_assertions) {
        println!("\nWARNING: Running in debug build. For accurate results, run with `--release`.\n");
    }
    println!("Running Large Table of Contents Performance Test...");

    let args: Vec<String> = env::args().collect();
    let num_sections = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(20);
    let subsections_per_section = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(10);
    let total_headings = num_sections + (num_sections * subsections_per_section);

    println!(
        "Configuration: {} sections, {} subsections/section ({} total headings).",
        num_sections, subsections_per_section, total_headings
    );

    let template_path = "templates/perf_toc.xsl";
    println!("✓ Using template: {}", template_path);

    let data = generate_structured_data(num_sections, subsections_per_section);
    println!("✓ Data generated.");

    // For templates with a Table of Contents, `TwoPass` is the most performant strategy
    // when the data source is clonable (like a `Vec`). `Hybrid` is an alternative for
    // non-clonable streams but is generally slower.
    let pipeline = PipelineBuilder::new()
        .with_template_file(template_path)?
        .with_pdf_backend(PdfBackend::LopdfParallel)
        .with_debug(false)
        .build()?;
    println!("✓ Pipeline built with XSLT engine.");

    let output_path = "performance_toc_output.pdf";
    println!("Starting PDF generation for {} headings to {}...", total_headings, output_path);
    let start_time = Instant::now();

    pipeline.generate_to_file(data, output_path)?;

    let duration = start_time.elapsed();
    println!("\nSuccess! Generated {}", output_path);
    println!("Total time taken: {:.2} seconds.", duration.as_secs_f64());
    if total_headings > 0 {
        println!(
            "Average time per heading: {:.2} ms.",
            duration.as_millis() as f64 / total_headings as f64
        );
    }
    Ok(())
}