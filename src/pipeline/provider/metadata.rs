// src/pipeline/provider/metadata.rs
// src/pipeline/provider/metadata.rs
use crate::core::layout::LayoutEngine;
use crate::error::PipelineError;
use crate::pipeline::api::{Anchor, Document, Heading, Hyperlink, PreparedDataSources};
use crate::pipeline::concurrency::{
    producer_task, run_in_order_streaming_consumer, spawn_workers,
};
use crate::pipeline::context::PipelineContext;
use crate::pipeline::provider::DataSourceProvider;
use crate::render::lopdf_renderer::LopdfRenderer;
use crate::render::renderer::Pass1Result;
use crate::render::DocumentRenderer;
use chrono::Utc;
use log::info;
use serde_json::Value;
use std::io::{Cursor, Seek, SeekFrom, Write};
use std::sync::Arc;
use tokio::task;

/// A data source provider that performs a full analysis pass on the data.
///
/// This provider consumes the entire data iterator to produce two key artifacts:
/// 1. A `Document` object containing rich metadata (headings, page count, etc.).
/// 2. A temporary file containing the pre-rendered PDF body.
///
/// These artifacts are then passed to a `ComposingRenderer` to assemble the
/// final document, including prepending content like a table of contents.
#[derive(Clone)]
pub struct MetadataGeneratingProvider;

impl DataSourceProvider for MetadataGeneratingProvider {
    fn provide<'a, I>(
        &self,
        context: &'a PipelineContext,
        data_iterator: I,
    ) -> Result<PreparedDataSources, PipelineError>
    where
        I: Iterator<Item = Value> + Send + 'static,
    {
        let num_layout_threads = num_cpus::get().saturating_sub(1).max(4);
        let channel_buffer_size = num_layout_threads;

        info!(
            "Starting Metadata Generating Provider pipeline with {} layout workers.",
            num_layout_threads
        );

        let (tx1, rx1) = async_channel::bounded(channel_buffer_size);
        let (tx2, rx2) = async_channel::bounded(channel_buffer_size);

        let producer = task::spawn(producer_task(data_iterator, tx1));
        let workers = spawn_workers(num_layout_threads, context, rx1, tx2);

        // --- Analysis Pass (Render to Tempfile via In-Order Streaming Consumer) ---
        info!("[METADATA] Starting analysis pass, streaming render to temporary file.");
        let mut temp_file = tempfile::tempfile()?;
        let pass1_result: Pass1Result;

        {
            let final_layout_engine = LayoutEngine::new(Arc::clone(&context.font_manager));
            let final_stylesheet = context.compiled_template.stylesheet();

            // Render to an in-memory buffer first for performance, then copy to tempfile.
            let mut renderer = LopdfRenderer::new(final_layout_engine, final_stylesheet.clone())?;
            renderer.begin_document(Cursor::new(Vec::new()))?;

            let (page_width, page_height) =
                renderer.stylesheet.get_default_page_layout().size.dimensions_pt();

            let (page_ids, p1_result) = run_in_order_streaming_consumer(
                rx2,
                &mut renderer,
                page_width,
                page_height,
                true, // Enable analysis
            )?;
            pass1_result = p1_result;

            // `finish` consumes the renderer and returns the writer (our Cursor).
            let cursor_writer = Box::new(renderer).finish(page_ids)?;
            let pdf_bytes = cursor_writer.into_inner();
            temp_file.write_all(&pdf_bytes)?;
            temp_file.flush()?;
        }
        info!(
            "[METADATA] Analysis pass complete. Pass1 Result: total_pages={}, toc_entries={}, resolved_anchors={}",
            pass1_result.total_pages,
            pass1_result.toc_entries.len(),
            pass1_result.resolved_anchors.len(),
        );

        producer.abort();
        for worker in workers {
            worker.abort();
        }

        // Transform Pass1Result -> Document
        let document = build_document_from_pass1_result(pass1_result);

        // Reset temp file for reading by the next stage
        temp_file.seek(SeekFrom::Start(0))?;

        Ok(PreparedDataSources {
            data_iterator: Box::new(std::iter::empty()),
            document: Some(Arc::new(document)),
            body_artifact: Some(Box::new(temp_file)),
        })
    }
}

