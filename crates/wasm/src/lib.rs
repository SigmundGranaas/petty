//! WebAssembly bindings for Petty PDF generation.
//!
//! This crate provides JavaScript/TypeScript bindings for the Petty PDF generation
//! engine, enabling PDF creation in browsers and Node.js environments.
//!
//! # Architecture
//!
//! The WASM bindings use a **synchronous pipeline** implementation that differs from
//! the native Rust API. All PDF generation happens synchronously within the WASM module,
//! with the JavaScript API using Promises to provide an async interface.
//!
//! ## Key Differences from Native API
//!
//! - **No tokio runtime**: Uses synchronous execution instead of async/await
//! - **No system fonts**: Must provide fonts explicitly (embedded or loaded from URLs)
//! - **In-memory only**: Generates PDF bytes in memory (no filesystem access)
//! - **LopdfRenderer only**: Uses lopdf backend (printpdf not used in WASM)
//!
//! ## Module Structure
//!
//! - [`builder`] - `PettyPdf` builder API for constructing PDF generators
//! - [`pipeline`] - Synchronous PDF generation pipeline (WASM-specific)
//! - [`fonts`] - Font loading utilities (URL-based and in-memory)
//! - [`resources`] - Resource provider for images and assets
//! - [`error`] - Error types with JavaScript interop
//! - [`types`] - TypeScript-friendly enums and configuration
//!
//! # WASM Compatibility Notes
//!
//! ## Time Measurement
//!
//! The layout engine conditionally uses the `instant` crate for profiling when the
//! `profiling` feature is enabled. By default, profiling is **disabled** in WASM builds
//! to reduce bundle size and eliminate overhead.
//!
//! ## Random Number Generation
//!
//! Uses `getrandom` with the `wasm_js` feature for generating PDF object IDs, which
//! calls into JavaScript's `crypto.getRandomValues()`.
//!
//! # Example
//!
//! ```javascript
//! import init, { PettyPdf } from '@petty/wasm';
//!
//! await init();
//!
//! const pdf = new PettyPdf()
//!   .withBuiltinFonts()
//!   .withTemplateJson(`{
//!     "_stylesheet": { "pageMasters": { "default": { "size": "A4" } } },
//!     "_template": { "type": "Paragraph", "children": [{ "type": "Text", "content": "Hello!" }] }
//!   }`);
//!
//! const bytes = await pdf.generate({});
//! const blob = new Blob([bytes], { type: 'application/pdf' });
//! ```
//!
//! # Browser Support
//!
//! Requires:
//! - WebAssembly support (all modern browsers)
//! - JavaScript ES6+ (for wasm-bindgen glue code)
//! - `crypto.getRandomValues()` (for random number generation)
//!
//! Tested on Chrome 90+, Firefox 88+, Safari 14+, Edge 90+.

mod builder;
mod error;
mod fonts;
mod pipeline;
mod resources;
mod types;

pub use builder::PettyPdf;
pub use error::PettyError;
pub use fonts::WasmFontProvider;
pub use resources::WasmResourceProvider;
pub use types::GenerationMode;

use wasm_bindgen::prelude::*;

/// Initialize the WASM module.
///
/// This function sets up panic hooks for better error messages in the browser console.
/// It is called automatically when using wasm-pack's generated JavaScript.
#[wasm_bindgen(start)]
pub fn init() {
    // Set up better panic messages
    console_error_panic_hook::set_once();

    #[cfg(feature = "console-logging")]
    {
        // Initialize console logging if the feature is enabled
        console_log::init_with_level(log::Level::Debug).ok();
    }
}

/// Get the version of the petty-wasm library.
#[wasm_bindgen(js_name = getVersion)]
pub fn get_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}
