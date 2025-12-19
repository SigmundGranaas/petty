//! FontProvider trait for abstracting font loading and discovery.
//!
//! This trait allows the layout engine to access fonts without being
//! tied to system font discovery or filesystem access.

use std::fmt::Debug;
use std::sync::Arc;
use thiserror::Error;

use crate::core::style::font::{FontStyle, FontWeight};

/// Error type for font loading operations.
#[derive(Error, Debug, Clone)]
pub enum FontError {
    #[error("Font not found: {family} (weight: {weight:?}, style: {style:?})")]
    NotFound {
        family: String,
        weight: FontWeight,
        style: FontStyle,
    },

    #[error("Failed to load font '{path}': {message}")]
    LoadFailed { path: String, message: String },

    #[error("Invalid font data: {0}")]
    InvalidData(String),

    #[error("Font parsing error: {0}")]
    ParseError(String),
}

/// Shared font data type (reference-counted bytes).
pub type SharedFontData = Arc<Vec<u8>>;

/// Descriptor for a font face available in a provider.
#[derive(Debug, Clone)]
pub struct FontDescriptor {
    /// The font family name (e.g., "Helvetica", "Arial")
    pub family: String,
    /// The font weight
    pub weight: FontWeight,
    /// The font style (normal, italic, oblique)
    pub style: FontStyle,
    /// PostScript name if available
    pub postscript_name: Option<String>,
}

/// A query for finding a font.
#[derive(Debug, Clone)]
pub struct FontQuery<'a> {
    /// Primary family name to search for
    pub family: &'a str,
    /// Fallback families to try if primary is not found
    pub fallbacks: &'a [&'a str],
    /// Desired font weight
    pub weight: FontWeight,
    /// Desired font style
    pub style: FontStyle,
}

impl<'a> FontQuery<'a> {
    /// Create a new font query for the given family.
    pub fn new(family: &'a str) -> Self {
        Self {
            family,
            fallbacks: &[],
            weight: FontWeight::Regular,
            style: FontStyle::Normal,
        }
    }

    /// Set fallback families.
    pub fn with_fallbacks(mut self, fallbacks: &'a [&'a str]) -> Self {
        self.fallbacks = fallbacks;
        self
    }

    /// Set the desired weight.
    pub fn with_weight(mut self, weight: FontWeight) -> Self {
        self.weight = weight;
        self
    }

    /// Set the desired style.
    pub fn with_style(mut self, style: FontStyle) -> Self {
        self.style = style;
        self
    }
}

