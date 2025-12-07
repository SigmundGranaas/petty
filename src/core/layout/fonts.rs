// src/core/layout/fonts.rs
use cosmic_text::{Attrs, Family, FontSystem, Style, Weight};
use std::path::PathBuf;
use std::time::Instant;

/// Holds configuration and raw data to initialize font systems.
/// This structure is thread-safe and designed to be shared (Arc) across threads.
/// It does not hold the `FontSystem` itself, which is not thread-safe.
pub struct SharedFontLibrary {
    load_system_fonts: bool,
    fallback_fonts: Vec<Vec<u8>>,
    custom_font_dirs: Vec<PathBuf>,
}

impl SharedFontLibrary {
    pub fn new() -> Self {
        Self {
            load_system_fonts: true,
            fallback_fonts: Vec::new(),
            custom_font_dirs: Vec::new(),
        }
    }

    pub fn with_system_fonts(mut self, enable: bool) -> Self {
        self.load_system_fonts = enable;
        self
    }

    pub fn add_fallback_font(&mut self, data: Vec<u8>) {
        self.fallback_fonts.push(data);
    }

    pub fn add_font_dir<P: Into<PathBuf>>(&mut self, path: P) {
        self.custom_font_dirs.push(path.into());
    }

    /// Loads the default hardcoded fallback fonts (Helvetica) if available.
    pub fn load_fallback_font(&mut self) {
        // Assuming assets are located relative to execution dir
        if let Ok(regular) = std::fs::read("assets/fonts/Helvetica.ttf") {
            self.add_fallback_font(regular);
        }
        if let Ok(bold) = std::fs::read("assets/fonts/helvetica-bold.ttf") {
            self.add_fallback_font(bold);
        }
    }
}

impl Default for SharedFontLibrary {
    fn default() -> Self {
        let mut lib = Self::new();
        lib.load_fallback_font();
        lib
    }
}

/// A thread-local container for the FontSystem.
/// Cosmic-text's FontSystem is not thread-safe, so we create one per worker
/// using the configuration from `SharedFontLibrary`.
pub struct LocalFontContext {
    pub system: FontSystem,
}

impl LocalFontContext {
    pub fn new(library: &SharedFontLibrary) -> Self {
        let start = Instant::now();
        let mut system = FontSystem::new();
        let db = system.db_mut();

        if library.load_system_fonts {
            db.load_system_fonts();
        }

        for path in &library.custom_font_dirs {
            db.load_fonts_dir(path);
        }

        // Load specific fallbacks
        for data in &library.fallback_fonts {
            db.load_font_data(data.clone());
        }

        let duration = start.elapsed();
        // Log initialization as it happens once per thread
        if duration.as_millis() > 10 {
            log::debug!(
                "[PERF] LocalFontContext::new (font loading) took {:?}",
                duration
            );
        }

        Self { system }
    }
}

/// Helper to convert styles to cosmic_text Attrs
pub fn attrs_from_style<'a>(
    style: &'a crate::core::layout::ComputedStyle,
) -> Attrs<'a> {
    use crate::core::style::font::FontWeight;

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
        crate::core::style::font::FontStyle::Normal => Style::Normal,
        crate::core::style::font::FontStyle::Italic => Style::Italic,
        crate::core::style::font::FontStyle::Oblique => Style::Oblique,
    };

    let family = Family::Name(&style.text.font_family);

    Attrs::new()
        .family(family)
        .weight(weight)
        .style(font_style)
}