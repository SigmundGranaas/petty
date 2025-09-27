use crate::core::layout::style::ComputedStyle;
use crate::core::style::font::FontWeight;
use crate::error::PipelineError;
use fontdue::{Font, FontSettings};
use lru::LruCache;
use std::collections::HashMap;
use std::fs;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

const CACHE_SIZE: usize = 10;

/// Manages loading, caching, and measuring text with system and user-provided fonts.
#[derive(Clone)]
pub struct FontManager {
    // Cache for parsed font objects.
    font_cache: Arc<Mutex<LruCache<String, Arc<Font>>>>,
    // A persistent map of family name to raw font data, for PDF embedding.
    pub font_data: Arc<HashMap<String, Arc<Vec<u8>>>>,
    // A map of font family names to their file paths.
    font_paths: Arc<HashMap<String, PathBuf>>,
}

impl FontManager {
    /// Creates a new, empty `FontManager`.
    pub fn new() -> Self {
        Self {
            font_cache: Arc::new(Mutex::new(LruCache::new(
                NonZeroUsize::new(CACHE_SIZE).unwrap(),
            ))),
            font_data: Arc::new(HashMap::new()),
            font_paths: Arc::new(HashMap::new()),
        }
    }

    /// Loads the built-in Helvetica font as a fallback.
    pub fn load_fallback_font(&mut self) -> Result<(), PipelineError> {
        // Regular
        let font_bytes = include_bytes!("../../../assets/fonts/Helvetica.ttf").to_vec();
        let font_data = Arc::new(font_bytes);
        let font_map = Arc::make_mut(&mut self.font_data);
        font_map.insert("Helvetica".to_string(), font_data);
        // Bold
        let font_bytes_bold = include_bytes!("../../../assets/fonts/helvetica-bold.ttf").to_vec();
        let font_data_bold = Arc::new(font_bytes_bold);
        font_map.insert("Helvetica-Bold".to_string(), font_data_bold);
        Ok(())
    }

    /// Scans a directory for `.ttf` files and registers them.
    pub fn load_fonts_from_dir(&mut self, path: &Path) -> Result<(), PipelineError> {
        log::info!("Scanning for fonts in '{}'...", path.display());
        let font_map = Arc::make_mut(&mut self.font_data);
        let path_map = Arc::make_mut(&mut self.font_paths);

        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let file_path = entry.path();
            if file_path.is_file() && file_path.extension().map_or(false, |e| e == "ttf") {
                // Convention: file stem is the font name (e.g., "Helvetica-Bold")
                if let Some(stem) = file_path.file_stem().and_then(|s| s.to_str()) {
                    let family_name = stem.to_string();
                    log::debug!("Registering font '{}' from {}", family_name, file_path.display());
                    let font_bytes = fs::read(&file_path)?;
                    font_map.insert(family_name.clone(), Arc::new(font_bytes));
                    path_map.insert(family_name, file_path);
                }
            }
        }
        Ok(())
    }

    /// Gets a parsed font by its family name, using the cache.
    fn get_font(&self, family_name: &str) -> Option<Arc<Font>> {
        let mut cache = self.font_cache.lock().unwrap();
        if let Some(font) = cache.get(family_name) {
            return Some(Arc::clone(font));
        }

        // Font not in cache, try to parse it from the raw data.
        if let Some(data) = self.font_data.get(family_name) {
            match Font::from_bytes(&data[..], FontSettings::default()) {
                Ok(font) => {
                    let font_arc = Arc::new(font);
                    cache.put(family_name.to_string(), Arc::clone(&font_arc));
                    return Some(font_arc);
                }
                Err(e) => {
                    log::error!("Failed to parse font '{}': {}", family_name, e);
                    return None;
                }
            }
        }
        None
    }

    /// Generates the specific font family name based on style (e.g., "Helvetica-Bold").
    fn get_styled_font_name(&self, style: &Arc<ComputedStyle>) -> String {
        let family = &style.font_family;
        // This logic can be expanded to handle italic, etc.
        match style.font_weight {
            FontWeight::Bold | FontWeight::Black => format!("{}-Bold", family),
            // Add more weights here if you have the font files
            // FontWeight::Light => format!("{}-Light", family),
            _ => family.to_string(),
        }
    }

    /// Measures the width of a string of text using the specified font and size.
    pub fn measure_text(&self, text: &str, style: &Arc<ComputedStyle>) -> f32 {
        let styled_font_name = self.get_styled_font_name(style);

        let font = match self.get_font(&styled_font_name) {
            Some(f) => f,
            None => {
                // The specific weight was not found, try the base family name.
                if &styled_font_name != style.font_family.as_str() {
                    log::warn!(
                        "Font style '{}' not found. Falling back to base font '{}'.",
                        styled_font_name,
                        style.font_family
                    );
                }
                // Fallback to the base font
                match self.get_font(&style.font_family) {
                    Some(f) => f,
                    None => {
                        // The base font was also not found, fall back to Helvetica.
                        log::warn!(
                            "Base font '{}' not found. Falling back to Helvetica.",
                            style.font_family
                        );
                        self.get_font("Helvetica").expect("Fallback font Helvetica is missing!")
                    }
                }
            }
        };

        let mut total_width = 0.0;
        for character in text.chars() {
            let metrics = font.metrics(character, style.font_size);
            total_width += metrics.advance_width;
        }
        total_width
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::layout::style::ComputedStyle;
    use crate::core::style::font::FontWeight;

    #[test]
    fn test_font_fallback() {
        let mut manager = FontManager::new();
        manager.load_fallback_font().unwrap();

        let text = "Hello World";
        let mut style = ComputedStyle::default();
        style.font_size = 12.0;

        // Measure with the actual fallback font
        let helvetica_width = manager.measure_text(text, &Arc::new(style.clone()));
        assert!(helvetica_width > 0.0);

        // Measure with a font that doesn't exist; it should fall back to Helvetica
        style.font_family = Arc::new("NonExistentFont123".to_string());
        let non_existent_font_width = manager.measure_text(text, &Arc::new(style));

        // The widths should be identical if the fallback worked correctly
        assert_eq!(helvetica_width, non_existent_font_width);
    }

    #[test]
    fn test_bold_font_selection_and_fallback() {
        let mut manager = FontManager::new();
        manager.load_fallback_font().unwrap(); // Loads Helvetica and Helvetica-Bold

        let text = "Bold Text";
        let mut regular_style = ComputedStyle::default();
        regular_style.font_family = Arc::new("Helvetica".to_string());

        let mut bold_style = regular_style.clone();
        bold_style.font_weight = FontWeight::Bold;

        let regular_width = manager.measure_text(text, &Arc::new(regular_style));
        let bold_width = manager.measure_text(text, &Arc::new(bold_style));

        assert!(bold_width > regular_width, "Bold text should be wider than regular text");

        // Now test fallback
        let mut missing_bold_style = ComputedStyle::default();
        missing_bold_style.font_family = Arc::new("Arial".to_string()); // We haven't loaded Arial
        missing_bold_style.font_weight = FontWeight::Bold;

        // This will warn "Arial-Bold not found", then "Arial not found", then use Helvetica
        let fallback_width = manager.measure_text(text, &Arc::new(missing_bold_style));
        assert_eq!(fallback_width, regular_width, "Should fall back to regular Helvetica width");
    }
}