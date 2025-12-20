//! Synchronous pipeline for WASM environments.
//!
//! This module provides a synchronous version of the document generation pipeline
//! that doesn't require tokio or async runtimes - suitable for WASM.

use crate::error::PettyError;
use petty_core::error::PipelineError;
use petty_idf::IRNode;
use petty_layout::config::LayoutConfig;
use petty_layout::fonts::SharedFontLibrary;
use petty_layout::{LayoutEngine, LayoutStore};
use petty_render_core::DocumentRenderer;
use petty_render_lopdf::LopdfRenderer;
use petty_template_core::CompiledTemplate;
use petty_traits::ResourceProvider;
use serde_json::Value;
use std::collections::HashMap;
use std::io::Cursor;
use std::sync::Arc;

/// Configuration for the WASM pipeline.
#[derive(Clone)]
pub struct WasmPipelineConfig {
    /// The compiled template
    pub compiled_template: Arc<dyn CompiledTemplate>,
    /// Role templates (header, footer, etc.)
    /// TODO: Implement role template support
    #[allow(dead_code)]
    pub role_templates: Arc<HashMap<String, Arc<dyn CompiledTemplate>>>,
    /// Font library for text rendering
    pub font_library: Arc<SharedFontLibrary>,
    /// Resource provider for images
    /// TODO: Implement resource provider integration
    #[allow(dead_code)]
    pub resource_provider: Arc<dyn ResourceProvider>,
    /// Enable debug mode
    pub debug: bool,
}

/// A synchronous document generation pipeline for WASM.
///
/// This pipeline performs all operations synchronously, making it suitable
/// for WASM environments where tokio's blocking tasks are not available.
pub struct WasmPipeline {
    config: WasmPipelineConfig,
}

impl WasmPipeline {
    /// Create a new WASM pipeline with the given configuration.
    pub fn new(config: WasmPipelineConfig) -> Self {
        Self { config }
    }

    /// Generate a PDF document synchronously.
    ///
    /// # Arguments
    ///
    /// * `data` - The data to render (array of objects)
    ///
    /// # Returns
    ///
    /// The PDF document as a byte vector.
    pub fn generate(&self, data: Vec<Value>) -> Result<Vec<u8>, PettyError> {
        // Serialize data for template execution
        let data_json = serde_json::to_string(&data).map_err(PettyError::from)?;

        // Execute the template to get IR nodes
        let execution_config = petty_template_core::ExecutionConfig {
            format: petty_template_core::DataSourceFormat::Json,
            strict: false,
        };

        let ir_nodes = self
            .config
            .compiled_template
            .execute(&data_json, execution_config)
            .map_err(|e| PipelineError::TemplateExecution(e.to_string()))?;

        if self.config.debug {
            log::debug!("Generated {} IR nodes", ir_nodes.len());
        }

        // Get the stylesheet from the template
        let stylesheet = self.config.compiled_template.stylesheet();

        // Create layout engine with default config (no system fonts in WASM)
        let layout_config = LayoutConfig::default();
        let layout_engine = LayoutEngine::new(&self.config.font_library, layout_config);

        // Create a store for layout memory management
        let store = LayoutStore::new();

        // Wrap IR nodes in a root
        let ir_tree = IRNode::Root(ir_nodes);

        // Build the render tree
        let root_render_node = layout_engine
            .build_render_tree(&ir_tree, &store)
            .map_err(|e| PipelineError::Layout(e.to_string()))?;

        // Paginate the document
        let page_iterator = layout_engine
            .paginate(&stylesheet, root_render_node, &store)
            .map_err(|e| PipelineError::Layout(e.to_string()))?;

        // Collect all pages
        let mut pages = Vec::new();
        for page_result in page_iterator {
            let page = page_result.map_err(|e| PipelineError::Layout(e.to_string()))?;
            pages.push(page.elements);
        }

        if self.config.debug {
            log::debug!("Layout complete: {} pages", pages.len());
        }

        // Get page dimensions
        let (page_width, page_height) = stylesheet.get_default_page_layout().size.dimensions_pt();

        // Build font map (same logic as LopdfRenderer::new)
        let mut font_map = HashMap::new();
        for (i, font_info) in layout_engine.registered_fonts().iter().enumerate() {
            font_map.insert(font_info.postscript_name.clone(), format!("F{}", i + 1));
        }

        // Create PDF renderer with in-memory output
        let output: Cursor<Vec<u8>> = Cursor::new(Vec::new());
        let mut renderer: LopdfRenderer<Cursor<Vec<u8>>> =
            LopdfRenderer::new(layout_engine, stylesheet.clone())
                .map_err(|e| PipelineError::Render(e.to_string()))?;

        // Begin document
        renderer
            .begin_document(output)
            .map_err(|e| PipelineError::Render(e.to_string()))?;

        // Render each page using the DocumentRenderer trait
        let mut all_page_ids = Vec::new();
        for page_elements in pages {
            // Render page content
            let content_id = renderer
                .render_page_content(page_elements, &font_map, page_width, page_height)
                .map_err(|e| PipelineError::Render(e.to_string()))?;

            // Write page object
            let page_id = renderer
                .write_page_object(vec![content_id], vec![], page_width, page_height)
                .map_err(|e| PipelineError::Render(e.to_string()))?;

            all_page_ids.push(page_id);
        }

        // Finish the document and get the PDF bytes
        let pdf_bytes = renderer
            .finish_into_buffer(all_page_ids)
            .map_err(|e| PipelineError::Render(e.to_string()))?;

        if self.config.debug {
            log::debug!("Rendering complete: {} bytes", pdf_bytes.len());
        }

        Ok(pdf_bytes)
    }
}
