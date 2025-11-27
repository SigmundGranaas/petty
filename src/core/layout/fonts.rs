// src/core/layout/fonts.rs

use cosmic_text::FontSystem;
use std::path::Path;
use std::sync::{Arc, Mutex};

/// Manages the font system.
#[derive(Clone)]
pub struct FontManager {
    pub system: Arc<Mutex<FontSystem>>,
}

impl FontManager {
    pub fn new() -> Self {
        let system = FontSystem::new();
        Self {
            system: Arc::new(Mutex::new(system)),
        }
    }

    pub fn load_fallback_font(&self) {
        let mut system = self.system.lock().unwrap();
        let db = system.db_mut();
        // Assuming assets are located relative to crate root
        if let Ok(regular) = std::fs::read("assets/fonts/Helvetica.ttf") {
            db.load_font_data(regular);
        }
        if let Ok(bold) = std::fs::read("assets/fonts/helvetica-bold.ttf") {
            db.load_font_data(bold);
        }
    }

    pub fn load_system_fonts(&self) {
        self.system.lock().unwrap().db_mut().load_system_fonts();
    }

    pub fn load_fonts_from_dir<P: AsRef<Path>>(&self, path: P) {
        self.system.lock().unwrap().db_mut().load_fonts_dir(path);
    }

    pub fn attrs_from_style<'a>(
        &self,
        style: &'a crate::core::layout::ComputedStyle,
    ) -> cosmic_text::Attrs<'a> {
        use crate::core::style::font::{FontStyle, FontWeight};
        use cosmic_text::{Attrs, Family, Style, Weight};

        let weight = match style.text.font_weight {
            FontWeight::Thin => Weight::THIN,
            FontWeight::Light => Weight::LIGHT,
            FontWeight::Regular => Weight::NORMAL,
            FontWeight::Medium => Weight::MEDIUM,
            FontWeight::Bold => Weight::BOLD,
            FontWeight::Black => Weight::BLACK,
            FontWeight::Numeric(w) => Weight(w),
        };

        let font_style = match style.text.font_style {
            FontStyle::Normal => Style::Normal,
            FontStyle::Italic => Style::Italic,
            FontStyle::Oblique => Style::Oblique,
        };

        let family = Family::Name(&style.text.font_family);

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