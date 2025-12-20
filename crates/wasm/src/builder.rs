//! PettyPdf builder for WASM.
//!
//! This module provides the main entry point for PDF generation in JavaScript.

use crate::error::PettyError;
use crate::fonts::{WasmFontProvider, fetch_font};
use crate::pipeline::{WasmPipeline, WasmPipelineConfig};
use crate::resources::{WasmResourceProvider, fetch_resource};
use crate::types::GenerationMode;
use petty_json_template::JsonParser;
use petty_layout::fonts::SharedFontLibrary;
use petty_template_core::TemplateParser;
use serde_json::Value;
use std::path::PathBuf;
use std::sync::Arc;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::future_to_promise;

/// The main entry point for PDF generation in JavaScript.
///
/// # Example
///
/// ```javascript
/// const pdf = new PettyPdf()
///   .withBuiltinFonts()
///   .withTemplateJson(`{
///     "_stylesheet": { "pageMasters": { "default": { "size": "A4" } } },
///     "_template": { "type": "Paragraph", "children": [{ "type": "Text", "content": "Hello!" }] }
///   }`);
///
/// const bytes = await pdf.generate({});
/// ```
#[wasm_bindgen]
pub struct PettyPdf {
    font_provider: WasmFontProvider,
    resource_provider: WasmResourceProvider,
    template_source: Option<String>,
    generation_mode: GenerationMode,
    debug: bool,
}

#[wasm_bindgen]
impl PettyPdf {
    /// Create a new PettyPdf builder.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        // Panic hook is set in lib.rs init()
        Self {
            font_provider: WasmFontProvider::new(),
            resource_provider: WasmResourceProvider::new(),
            template_source: None,
            generation_mode: GenerationMode::Auto,
            debug: false,
        }
    }

    /// Set the template from a JSON string.
    ///
    /// The JSON should contain `_stylesheet` and `_template` keys.
    #[wasm_bindgen(js_name = withTemplateJson)]
    pub fn with_template_json(mut self, source: &str) -> Self {
        self.template_source = Some(source.to_string());
        self
    }

    /// Set the template from a JavaScript object.
    ///
    /// The object will be serialized to JSON internally.
    #[wasm_bindgen(js_name = withTemplateObject)]
    pub fn with_template_object(mut self, template: JsValue) -> Result<PettyPdf, JsValue> {
        let json: Value = serde_wasm_bindgen::from_value(template)
            .map_err(|e| PettyError::config(format!("Invalid template object: {}", e)))?;
        self.template_source = Some(
            serde_json::to_string(&json)
                .map_err(|e| PettyError::config(format!("Failed to serialize template: {}", e)))?,
        );
        Ok(self)
    }

    /// Load the built-in Liberation fonts.
    ///
    /// This adds Liberation Sans and Liberation Mono fonts for basic text rendering.
    #[wasm_bindgen(js_name = withBuiltinFonts)]
    pub fn with_builtin_fonts(self) -> Result<PettyPdf, JsValue> {
        self.font_provider.load_builtin_fonts()?;
        Ok(self)
    }

    /// Add a font from raw bytes.
    ///
    /// # Arguments
    ///
    /// * `family` - The font family name (e.g., "My Font")
    /// * `data` - The font file data (TTF/OTF)
    /// * `weight` - Optional font weight ("regular", "bold", "700", etc.)
    /// * `style` - Optional font style ("normal", "italic")
    #[wasm_bindgen(js_name = addFontFromBytes)]
    pub fn add_font_from_bytes(
        self,
        family: &str,
        data: &[u8],
        weight: Option<String>,
        style: Option<String>,
    ) -> Result<PettyPdf, JsValue> {
        self.font_provider
            .add_font_from_bytes(family, data, weight, style)?;
        Ok(self)
    }

    /// Add a font from a URL.
    ///
    /// This is an async method that fetches the font data and adds it.
    ///
    /// # Arguments
    ///
    /// * `family` - The font family name
    /// * `url` - The URL to fetch the font from
    /// * `weight` - Optional font weight
    /// * `style` - Optional font style
    #[wasm_bindgen(js_name = addFontFromUrl)]
    pub fn add_font_from_url(
        self,
        family: String,
        url: String,
        weight: Option<String>,
        style: Option<String>,
    ) -> js_sys::Promise {
        let font_provider = self.font_provider.clone();
        let resource_provider = self.resource_provider.clone();
        let template_source = self.template_source.clone();
        let generation_mode = self.generation_mode;
        let debug = self.debug;

        future_to_promise(async move {
            let font_data = fetch_font(&url).await?;

            font_provider.add_font_from_bytes(&family, &font_data, weight, style)?;

            Ok(JsValue::from(PettyPdf {
                font_provider,
                resource_provider,
                template_source,
                generation_mode,
                debug,
            }))
        })
    }

    /// Add a resource (image, etc.) from raw bytes.
    ///
    /// # Arguments
    ///
    /// * `path` - The path to use in templates (e.g., "logo.png")
    /// * `data` - The resource data
    #[wasm_bindgen(js_name = addResource)]
    pub fn add_resource(self, path: &str, data: &[u8]) -> Result<PettyPdf, JsValue> {
        self.resource_provider.add_resource(path, data)?;
        Ok(self)
    }

    /// Add a resource from a URL.
    ///
    /// # Arguments
    ///
    /// * `path` - The path to use in templates
    /// * `url` - The URL to fetch the resource from
    #[wasm_bindgen(js_name = addResourceFromUrl)]
    pub fn add_resource_from_url(self, path: String, url: String) -> js_sys::Promise {
        let font_provider = self.font_provider.clone();
        let resource_provider = self.resource_provider.clone();
        let template_source = self.template_source.clone();
        let generation_mode = self.generation_mode;
        let debug = self.debug;

        future_to_promise(async move {
            let resource_data = fetch_resource(&url).await?;

            resource_provider.add_resource(&path, &resource_data)?;

            Ok(JsValue::from(PettyPdf {
                font_provider,
                resource_provider,
                template_source,
                generation_mode,
                debug,
            }))
        })
    }

    /// Set the generation mode.
    ///
    /// * `Auto` - Automatically select based on template features
    /// * `ForceStreaming` - Force single-pass streaming mode
    #[wasm_bindgen(js_name = withGenerationMode)]
    pub fn with_generation_mode(mut self, mode: GenerationMode) -> Self {
        self.generation_mode = mode;
        self
    }

    /// Enable or disable debug mode.
    ///
    /// When enabled, additional logging will be output to the console.
    #[wasm_bindgen(js_name = withDebug)]
    pub fn with_debug(mut self, enabled: bool) -> Self {
        self.debug = enabled;
        self
    }

    /// Generate a PDF from the provided data.
    ///
    /// # Arguments
    ///
    /// * `data` - The data to render (single object or array of objects)
    ///
    /// # Returns
    ///
    /// A Promise that resolves to a Uint8Array containing the PDF bytes.
    #[wasm_bindgen]
    pub fn generate(&self, data: JsValue) -> js_sys::Promise {
        let template_source = self.template_source.clone();
        let font_provider = self.font_provider.clone();
        let resource_provider = self.resource_provider.clone();
        let debug = self.debug;

        future_to_promise(async move {
            // Parse the data
            let data_vec = parse_data(data)?;

            // Build and run the pipeline
            let pdf_bytes = generate_pdf_sync(
                template_source,
                font_provider,
                resource_provider,
                data_vec,
                debug,
            )?;

            // Convert to Uint8Array
            Ok(js_sys::Uint8Array::from(&pdf_bytes[..]).into())
        })
    }
}

