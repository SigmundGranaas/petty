use crate::core::layout::style::ComputedStyle;
use crate::core::style::font::{FontStyle, FontWeight};
use fontdb::{Database, ID, Query, Style, Weight, Family};
use fontdue::{Font, FontSettings};
use lru::LruCache;
use std::num::NonZeroUsize;
use std::path::Path;
use std::sync::{Arc, Mutex};

const FONT_CACHE_SIZE: usize = 20;

/// Manages loading, querying, and caching fonts from various sources using a font database.
#[derive(Clone)]
pub struct FontManager {
    db: Arc<Database>,
    // Cache for parsed font objects, keyed by their ID in the database.
    font_cache: Arc<Mutex<LruCache<ID, Arc<Font>>>>,
}

impl FontManager {
    /// Creates a new, empty `FontManager`.
    pub fn new() -> Self {
        Self {
            db: Arc::new(Database::new()),
            font_cache: Arc::new(Mutex::new(LruCache::new(
                NonZeroUsize::new(FONT_CACHE_SIZE).unwrap(),
            ))),
        }
    }

    /// Provides access to the underlying font database for renderers.
    pub fn db(&self) -> &Database {
        &self.db
    }

    /// Loads the built-in Helvetica fonts and sets it as the ultimate fallback.
    pub fn load_fallback_font(&mut self) {
        let db: &mut Database = Arc::make_mut(&mut self.db);
        // Regular
        db.load_font_data(include_bytes!("../../../assets/fonts/Helvetica.ttf").to_vec());
        // Bold
        db.load_font_data(include_bytes!("../../../assets/fonts/helvetica-bold.ttf").to_vec());
    }

    /// Scans a directory for fonts and adds them to the database.
    pub fn load_fonts_from_dir(&mut self, path: &Path) {
        log::info!("Scanning for fonts in '{}'...", path.display());
        let db: &mut Database = Arc::make_mut(&mut self.db);
        db.load_fonts_dir(path);
    }

    /// Loads system-installed fonts into the database.
    pub fn load_system_fonts(&mut self) {
        log::info!("Loading system fonts...");
        let db: &mut Database = Arc::make_mut(&mut self.db);
        db.load_system_fonts();
        log::info!("Finished loading system fonts. Total font faces: {}", db.len());
    }

    /// Gets a parsed `fontdue::Font` object by its database ID, using a cache.
    fn get_font(&self, id: ID) -> Option<Arc<Font>> {
        let mut cache = self.font_cache.lock().unwrap();
        if let Some(font) = cache.get(&id) {
            return Some(Arc::clone(font));
        }

        // Font not in cache, parse it from the database.
        let face = self.db.face(id)?;
        let face_index = face.index;
        let post_script_name = &face.post_script_name;

        let parsed_font = self.db.with_face_data(id, |data, _| {
            Font::from_bytes(
                data,
                FontSettings {
                    collection_index: face_index,
                    ..Default::default()
                },
            )
        });

        match parsed_font {
            Some(Ok(font)) => {
                let font_arc = Arc::new(font);
                cache.put(id, Arc::clone(&font_arc));
                Some(font_arc)
            }
            Some(Err(e)) => {
                log::error!(
                    "Failed to parse font (ID: {:?}, PostScript: {}): {}",
                    id,
                    post_script_name,
                    e
                );
                None
            }
            None => {
                log::error!(
                    "Could not read font data for font (ID: {:?}, PostScript: {})",
                    id,
                    post_script_name
                );
                None
            }
        }
    }

    /// Finds the best font matching the style and measures text with it.
    pub fn measure_text(&self, text: &str, style: &Arc<ComputedStyle>) -> f32 {
        let query = Query {
            families: &[Family::Name(&style.font_family)],
            weight: map_font_weight(style.font_weight.clone()),
            style: map_font_style(style.font_style.clone()),
            ..Default::default()
        };

        // `db.query` performs the matching and fallback logic.
        let font_id = self.db.query(&query).unwrap_or_else(|| {
            log::warn!(
                "Font query failed for family '{}' (weight {:?}, style {:?}). Using default fallback.",
                style.font_family, style.font_weight, style.font_style
            );
            self.db
                .query(&Query {
                    families: &[Family::Name("Helvetica")],
                    ..Default::default()
                })
                .expect("Critical error: Default fallback font 'Helvetica' is missing!")
        });

        let font = self
            .get_font(font_id)
            .expect("Failed to load font from database ID.");

        let mut total_width = 0.0;
        for character in text.chars() {
            let metrics = font.metrics(character, style.font_size);
            total_width += metrics.advance_width;
        }
        total_width
    }
}

/// Maps our internal `FontWeight` enum to the `fontdb::Weight` enum.
fn map_font_weight(weight: FontWeight) -> Weight {
    match weight {
        FontWeight::Thin => Weight::THIN,
        FontWeight::Light => Weight::LIGHT,
        FontWeight::Regular => Weight::NORMAL,
        FontWeight::Medium => Weight::MEDIUM,
        FontWeight::Bold => Weight::BOLD,
        FontWeight::Black => Weight::BLACK,
        FontWeight::Numeric(val) => Weight(val),
    }
}

/// Maps our internal `FontStyle` enum to the `fontdb::Style` enum.
fn map_font_style(style: FontStyle) -> Style {
    match style {
        FontStyle::Normal => Style::Normal,
        FontStyle::Italic => Style::Italic,
        FontStyle::Oblique => Style::Oblique,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::layout::style::ComputedStyle;
    use crate::core::style::font::FontWeight;
    use std::sync::Arc;

    #[test]
    fn test_bold_font_selection_and_fallback() {
        let mut manager = FontManager::new();
        let helvetica_reg_data = include_bytes!("../../../assets/fonts/Helvetica.ttf").to_vec();
        let helvetica_bold_data =
            include_bytes!("../../../assets/fonts/helvetica-bold.ttf").to_vec();

        let db = Arc::make_mut(&mut manager.db);
        db.load_font_data(helvetica_reg_data);
        db.load_font_data(helvetica_bold_data);

        let text = "Bold Text";
        let mut regular_style = ComputedStyle::default();
        regular_style.font_family = Arc::from("Helvetica".to_string());

        let mut bold_style = regular_style.clone();
        bold_style.font_weight = FontWeight::Bold;

        let regular_width = manager.measure_text(text, &Arc::new(regular_style.clone()));
        let bold_width = manager.measure_text(text, &Arc::new(bold_style));
        assert!(
            bold_width > regular_width,
            "Bold text should be wider than regular text"
        );

        let mut missing_font_style = ComputedStyle::default();
        missing_font_style.font_family = "NonExistentFont123".to_string().into();

        let fallback_width = manager.measure_text(text, &Arc::new(missing_font_style));
        assert_eq!(
            fallback_width, regular_width,
            "Should fall back to regular Helvetica width"
        );
    }
}