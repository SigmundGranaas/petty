//! Font provider for WASM environments.
//!
//! Provides font loading via:
//! - Embedded fonts (Liberation family, feature-gated)
//! - User-provided font bytes
//! - URL-based font loading via fetch API

use crate::error::PettyError;
use crate::types::{parse_font_style, parse_font_weight};
#[cfg(feature = "embedded-fonts")]
use petty_style::font::{FontStyle, FontWeight};
use petty_traits::{FontProvider, InMemoryFontProvider};
use std::sync::Arc;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

// Embedded fonts (Liberation family)
#[cfg(feature = "embedded-fonts")]
mod embedded {
    pub static LIBERATION_SANS_REGULAR: &[u8] =
        include_bytes!("../assets/LiberationSans-Regular.ttf");
    pub static LIBERATION_SANS_BOLD: &[u8] = include_bytes!("../assets/LiberationSans-Bold.ttf");
    pub static LIBERATION_SANS_ITALIC: &[u8] =
        include_bytes!("../assets/LiberationSans-Italic.ttf");
    pub static LIBERATION_SANS_BOLD_ITALIC: &[u8] =
        include_bytes!("../assets/LiberationSans-BoldItalic.ttf");
    pub static LIBERATION_MONO_REGULAR: &[u8] =
        include_bytes!("../assets/LiberationMono-Regular.ttf");
}

/// A font provider for WASM environments.
///
/// Wraps `InMemoryFontProvider` with additional functionality for:
/// - Loading embedded fonts
/// - Loading fonts from URLs
#[wasm_bindgen]
#[derive(Debug, Clone)]
pub struct WasmFontProvider {
    inner: Arc<InMemoryFontProvider>,
}

#[wasm_bindgen]
impl WasmFontProvider {
    /// Create a new empty font provider.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            inner: Arc::new(InMemoryFontProvider::new()),
        }
    }

    /// Load the built-in Liberation fonts.
    ///
    /// This adds Liberation Sans (Regular, Bold, Italic, Bold Italic) and
    /// Liberation Mono (Regular) to the font provider.
    #[cfg(feature = "embedded-fonts")]
    #[wasm_bindgen(js_name = loadBuiltinFonts)]
    pub fn load_builtin_fonts(&self) -> Result<(), JsValue> {
        // Liberation Sans
        self.inner
            .add_font(
                "Liberation Sans",
                FontWeight::Regular,
                FontStyle::Normal,
                embedded::LIBERATION_SANS_REGULAR.to_vec(),
            )
            .map_err(|e| PettyError::font(e.to_string()))?;

        self.inner
            .add_font(
                "Liberation Sans",
                FontWeight::Bold,
                FontStyle::Normal,
                embedded::LIBERATION_SANS_BOLD.to_vec(),
            )
            .map_err(|e| PettyError::font(e.to_string()))?;

        self.inner
            .add_font(
                "Liberation Sans",
                FontWeight::Regular,
                FontStyle::Italic,
                embedded::LIBERATION_SANS_ITALIC.to_vec(),
            )
            .map_err(|e| PettyError::font(e.to_string()))?;

        self.inner
            .add_font(
                "Liberation Sans",
                FontWeight::Bold,
                FontStyle::Italic,
                embedded::LIBERATION_SANS_BOLD_ITALIC.to_vec(),
            )
            .map_err(|e| PettyError::font(e.to_string()))?;

        // Liberation Mono
        self.inner
            .add_font(
                "Liberation Mono",
                FontWeight::Regular,
                FontStyle::Normal,
                embedded::LIBERATION_MONO_REGULAR.to_vec(),
            )
            .map_err(|e| PettyError::font(e.to_string()))?;

        // Add aliases for common font families
        self.inner
            .add_font_shared(
                "sans-serif",
                FontWeight::Regular,
                FontStyle::Normal,
                Arc::new(embedded::LIBERATION_SANS_REGULAR.to_vec()),
            )
            .map_err(|e| PettyError::font(e.to_string()))?;

        self.inner
            .add_font_shared(
                "monospace",
                FontWeight::Regular,
                FontStyle::Normal,
                Arc::new(embedded::LIBERATION_MONO_REGULAR.to_vec()),
            )
            .map_err(|e| PettyError::font(e.to_string()))?;

        Ok(())
    }

    /// Stub for when embedded-fonts feature is disabled.
    #[cfg(not(feature = "embedded-fonts"))]
    #[wasm_bindgen(js_name = loadBuiltinFonts)]
    pub fn load_builtin_fonts(&self) -> Result<(), JsValue> {
        Err(PettyError::font(
            "Embedded fonts not available. Build with 'embedded-fonts' feature enabled.",
        )
        .into())
    }

    /// Add a font from raw bytes.
    ///
    /// # Arguments
    ///
    /// * `family` - The font family name (e.g., "Arial", "My Custom Font")
    /// * `data` - The font file data (TTF/OTF bytes)
    /// * `weight` - Optional font weight (e.g., "regular", "bold", "700")
    /// * `style` - Optional font style (e.g., "normal", "italic")
    #[wasm_bindgen(js_name = addFontFromBytes)]
    pub fn add_font_from_bytes(
        &self,
        family: &str,
        data: &[u8],
        weight: Option<String>,
        style: Option<String>,
    ) -> Result<(), JsValue> {
        let weight = parse_font_weight(weight.as_deref());
        let style = parse_font_style(style.as_deref());

        self.inner
            .add_font(family, weight, style, data.to_vec())
            .map_err(|e| PettyError::font(e.to_string()))?;

        Ok(())
    }

    /// Get the number of fonts in the provider.
    #[wasm_bindgen(getter)]
    pub fn count(&self) -> usize {
        self.inner.len()
    }

    /// Check if the provider has no fonts.
    #[wasm_bindgen(js_name = isEmpty)]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// List all available font families.
    #[wasm_bindgen(js_name = listFamilies)]
    pub fn list_families(&self) -> Vec<String> {
        self.inner.list_families()
    }
}