/// A trait for loading and discovering fonts.
///
/// This abstraction allows the layout engine to work with fonts from:
/// - System font directories
/// - In-memory font storage
/// - Embedded font resources
/// - Remote font services
///
/// # Implementations
///
/// - `SystemFontProvider`: Loads from system font directories (feature-gated)
/// - `InMemoryFontProvider`: Uses pre-loaded font data (always available)
/// - `EmbeddedFontProvider`: Uses bundled font resources (feature-gated)
///
/// # Example
///
/// ```ignore
/// let provider: Box<dyn FontProvider> = Box::new(InMemoryFontProvider::new());
/// provider.add_font("Helvetica", FontWeight::Regular, FontStyle::Normal, font_bytes).unwrap();
/// let data = provider.load_font(&FontQuery::new("Helvetica"))?;
/// ```
pub trait FontProvider: Send + Sync + Debug {
    /// Load a font matching the given query.
    ///
    /// The provider should attempt to find the best match for the query,
    /// trying fallback families if the primary family is not found.
    ///
    /// # Arguments
    ///
    /// * `query` - The font query specifying family, weight, and style
    ///
    /// # Returns
    ///
    /// The font data as shared bytes, or an error if no matching font is found.
    fn load_font(&self, query: &FontQuery<'_>) -> Result<SharedFontData, FontError>;

    /// Check if a font matching the query is available.
    ///
    /// # Arguments
    ///
    /// * `query` - The font query to check
    ///
    /// # Returns
    ///
    /// `true` if a matching font can be loaded.
    fn has_font(&self, query: &FontQuery<'_>) -> bool;

    /// List all available font families.
    ///
    /// # Returns
    ///
    /// A list of unique font family names available in this provider.
    fn list_families(&self) -> Vec<String>;

    /// List all available font faces with full descriptors.
    ///
    /// # Returns
    ///
    /// A list of font descriptors for all available fonts.
    fn list_fonts(&self) -> Vec<FontDescriptor>;

    /// Returns a human-readable name for this provider (for logging/debugging).
    fn name(&self) -> &'static str;
}

/// An in-memory font provider.
///
/// Fonts are stored in memory and must be pre-populated before use.
/// This is the simplest provider and works in any environment including WASM.
#[derive(Debug, Default)]
pub struct InMemoryFontProvider {
    fonts: std::sync::RwLock<Vec<(FontDescriptor, SharedFontData)>>,
}

impl InMemoryFontProvider {
    pub fn new() -> Self {
        Self {
            fonts: std::sync::RwLock::new(Vec::new()),
        }
    }

    /// Add a font to the in-memory store.
    ///
    /// # Arguments
    ///
    /// * `family` - The font family name
    /// * `weight` - The font weight
    /// * `style` - The font style
    /// * `data` - The font file data (TTF/OTF bytes)
    ///
    /// # Errors
    ///
    /// Returns `FontError::LoadFailed` if the internal lock is poisoned.
    pub fn add_font(
        &self,
        family: impl Into<String>,
        weight: FontWeight,
        style: FontStyle,
        data: Vec<u8>,
    ) -> Result<(), FontError> {
        let family_string = family.into();
        let descriptor = FontDescriptor {
            family: family_string.clone(),
            weight: weight.clone(),
            style: style.clone(),
            postscript_name: None,
        };
        let mut fonts = self.fonts.write()
            .map_err(|_| FontError::LoadFailed {
                path: format!("{}:{}:{:?}", family_string, weight.numeric_value(), style),
                message: "font store lock poisoned".to_string(),
            })?;
        fonts.push((descriptor, Arc::new(data)));
        Ok(())
    }

    /// Add a font with shared data.
    ///
    /// # Errors
    ///
    /// Returns `FontError::LoadFailed` if the internal lock is poisoned.
    pub fn add_font_shared(
        &self,
        family: impl Into<String>,
        weight: FontWeight,
        style: FontStyle,
        data: SharedFontData,
    ) -> Result<(), FontError> {
        let family_string = family.into();
        let descriptor = FontDescriptor {
            family: family_string.clone(),
            weight: weight.clone(),
            style: style.clone(),
            postscript_name: None,
        };
        let mut fonts = self.fonts.write()
            .map_err(|_| FontError::LoadFailed {
                path: format!("{}:{}:{:?}", family_string, weight.numeric_value(), style),
                message: "font store lock poisoned".to_string(),
            })?;
        fonts.push((descriptor, data));
        Ok(())
    }

    /// Add a font with a full descriptor.
    ///
    /// # Errors
    ///
    /// Returns `FontError::LoadFailed` if the internal lock is poisoned.
    pub fn add_font_with_descriptor(&self, descriptor: FontDescriptor, data: Vec<u8>) -> Result<(), FontError> {
        let path = format!("{}:{}:{:?}", descriptor.family, descriptor.weight.numeric_value(), descriptor.style);
        let mut fonts = self.fonts.write()
            .map_err(|_| FontError::LoadFailed {
                path,
                message: "font store lock poisoned".to_string(),
            })?;
        fonts.push((descriptor, Arc::new(data)));
        Ok(())
    }

    /// Get the number of fonts in the store.
    ///
    /// Returns 0 if the lock is poisoned.
    pub fn len(&self) -> usize {
        self.fonts.read().map(|f| f.len()).unwrap_or(0)
    }

    /// Check if the store is empty.
    ///
    /// Returns `true` if the lock is poisoned (safe default).
    pub fn is_empty(&self) -> bool {
        self.fonts.read().map(|f| f.is_empty()).unwrap_or(true)
    }

    /// Clear all fonts from the store.
    ///
    /// Does nothing if the lock is poisoned.
    pub fn clear(&self) {
        if let Ok(mut fonts) = self.fonts.write() {
            fonts.clear();
        }
    }

    /// Find the best matching font for a query.
    fn find_match(&self, query: &FontQuery<'_>) -> Option<SharedFontData> {
        let fonts = self.fonts.read().ok()?;

        // Try exact match on primary family
        if let Some(data) = self.find_in_family(&fonts, query.family, &query.weight, &query.style) {
            return Some(data);
        }

        // Try fallback families
        for fallback in query.fallbacks {
            if let Some(data) = self.find_in_family(&fonts, fallback, &query.weight, &query.style) {
                return Some(data);
            }
        }

        // Try any font in primary family with any weight/style
        if let Some((_, data)) = fonts.iter().find(|(d, _)| d.family.eq_ignore_ascii_case(query.family)) {
            return Some(data.clone());
        }

        // Try any font in fallback families
        for fallback in query.fallbacks {
            if let Some((_, data)) = fonts.iter().find(|(d, _)| d.family.eq_ignore_ascii_case(fallback)) {
                return Some(data.clone());
            }
        }

        None
    }

    fn find_in_family(
        &self,
        fonts: &[(FontDescriptor, SharedFontData)],
        family: &str,
        weight: &FontWeight,
        style: &FontStyle,
    ) -> Option<SharedFontData> {
        // Exact match
        for (descriptor, data) in fonts {
            if descriptor.family.eq_ignore_ascii_case(family)
                && &descriptor.weight == weight
                && &descriptor.style == style
            {
                return Some(data.clone());
            }
        }

        // Match family and style, closest weight
        let family_matches: Vec<_> = fonts
            .iter()
            .filter(|(d, _)| d.family.eq_ignore_ascii_case(family) && &d.style == style)
            .collect();

        if !family_matches.is_empty() {
            // Find closest weight
            let target_weight = weight.numeric_value();
            let closest = family_matches
                .iter()
                .min_by_key(|(d, _)| (d.weight.numeric_value() as i32 - target_weight as i32).abs());
            if let Some((_, data)) = closest {
                return Some(data.clone());
            }
        }

        None
    }
}

impl FontProvider for InMemoryFontProvider {
    fn load_font(&self, query: &FontQuery<'_>) -> Result<SharedFontData, FontError> {
        self.find_match(query).ok_or_else(|| FontError::NotFound {
            family: query.family.to_string(),
            weight: query.weight.clone(),
            style: query.style.clone(),
        })
    }

    fn has_font(&self, query: &FontQuery<'_>) -> bool {
        self.find_match(query).is_some()
    }

    fn list_families(&self) -> Vec<String> {
        let fonts = match self.fonts.read() {
            Ok(f) => f,
            Err(_) => return Vec::new(),
        };
        let mut families: Vec<_> = fonts.iter().map(|(d, _)| d.family.clone()).collect();
        families.sort();
        families.dedup();
        families
    }

    fn list_fonts(&self) -> Vec<FontDescriptor> {
        self.fonts.read()
            .map(|f| f.iter().map(|(d, _)| d.clone()).collect())
            .unwrap_or_default()
    }

    fn name(&self) -> &'static str {
        "InMemoryFontProvider"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_fake_font_data(name: &str) -> Vec<u8> {
        // Just use the name as fake font data for testing
        name.as_bytes().to_vec()
    }

    #[test]
    fn test_in_memory_provider_add_and_load() {
        let provider = InMemoryFontProvider::new();
        provider.add_font("TestFont", FontWeight::Regular, FontStyle::Normal, make_fake_font_data("test")).unwrap();

        let query = FontQuery::new("TestFont");
        let data = provider.load_font(&query).unwrap();
        assert_eq!(&*data, b"test");
    }

    #[test]
    fn test_in_memory_provider_not_found() {
        let provider = InMemoryFontProvider::new();
        let query = FontQuery::new("NonexistentFont");
        let result = provider.load_font(&query);
        assert!(matches!(result, Err(FontError::NotFound { .. })));
    }

    #[test]
    fn test_in_memory_provider_has_font() {
        let provider = InMemoryFontProvider::new();
        provider.add_font("TestFont", FontWeight::Regular, FontStyle::Normal, make_fake_font_data("test")).unwrap();

        assert!(provider.has_font(&FontQuery::new("TestFont")));
        assert!(!provider.has_font(&FontQuery::new("OtherFont")));
    }

    #[test]
    fn test_in_memory_provider_list_families() {
        let provider = InMemoryFontProvider::new();
        provider.add_font("Arial", FontWeight::Regular, FontStyle::Normal, make_fake_font_data("arial")).unwrap();
        provider.add_font("Arial", FontWeight::Bold, FontStyle::Normal, make_fake_font_data("arial-bold")).unwrap();
        provider.add_font("Helvetica", FontWeight::Regular, FontStyle::Normal, make_fake_font_data("helvetica")).unwrap();

        let families = provider.list_families();
        assert_eq!(families.len(), 2);
        assert!(families.contains(&"Arial".to_string()));
        assert!(families.contains(&"Helvetica".to_string()));
    }

    #[test]
    fn test_in_memory_provider_fallback() {
        let provider = InMemoryFontProvider::new();
        provider.add_font("Fallback", FontWeight::Regular, FontStyle::Normal, make_fake_font_data("fallback")).unwrap();

        let query = FontQuery::new("Primary").with_fallbacks(&["Fallback"]);
        let data = provider.load_font(&query).unwrap();
        assert_eq!(&*data, b"fallback");
    }

    #[test]
    fn test_in_memory_provider_weight_matching() {
        let provider = InMemoryFontProvider::new();
        provider.add_font("TestFont", FontWeight::Regular, FontStyle::Normal, make_fake_font_data("regular")).unwrap();
        provider.add_font("TestFont", FontWeight::Bold, FontStyle::Normal, make_fake_font_data("bold")).unwrap();

        // Exact match
        let query = FontQuery::new("TestFont").with_weight(FontWeight::Bold);
        let data = provider.load_font(&query).unwrap();
        assert_eq!(&*data, b"bold");

        // Closest match (Medium should match Regular which is closer than Bold)
        let query = FontQuery::new("TestFont").with_weight(FontWeight::Medium);
        let data = provider.load_font(&query).unwrap();
        // Medium (500) is closer to Regular (400) than Bold (700)
        assert_eq!(&*data, b"regular");
    }

    #[test]
    fn test_in_memory_provider_case_insensitive() {
        let provider = InMemoryFontProvider::new();
        provider.add_font("TestFont", FontWeight::Regular, FontStyle::Normal, make_fake_font_data("test")).unwrap();

        let query = FontQuery::new("testfont");
        assert!(provider.has_font(&query));

        let query = FontQuery::new("TESTFONT");
        assert!(provider.has_font(&query));
    }

    // Edge case tests

    #[test]
    fn test_in_memory_provider_empty() {
        let provider = InMemoryFontProvider::new();
        assert!(provider.is_empty());
        assert_eq!(provider.len(), 0);
        assert!(provider.list_families().is_empty());
        assert!(provider.list_fonts().is_empty());
    }

    #[test]
    fn test_in_memory_provider_clear() {
        let provider = InMemoryFontProvider::new();
        provider.add_font("Font1", FontWeight::Regular, FontStyle::Normal, make_fake_font_data("a")).unwrap();
        provider.add_font("Font2", FontWeight::Regular, FontStyle::Normal, make_fake_font_data("b")).unwrap();

        assert_eq!(provider.len(), 2);
        provider.clear();
        assert!(provider.is_empty());
        assert_eq!(provider.len(), 0);
    }

    #[test]
    fn test_in_memory_provider_style_matching() {
        let provider = InMemoryFontProvider::new();
        provider.add_font("TestFont", FontWeight::Regular, FontStyle::Normal, make_fake_font_data("normal")).unwrap();
        provider.add_font("TestFont", FontWeight::Regular, FontStyle::Italic, make_fake_font_data("italic")).unwrap();

        let query = FontQuery::new("TestFont").with_style(FontStyle::Italic);
        let data = provider.load_font(&query).unwrap();
        assert_eq!(&*data, b"italic");

        let query = FontQuery::new("TestFont").with_style(FontStyle::Normal);
        let data = provider.load_font(&query).unwrap();
        assert_eq!(&*data, b"normal");
    }

    #[test]
    fn test_in_memory_provider_add_with_descriptor() {
        let provider = InMemoryFontProvider::new();
        let descriptor = FontDescriptor {
            family: "CustomFont".to_string(),
            weight: FontWeight::Bold,
            style: FontStyle::Italic,
            postscript_name: Some("CustomFont-BoldItalic".to_string()),
        };
        provider.add_font_with_descriptor(descriptor, make_fake_font_data("custom")).unwrap();

        let query = FontQuery::new("CustomFont")
            .with_weight(FontWeight::Bold)
            .with_style(FontStyle::Italic);
        let data = provider.load_font(&query).unwrap();
        assert_eq!(&*data, b"custom");
    }

    #[test]
    fn test_in_memory_provider_multiple_fallbacks() {
        let provider = InMemoryFontProvider::new();
        provider.add_font("ThirdChoice", FontWeight::Regular, FontStyle::Normal, make_fake_font_data("third")).unwrap();

        // First and second fallbacks don't exist
        let query = FontQuery::new("Primary")
            .with_fallbacks(&["Second", "ThirdChoice"]);
        let data = provider.load_font(&query).unwrap();
        assert_eq!(&*data, b"third");
    }

    #[test]
    fn test_in_memory_provider_no_matching_fallback() {
        let provider = InMemoryFontProvider::new();
        provider.add_font("SomeFont", FontWeight::Regular, FontStyle::Normal, make_fake_font_data("some")).unwrap();

        let query = FontQuery::new("Primary")
            .with_fallbacks(&["Fallback1", "Fallback2"]);
        let result = provider.load_font(&query);
        assert!(matches!(result, Err(FontError::NotFound { .. })));
    }

    #[test]
    fn test_in_memory_provider_name() {
        let provider = InMemoryFontProvider::new();
        assert_eq!(provider.name(), "InMemoryFontProvider");
    }

    #[test]
    fn test_font_query_builder() {
        let query = FontQuery::new("Arial")
            .with_weight(FontWeight::Bold)
            .with_style(FontStyle::Italic)
            .with_fallbacks(&["Helvetica", "sans-serif"]);

        assert_eq!(query.family, "Arial");
        assert_eq!(query.weight, FontWeight::Bold);
        assert_eq!(query.style, FontStyle::Italic);
        assert_eq!(query.fallbacks, &["Helvetica", "sans-serif"]);
    }

    #[test]
    fn test_font_error_display() {
        let err = FontError::NotFound {
            family: "Arial".to_string(),
            weight: FontWeight::Bold,
            style: FontStyle::Normal,
        };
        let msg = err.to_string();
        assert!(msg.contains("Arial"));
        assert!(msg.contains("Bold"));
    }
}
