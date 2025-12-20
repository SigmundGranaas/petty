//! Resource provider for WASM environments.
//!
//! Provides resource loading via:
//! - User-provided resource bytes
//! - URL-based resource loading via fetch API

use crate::error::PettyError;
use petty_traits::{InMemoryResourceProvider, ResourceError, ResourceProvider, SharedResourceData};
use std::sync::Arc;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

/// A resource provider for WASM environments.
///
/// Wraps `InMemoryResourceProvider` with additional functionality for:
/// - Loading resources from URLs
#[wasm_bindgen]
#[derive(Debug, Clone)]
pub struct WasmResourceProvider {
    inner: Arc<InMemoryResourceProvider>,
}

#[wasm_bindgen]
impl WasmResourceProvider {
    /// Create a new empty resource provider.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            inner: Arc::new(InMemoryResourceProvider::new()),
        }
    }

    /// Add a resource from raw bytes.
    ///
    /// # Arguments
    ///
    /// * `path` - The path/key to store the resource under (used in templates)
    /// * `data` - The resource data (image bytes, etc.)
    #[wasm_bindgen(js_name = addResource)]
    pub fn add_resource(&self, path: &str, data: &[u8]) -> Result<(), JsValue> {
        self.inner
            .add(path, data.to_vec())
            .map_err(|e| PettyError::resource(e.to_string()))?;
        Ok(())
    }

    /// Check if a resource exists.
    #[wasm_bindgen]
    pub fn exists(&self, path: &str) -> bool {
        self.inner.exists(path)
    }

    /// Remove a resource.
    #[wasm_bindgen]
    pub fn remove(&self, path: &str) -> bool {
        self.inner.remove(path).is_some()
    }

    /// Clear all resources.
    #[wasm_bindgen]
    pub fn clear(&self) {
        self.inner.clear();
    }

    /// Get the number of resources.
    #[wasm_bindgen(getter)]
    pub fn count(&self) -> usize {
        self.inner.len()
    }

    /// Check if the provider has no resources.
    #[wasm_bindgen(js_name = isEmpty)]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

impl Default for WasmResourceProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl WasmResourceProvider {
    /// Get the inner `InMemoryResourceProvider`.
    pub fn inner(&self) -> &Arc<InMemoryResourceProvider> {
        &self.inner
    }

    /// Create a resource provider that implements `ResourceProvider` trait.
    pub fn as_resource_provider(&self) -> Arc<dyn ResourceProvider> {
        self.inner.clone()
    }

    /// Add a resource from shared data.
    pub fn add_shared(&self, path: &str, data: SharedResourceData) -> Result<(), ResourceError> {
        self.inner.add_shared(path, data)
    }
}

/// Fetch a resource from a URL.
///
/// This is an async function that fetches resource data from a URL using the Fetch API.
pub async fn fetch_resource(url: &str) -> Result<Vec<u8>, PettyError> {
    let window =
        web_sys::window().ok_or_else(|| PettyError::resource("No window object available"))?;

    let opts = web_sys::RequestInit::new();
    opts.set_method("GET");
    opts.set_mode(web_sys::RequestMode::Cors);

    let request = web_sys::Request::new_with_str_and_init(url, &opts)
        .map_err(|e| PettyError::resource(format!("Failed to create request: {:?}", e)))?;

    let response_value = JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(|e| PettyError::resource(format!("Fetch failed: {:?}", e)))?;

    let response: web_sys::Response = response_value
        .dyn_into()
        .map_err(|_| PettyError::resource("Failed to convert response"))?;

    if !response.ok() {
        return Err(PettyError::resource(format!(
            "HTTP error: {} {}",
            response.status(),
            response.status_text()
        )));
    }

    let array_buffer = JsFuture::from(
        response
            .array_buffer()
            .map_err(|e| PettyError::resource(format!("Failed to get array buffer: {:?}", e)))?,
    )
    .await
    .map_err(|e| PettyError::resource(format!("Failed to read response body: {:?}", e)))?;

    let uint8_array = js_sys::Uint8Array::new(&array_buffer);
    Ok(uint8_array.to_vec())
}