impl Default for WasmFontProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl WasmFontProvider {
    /// Get the inner `InMemoryFontProvider`.
    pub fn inner(&self) -> &Arc<InMemoryFontProvider> {
        &self.inner
    }

    /// Create a font provider that implements `FontProvider` trait.
    pub fn as_font_provider(&self) -> Arc<dyn FontProvider> {
        self.inner.clone()
    }
}

/// Fetch a font from a URL.
///
/// This is an async function that fetches font data from a URL using the Fetch API.
pub async fn fetch_font(url: &str) -> Result<Vec<u8>, PettyError> {
    let window = web_sys::window().ok_or_else(|| PettyError::font("No window object available"))?;

    let opts = web_sys::RequestInit::new();
    opts.set_method("GET");
    opts.set_mode(web_sys::RequestMode::Cors);

    let request = web_sys::Request::new_with_str_and_init(url, &opts)
        .map_err(|e| PettyError::font(format!("Failed to create request: {:?}", e)))?;

    let response_value = JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(|e| PettyError::font(format!("Fetch failed: {:?}", e)))?;

    let response: web_sys::Response = response_value
        .dyn_into()
        .map_err(|_| PettyError::font("Failed to convert response"))?;

    if !response.ok() {
        return Err(PettyError::font(format!(
            "HTTP error: {} {}",
            response.status(),
            response.status_text()
        )));
    }

    let array_buffer = JsFuture::from(
        response
            .array_buffer()
            .map_err(|e| PettyError::font(format!("Failed to get array buffer: {:?}", e)))?,
    )
    .await
    .map_err(|e| PettyError::font(format!("Failed to read response body: {:?}", e)))?;

    let uint8_array = js_sys::Uint8Array::new(&array_buffer);
    Ok(uint8_array.to_vec())
}

// Re-export types for internal use
pub use petty_traits::FontProvider as FontProviderTrait;
