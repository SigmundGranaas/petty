use cosmic_text::{FontSystem};
use std::path::Path;
use std::sync::{Arc, Mutex};

/// Manages the font system (loading and caching) via `cosmic-text`.
#[derive(Clone)]
pub struct FontManager {
    // FontSystem is not thread-safe, so we wrap it in a Mutex.
    pub system: Arc<Mutex<FontSystem>>,
}

impl FontManager {
    pub fn new() -> Self {
        let system = FontSystem::new();
        Self {
            system: Arc::new(Mutex::new(system)),
        }
    }

    /// Loads the built-in fallback font (e.g. Helvetica) from memory.
    pub fn load_fallback_font(&self) {
        let mut system = self.system.lock().unwrap();
        let db = system.db_mut();

        // Load embedded fonts as a reliable reliable fallback
        let regular = include_bytes!("../../../assets/fonts/Helvetica.ttf").to_vec();
        let bold = include_bytes!("../../../assets/fonts/helvetica-bold.ttf").to_vec();

        db.load_font_data(regular);
        db.load_font_data(bold);
    }

    pub fn load_system_fonts(&self) {
        self.system.lock().unwrap().db_mut().load_system_fonts();
    }

    pub fn load_fonts_from_dir<P: AsRef<Path>>(&self, path: P) {
        self.system.lock().unwrap().db_mut().load_fonts_dir(path);
    }

    /// Helper to map a computed style to cosmic-text attributes.
    pub fn attrs_from_style<'a>(&self, style: &'a crate::core::layout::ComputedStyle) -> cosmic_text::Attrs<'a> {
        use crate::core::style::font::{FontStyle, FontWeight};
        use cosmic_text::{Attrs, Family, Style, Weight};

        let weight = match style.font_weight {
            FontWeight::Thin => Weight::THIN,
            FontWeight::Light => Weight::LIGHT,
            FontWeight::Regular => Weight::NORMAL,
            FontWeight::Medium => Weight::MEDIUM,
            FontWeight::Bold => Weight::BOLD,
            FontWeight::Black => Weight::BLACK,
            FontWeight::Numeric(w) => Weight(w),
        };

        let font_style = match style.font_style {
            FontStyle::Normal => Style::Normal,
            FontStyle::Italic => Style::Italic,
            FontStyle::Oblique => Style::Oblique,
        };

        // Map "Helvetica" to SansSerif to ensure fallback works if exact match fails.
        // This is important for tests running in environments without system fonts.
        let family = if style.font_family.eq_ignore_ascii_case("Helvetica") {
            Family::SansSerif
        } else {
            Family::Name(&style.font_family)
        };

        Attrs::new()
            .family(family)
            .weight(weight)
            .style(font_style)
    }
}

impl Default for FontManager {
    fn default() -> Self {
        Self::new()
    }
}