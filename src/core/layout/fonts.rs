use crate::core::layout::ComputedStyle;
use crate::core::style::font::{FontStyle, FontWeight};
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, RwLock};

/// A thread-safe handle to font data.
pub struct FontInstance {
    pub data: Arc<Vec<u8>>,
}

impl std::fmt::Debug for FontInstance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FontInstance")
            .field("data_len", &self.data.len())
            .finish()
    }
}

impl FontInstance {
    pub fn new(data: Arc<Vec<u8>>) -> Self {
        Self { data }
    }

    /// Creates a lightweight Face view over the font data.
    /// This is cheap (parsing header) and avoids self-referential struct issues.
    pub fn as_face(&self) -> Option<rustybuzz::Face<'_>> {
        rustybuzz::Face::from_slice(&self.data, 0)
    }
}

pub type FontData = Arc<FontInstance>;

/// Holds configuration and raw data to initialize font systems.
#[derive(Clone)]
pub struct SharedFontLibrary {
    pub db: Arc<RwLock<fontdb::Database>>,
    /// Cache of loaded font binaries, keyed by fontdb::ID.
    font_data_cache: Arc<RwLock<HashMap<fontdb::ID, FontData>>>,
}

impl SharedFontLibrary {
    pub fn new() -> Self {
        Self {
            db: Arc::new(RwLock::new(fontdb::Database::new())),
            font_data_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn with_system_fonts(self, enable: bool) -> Self {
        if enable {
            // gracefully ignore lock poisoning
            if let Ok(mut db) = self.db.write() {
                db.load_system_fonts();
            }
        }
        self
    }

    pub fn add_fallback_font(&self, data: Vec<u8>) {
        if let Ok(mut db) = self.db.write() {
            db.load_font_data(data);
        }
    }

    pub fn add_font_dir<P: AsRef<Path>>(&self, path: P) {
        if let Ok(mut db) = self.db.write() {
            db.load_fonts_dir(path);
        }
    }

    pub fn load_fallback_font(&self) {
        if let Ok(regular) = std::fs::read("assets/fonts/Helvetica.ttf") {
            self.add_fallback_font(regular);
        }
        if let Ok(bold) = std::fs::read("assets/fonts/helvetica-bold.ttf") {
            self.add_fallback_font(bold);
        }
    }

    /// Resolves the raw font data for a given style.
    pub fn resolve_font_data(&self, style: &ComputedStyle) -> Option<FontData> {
        let family = style.text.font_family.as_str();
        let weight = map_weight(style.text.font_weight.clone());
        let font_style = map_style(style.text.font_style.clone());

        let query = fontdb::Query {
            families: &[fontdb::Family::Name(family), fontdb::Family::SansSerif],
            weight,
            stretch: fontdb::Stretch::Normal,
            style: font_style,
        };

        let id = {
            let db = self.db.read().ok()?;
            db.query(&query).or_else(|| {
                db.query(&fontdb::Query {
                    families: &[fontdb::Family::SansSerif],
                    weight,
                    stretch: fontdb::Stretch::Normal,
                    style: font_style,
                })
            })?
        };

        // Fast path: check data cache
        {
            if let Ok(cache) = self.font_data_cache.read() {
                if let Some(data) = cache.get(&id) {
                    return Some(data.clone());
                }
            }
        }

        // Slow path: load data
        // We need to re-acquire the DB lock. If it's poisoned now, we abort.
        let db = self.db.read().ok()?;

        if let Some(face_info) = db.face(id) {
            match &face_info.source {
                fontdb::Source::Binary(data) => {
                    let vec_data = data.as_ref().as_ref().to_vec();
                    let data_arc = Arc::new(vec_data);
                    let instance = Arc::new(FontInstance::new(data_arc));

                    if let Ok(mut cache) = self.font_data_cache.write() {
                        cache.insert(id, instance.clone());
                    }
                    Some(instance)
                }
                fontdb::Source::File(path) => {
                    if let Ok(data) = std::fs::read(path) {
                        let data_arc = Arc::new(data);
                        let instance = Arc::new(FontInstance::new(data_arc));

                        if let Ok(mut cache) = self.font_data_cache.write() {
                            cache.insert(id, instance.clone());
                        }
                        Some(instance)
                    } else {
                        None
                    }
                }
                _ => None,
            }
        } else {
            None
        }
    }
}

impl Default for SharedFontLibrary {
    fn default() -> Self {
        let lib = Self::new();
        lib.load_fallback_font();
        lib
    }
}

pub struct LocalFontContext {}

impl LocalFontContext {
    pub fn new(_library: &SharedFontLibrary) -> Self {
        LocalFontContext {}
    }
}

fn map_weight(w: FontWeight) -> fontdb::Weight {
    match w {
        FontWeight::Thin => fontdb::Weight::THIN,
        FontWeight::Light => fontdb::Weight::LIGHT,
        FontWeight::Regular => fontdb::Weight::NORMAL,
        FontWeight::Medium => fontdb::Weight::MEDIUM,
        FontWeight::Bold => fontdb::Weight::BOLD,
        FontWeight::Black => fontdb::Weight::BLACK,
        FontWeight::Numeric(n) => fontdb::Weight(n),
    }
}

fn map_style(s: FontStyle) -> fontdb::Style {
    match s {
        FontStyle::Normal => fontdb::Style::Normal,
        FontStyle::Italic => fontdb::Style::Italic,
        FontStyle::Oblique => fontdb::Style::Oblique,
    }
}