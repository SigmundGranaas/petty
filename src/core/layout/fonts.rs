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
        let font_bytes = include_bytes!("../../../assets/fonts/Helvetica.ttf").to_vec();
        let font_data = Arc::new(font_bytes);
        let font_map = Arc::make_mut(&mut self.font_data);
        font_map.insert("Helvetica".to_string(), font_data);
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

    /// Measures the width of a string of text using the specified font and size.
    pub fn measure_text(&self, text: &str, family_name: &str, size: f32) -> f32 {
        let font = match self.get_font(family_name) {
            Some(f) => f,
            None => {
                // Fallback to Helvetica if the specified font is not found.
                if family_name != "Helvetica" {
                    log::warn!("Font '{}' not found, falling back to Helvetica.", family_name);
                    self.get_font("Helvetica").unwrap()
                } else {
                    // This should not happen if the fallback is always loaded.
                    panic!("Fallback font Helvetica is missing!");
                }
            }
        };

        let mut total_width = 0.0;
        for character in text.chars() {
            let metrics = font.metrics(character, size);
            total_width += metrics.advance_width;
        }
        total_width
    }
}