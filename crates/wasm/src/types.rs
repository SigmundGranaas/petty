//! TypeScript-friendly type definitions.
//!
//! These types are exported to JavaScript with appropriate wasm-bindgen annotations.

use wasm_bindgen::prelude::*;

/// PDF generation mode.
///
/// Controls how the PDF is generated:
/// - `Auto`: Automatically selects the best mode based on template features
/// - `ForceStreaming`: Forces single-pass streaming mode (faster, but no TOC/page numbers)
#[wasm_bindgen]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum GenerationMode {
    /// Automatically select mode based on template features.
    /// Uses streaming for simple templates, composing for advanced features.
    #[default]
    Auto,
    /// Force single-pass streaming mode.
    /// Faster but doesn't support TOC, page numbers, or cross-references.
    ForceStreaming,
}

/// Font weight for font loading.
#[wasm_bindgen]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FontWeight {
    Thin,
    Light,
    #[default]
    Regular,
    Medium,
    Bold,
    Black,
}

impl From<FontWeight> for petty_style::font::FontWeight {
    fn from(weight: FontWeight) -> Self {
        match weight {
            FontWeight::Thin => petty_style::font::FontWeight::Thin,
            FontWeight::Light => petty_style::font::FontWeight::Light,
            FontWeight::Regular => petty_style::font::FontWeight::Regular,
            FontWeight::Medium => petty_style::font::FontWeight::Medium,
            FontWeight::Bold => petty_style::font::FontWeight::Bold,
            FontWeight::Black => petty_style::font::FontWeight::Black,
        }
    }
}

/// Font style for font loading.
#[wasm_bindgen]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FontStyle {
    #[default]
    Normal,
    Italic,
    Oblique,
}

impl From<FontStyle> for petty_style::font::FontStyle {
    fn from(style: FontStyle) -> Self {
        match style {
            FontStyle::Normal => petty_style::font::FontStyle::Normal,
            FontStyle::Italic => petty_style::font::FontStyle::Italic,
            FontStyle::Oblique => petty_style::font::FontStyle::Oblique,
        }
    }
}

/// Parse a font weight from a string.
///
/// Accepts CSS-style weight names or numeric values (100-900).
pub fn parse_font_weight(weight: Option<&str>) -> petty_style::font::FontWeight {
    match weight {
        Some("thin") | Some("100") => petty_style::font::FontWeight::Thin,
        Some("extralight") | Some("extra-light") | Some("200") => {
            petty_style::font::FontWeight::Numeric(200)
        }
        Some("light") | Some("300") => petty_style::font::FontWeight::Light,
        Some("regular") | Some("normal") | Some("400") | None => {
            petty_style::font::FontWeight::Regular
        }
        Some("medium") | Some("500") => petty_style::font::FontWeight::Medium,
        Some("semibold") | Some("semi-bold") | Some("600") => {
            petty_style::font::FontWeight::Numeric(600)
        }
        Some("bold") | Some("700") => petty_style::font::FontWeight::Bold,
        Some("extrabold") | Some("extra-bold") | Some("800") => {
            petty_style::font::FontWeight::Numeric(800)
        }
        Some("black") | Some("900") => petty_style::font::FontWeight::Black,
        Some(other) => {
            // Try to parse as a number
            if let Ok(n) = other.parse::<u16>() {
                petty_style::font::FontWeight::Numeric(n)
            } else {
                petty_style::font::FontWeight::Regular
            }
        }
    }
}

/// Parse a font style from a string.
pub fn parse_font_style(style: Option<&str>) -> petty_style::font::FontStyle {
    match style {
        Some("italic") => petty_style::font::FontStyle::Italic,
        Some("oblique") => petty_style::font::FontStyle::Oblique,
        Some("normal") | None => petty_style::font::FontStyle::Normal,
        Some(_) => petty_style::font::FontStyle::Normal,
    }
}