impl Default for PettyPdf {
    fn default() -> Self {
        Self::new()
    }
}

/// Parse JavaScript data into a Vec<Value>.
fn parse_data(data: JsValue) -> Result<Vec<Value>, PettyError> {
    let value: Value = serde_wasm_bindgen::from_value(data)
        .map_err(|e| PettyError::config(format!("Invalid data: {}", e)))?;

    match value {
        Value::Array(arr) => Ok(arr),
        Value::Object(_) => Ok(vec![value]),
        Value::Null => Ok(vec![Value::Object(serde_json::Map::new())]),
        _ => Err(PettyError::config(
            "Data must be an object or array of objects",
        )),
    }
}

/// Generate PDF synchronously.
fn generate_pdf_sync(
    template_source: Option<String>,
    font_provider: WasmFontProvider,
    resource_provider: WasmResourceProvider,
    data: Vec<Value>,
    debug: bool,
) -> Result<Vec<u8>, PettyError> {
    let template_source =
        template_source.ok_or_else(|| PettyError::config("No template configured"))?;

    // Parse the template
    let parser = JsonParser;
    let template_features = parser
        .parse(&template_source, PathBuf::new())
        .map_err(|e| PettyError::config(format!("Template parse error: {}", e)))?;

    // Create font library from provider
    let font_library = SharedFontLibrary::from_provider(font_provider.as_font_provider());

    // Create pipeline config
    let config = WasmPipelineConfig {
        compiled_template: template_features.main_template,
        role_templates: Arc::new(template_features.role_templates),
        font_library: Arc::new(font_library),
        resource_provider: resource_provider.as_resource_provider(),
        debug,
    };

    // Run the pipeline
    let pipeline = WasmPipeline::new(config);
    pipeline.generate(data)
}