/// Transforms the internal `Pass1Result` into the public `Document` API object.
fn build_document_from_pass1_result(pass1_result: Pass1Result) -> Document {
    let headings = pass1_result
        .toc_entries
        .iter()
        .filter_map(|entry| {
            pass1_result.resolved_anchors.get(&entry.target_id).map(|anchor| Heading {
                id: entry.target_id.clone(),
                level: entry.level,
                text: entry.text.clone(),
                page_number: anchor.global_page_index,
            })
        })
        .collect();

    let anchors = pass1_result
        .resolved_anchors
        .iter()
        .map(|(id, anchor)| Anchor {
            id: id.clone(),
            page_number: anchor.global_page_index,
            y_position: anchor.y_pos,
        })
        .collect();

    let hyperlinks = pass1_result
        .hyperlink_locations
        .into_iter()
        .map(|loc| Hyperlink {
            page_number: loc.global_page_index,
            rect: loc.rect,
            target_id: loc.target_id,
        })
        .collect();

    Document {
        page_count: pass1_result.total_pages,
        build_timestamp: Utc::now().to_rfc3339(),
        headings,
        figures: vec![], // Not implemented yet
        index_entries: pass1_result.index_entries,
        anchors,
        hyperlinks,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::layout::FontManager;
    use crate::parser::json::processor::JsonParser;
    use crate::parser::processor::TemplateParser;
    use crate::pipeline::api::IndexEntry;
    use crate::pipeline::context::PipelineContext;
    use crate::render::renderer::{HyperlinkLocation, ResolvedAnchor};
    use crate::pipeline::worker::TocEntry;
    use serde_json::json;
    use std::collections::HashMap;
    use std::io::Read;
    use std::path::PathBuf;

    #[test]
    fn test_document_creation_from_pass1_result() {
        // Unit test the transformation logic
        let mut resolved_anchors = HashMap::new();
        resolved_anchors.insert(
            "h1".to_string(),
            ResolvedAnchor { global_page_index: 1, y_pos: 700.0 },
        );
        resolved_anchors.insert(
            "h2".to_string(),
            ResolvedAnchor { global_page_index: 2, y_pos: 650.0 },
        );
        resolved_anchors.insert(
            "some-other-anchor".to_string(),
            ResolvedAnchor { global_page_index: 3, y_pos: 100.0 },
        );

        let pass1_result = Pass1Result {
            total_pages: 5,
            toc_entries: vec![
                TocEntry { level: 1, text: "Heading 1".to_string(), target_id: "h1".to_string() },
                TocEntry { level: 2, text: "Heading 2".to_string(), target_id: "h2".to_string() },
                // This entry has no matching anchor and should be ignored
                TocEntry {
                    level: 2,
                    text: "No Anchor".to_string(),
                    target_id: "nonexistent".to_string(),
                },
            ],
            resolved_anchors,
            hyperlink_locations: vec![HyperlinkLocation {
                global_page_index: 1,
                rect: [10.0, 20.0, 30.0, 40.0],
                target_id: "h2".to_string(),
            }],
            index_entries: vec![IndexEntry { text: "Rust".to_string(), page_number: 4 }],
            ..Default::default()
        };

        let doc = build_document_from_pass1_result(pass1_result);

        assert_eq!(doc.page_count, 5);
        assert_eq!(doc.headings.len(), 2);
        assert_eq!(doc.anchors.len(), 3);
        assert_eq!(doc.hyperlinks.len(), 1);
        assert_eq!(doc.index_entries.len(), 1);

        assert_eq!(doc.headings[0].id, "h1");
        assert_eq!(doc.headings[0].text, "Heading 1");
        assert_eq!(doc.headings[0].page_number, 1);

        assert_eq!(doc.headings[1].id, "h2");
        assert_eq!(doc.headings[1].text, "Heading 2");
        assert_eq!(doc.headings[1].page_number, 2);

        // Anchors should contain all resolved anchors
        let anchor1 = doc.anchors.iter().find(|a| a.id == "h1").unwrap();
        assert_eq!(anchor1.page_number, 1);
        assert_eq!(anchor1.y_position, 700.0);

        let hyperlink1 = &doc.hyperlinks[0];
        assert_eq!(hyperlink1.page_number, 1);
        assert_eq!(hyperlink1.target_id, "h2");

        let index_entry = &doc.index_entries[0];
        assert_eq!(index_entry.text, "Rust");
        assert_eq!(index_entry.page_number, 4);
    }

    #[tokio::test]
    async fn test_metadata_provider_integration() {
        let _ = env_logger::builder().is_test(true).try_init();
        // Integration test: Run the full provider and check its outputs.

        let template_json = json!({
            "_stylesheet": {
                "defaultPageMaster": "default",
                "pageMasters": { "default": { "size": "A4", "margins": "1cm" } },
                "styles": { "default": { "font-family": "Helvetica" } }
            },
            "_template": { "type": "Block", "children": [
                { "type": "Heading", "level": 1, "id": "sec1", "children": [ { "type": "Text", "content": "{{section1.title}}" } ] },
                { "type": "IndexMarker", "term": "first" },
                { "type": "PageBreak" },
                { "type": "Heading", "level": 1, "id": "sec2", "children": [ { "type": "Text", "content": "{{section2.title}}" } ] }
            ]}
        });
        let template_str = serde_json::to_string(&template_json).unwrap();
        let parser = JsonParser;
        let features = parser.parse(&template_str, PathBuf::new()).unwrap();
        let mut font_manager = FontManager::new();
        font_manager.load_fallback_font();
        let context = PipelineContext {
            compiled_template: features.main_template,
            role_templates: Arc::new(features.role_templates),
            font_manager: Arc::new(font_manager),
        };

        let provider = MetadataGeneratingProvider;
        let data = vec![json!({
            "section1": { "title": "First Section" },
            "section2": { "title": "Second Section" },
        })];

        // The provider's logic is blocking, so spawn it.
        let sources = tokio::task::spawn_blocking(move || {
            provider.provide(&context, data.into_iter())
        })
            .await
            .unwrap()
            .unwrap();

        // 1. Assert Document object
        let doc = sources.document.expect("Document object should be generated");
        assert_eq!(doc.page_count, 2);
        assert_eq!(doc.headings.len(), 2);
        assert_eq!(doc.headings[0].id, "sec1");
        assert_eq!(doc.headings[0].text, "First Section");
        assert_eq!(doc.headings[0].page_number, 1);
        assert_eq!(doc.headings[1].id, "sec2");
        assert_eq!(doc.headings[1].text, "Second Section");
        assert_eq!(doc.headings[1].page_number, 2);

        assert_eq!(doc.index_entries.len(), 1, "Should have collected one index entry");
        assert_eq!(doc.index_entries[0].text, "first");
        assert_eq!(doc.index_entries[0].page_number, 1);

        // 2. Assert Body Artifact
        let mut artifact = sources.body_artifact.expect("Body artifact should exist");
        let mut buffer = Vec::new();
        artifact.read_to_end(&mut buffer).unwrap();
        assert!(!buffer.is_empty(), "Temporary PDF file should not be empty");

        let pdf_content = String::from_utf8_lossy(&buffer);
        assert!(pdf_content.starts_with("%PDF-1.7"), "Artifact should be a PDF file.");
        assert!(pdf_content.contains("First Section"), "Artifact should contain rendered text.");
        assert!(pdf_content.contains("Second Section"), "Artifact should contain rendered text.");
    }
}