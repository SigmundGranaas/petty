use petty_core::core::layout::LayoutEngine;
use petty_core::error::PipelineError;
use crate::pipeline::api::{Anchor, Document, Heading, Hyperlink, PreparedDataSources};
use crate::pipeline::concurrency::{
    producer_task, run_in_order_streaming_consumer, spawn_workers,
};
use crate::pipeline::context::PipelineContext;
use crate::pipeline::provider::DataSourceProvider;
use petty_core::render::lopdf_renderer::LopdfRenderer;
use petty_core::render::renderer::Pass1Result;
use petty_core::render::DocumentRenderer;
use chrono::Utc;
use log::info;
use serde_json::Value;
use std::io::{BufWriter, Cursor, Seek, SeekFrom};
use std::sync::Arc;
use tokio::sync::Semaphore;
use tokio::task;

/// A data source provider that performs a full analysis pass on the data.
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
        let num_layout_threads = num_cpus::get().saturating_sub(1).clamp(2, 6);
        let channel_buffer_size = num_layout_threads;

        let max_in_flight = num_layout_threads + 2;
        let semaphore = Arc::new(Semaphore::new(max_in_flight));

        info!(
            "Starting Metadata Generating Provider pipeline with {} layout workers (Max in-flight: {}).",
            num_layout_threads, max_in_flight
        );

        let (tx1, rx1) = async_channel::bounded(channel_buffer_size);
        let (tx2, rx2) = async_channel::bounded(channel_buffer_size);

        let producer = task::spawn(producer_task(data_iterator, tx1, semaphore.clone()));
        let workers = spawn_workers(num_layout_threads, context, rx1, tx2);

        // --- Analysis Pass (Render to Temporary Storage via In-Order Streaming Consumer) ---
        #[cfg(feature = "tempfile")]
        {
            info!("[METADATA] Starting analysis pass, streaming render to tempfile (native).");
        }
        #[cfg(not(feature = "tempfile"))]
        {
            info!("[METADATA] Starting analysis pass, streaming render to in-memory buffer (WASM).");
        }

        // Use tempfile on native platforms for memory efficiency, in-memory buffer for WASM
        #[cfg(feature = "tempfile")]
        let buf_writer = {
            let temp_file = tempfile::tempfile()?;
            BufWriter::new(temp_file)
        };

        #[cfg(not(feature = "tempfile"))]
        let buf_writer = {
            let memory_buffer = Cursor::new(Vec::new());
            BufWriter::new(memory_buffer)
        };

        let pass1_result: Pass1Result;

        let final_buf_writer = {
            let final_layout_engine = LayoutEngine::new(&context.font_library, context.cache_config);
            let final_stylesheet = context.compiled_template.stylesheet();

            // Pass Arc<Stylesheet> correctly
            let mut renderer = LopdfRenderer::new(final_layout_engine, final_stylesheet.clone())?;
            renderer.begin_document(buf_writer)?;

            let (page_width, page_height) =
                renderer.stylesheet.get_default_page_layout().size.dimensions_pt();

            let (page_ids, p1_result) = run_in_order_streaming_consumer(
                rx2,
                &mut renderer,
                page_width,
                page_height,
                true,
                semaphore
            )?;
            pass1_result = p1_result;

            Box::new(renderer).finish(page_ids)?
        };

        info!(
            "[METADATA] Analysis pass complete. Pass1 Result: total_pages={}, toc_entries={}, resolved_anchors={}, hyperlinks={}",
            pass1_result.total_pages,
            pass1_result.toc_entries.len(),
            pass1_result.resolved_anchors.len(),
            pass1_result.hyperlink_locations.len(),
        );

        producer.abort();
        for worker in workers {
            worker.abort();
        }

        let document = build_document_from_pass1_result(pass1_result);

        let mut temp_file = final_buf_writer.into_inner().map_err(|e| {
            PipelineError::Io(e.into_error())
        })?;

        temp_file.seek(SeekFrom::Start(0))?;

        Ok(PreparedDataSources {
            data_iterator: Box::new(std::iter::empty()),
            document: Some(Arc::new(document)),
            body_artifact: Some(Box::new(temp_file)),
        })
    }
}

fn build_document_from_pass1_result(pass1_result: Pass1Result) -> Document {
    let headings = pass1_result
        .toc_entries
        .iter()
        .filter_map(|entry| {
            pass1_result.resolved_anchors.get(&entry.target_id).map(|anchor| {
                Heading {
                    id: entry.target_id.clone(),
                    level: entry.level,
                    text: entry.text.clone(),
                    page_number: anchor.global_page_index,
                }
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
        figures: vec![],
        index_entries: pass1_result.index_entries,
        anchors,
        hyperlinks,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use petty_core::core::layout::fonts::SharedFontLibrary;
    use petty_core::parser::json::processor::JsonParser;
    use petty_core::parser::processor::TemplateParser;
    use crate::pipeline::api::IndexEntry;
    use crate::pipeline::context::PipelineContext;
    use petty_core::render::renderer::{HyperlinkLocation, ResolvedAnchor};
    use crate::pipeline::worker::TocEntry;
    use serde_json::json;
    use std::collections::HashMap;
    use std::io::Read;
    use std::path::PathBuf;

    #[test]
    fn test_document_creation_from_pass1_result() {
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
    }

    #[tokio::test]
    async fn test_metadata_provider_integration() {
        let _ = env_logger::builder().is_test(true).try_init();

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
        let library = SharedFontLibrary::new();
        library.load_fallback_font();

        let context = PipelineContext {
            compiled_template: features.main_template,
            role_templates: Arc::new(features.role_templates),
            font_library: Arc::new(library),
            resource_provider: Arc::new(crate::resource::InMemoryResourceProvider::new()),
            executor: crate::executor::ExecutorImpl::Sync(crate::executor::SyncExecutor::new()),
            cache_config: Default::default(),
        };

        let provider = MetadataGeneratingProvider;
        let data = vec![json!({
            "section1": { "title": "First Section" },
            "section2": { "title": "Second Section" },
        })];

        let sources = tokio::task::spawn_blocking(move || {
            provider.provide(&context, data.into_iter())
        })
            .await
            .unwrap()
            .unwrap();

        let doc = sources.document.expect("Document object should be generated");
        assert_eq!(doc.page_count, 2);

        let mut artifact = sources.body_artifact.expect("Body artifact should exist");
        let mut buffer = Vec::new();
        artifact.read_to_end(&mut buffer).unwrap();
        assert!(!buffer.is_empty(), "Temporary PDF file should not be empty");
    }
}