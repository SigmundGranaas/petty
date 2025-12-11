// src/core/layout/fonts.rs
use crate::core::layout::ComputedStyle;
use crate::core::style::font::{FontStyle, FontWeight};
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, RwLock};

// FIX: FontInstance holds both the data and the parsed Face to avoid re-parsing overhead.
pub struct FontInstance {
    pub data: Arc<Vec<u8>>,
    // We use 'static lifetime internally via unsafe, but expose it safely tied to &self.
    face: rustybuzz::Face<'static>,
}

// FIX: Implement Debug manually because rustybuzz::Face might not implement it or we want a cleaner output.
impl std::fmt::Debug for FontInstance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FontInstance")
            .field("data_len", &self.data.len())
            .finish()
    }
}

// SAFETY: rustybuzz::Face is Send/Sync (holds raw pointers/slices).
// The data backing the face is owned by the Arc in the same struct.
// Since the Arc ensures the data is stable in memory, it is safe to send this struct across threads.
unsafe impl Send for FontInstance {}
unsafe impl Sync for FontInstance {}

impl FontInstance {
    pub fn new(data: Arc<Vec<u8>>) -> Option<Self> {
        // SAFETY: We extend the lifetime of the slice to 'static.
        // This is safe because the slice comes from the Arc<Vec<u8>> which is owned by Self.
        // As long as Self exists, the Arc exists, and the data pointer is valid and pinned in memory.
        let ptr = data.as_ptr();
        let len = data.len();
        let slice = unsafe { std::slice::from_raw_parts(ptr, len) };

        let face = rustybuzz::Face::from_slice(slice, 0)?;

        Some(Self {
            data,
            face,
        })
    }

    pub fn face(&self) -> &rustybuzz::Face<'_> {
        &self.face
    }
}

// FontData is now an Arc to the cached instance
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
            self.db.write().unwrap().load_system_fonts();
        }
        self
    }

    pub fn add_fallback_font(&self, data: Vec<u8>) {
        let mut db = self.db.write().unwrap();
        db.load_font_data(data);
    }

    pub fn add_font_dir<P: AsRef<Path>>(&self, path: P) {
        let mut db = self.db.write().unwrap();
        db.load_fonts_dir(path);
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
        // This is the SLOW PATH (Locking)
        // Clone properties to avoid move errors
        let family = style.text.font_family.as_str();
        let weight = map_weight(style.text.font_weight.clone());
        let font_style = map_style(style.text.font_style.clone());

        let query = fontdb::Query {
            families: &[fontdb::Family::Name(family), fontdb::Family::SansSerif],
            weight,
            stretch: fontdb::Stretch::Normal,
            style: font_style,
        };

        // CONTENTION POINT: Acquiring read lock on shared DB
        let db = self.db.read().unwrap();
        let id = db.query(&query).or_else(|| {
            db.query(&fontdb::Query {
                families: &[fontdb::Family::SansSerif],
                weight,
                stretch: fontdb::Stretch::Normal,
                style: font_style,
            })
        })?;

        // Fast path: check data cache (Shared lock)
        {
            let cache = self.font_data_cache.read().unwrap();
            if let Some(data) = cache.get(&id) {
                return Some(data.clone());
            }
        }

        // Slow path: load data from file/source (Write lock)
        if let Some(face_info) = db.face(id) {
            match &face_info.source {
                fontdb::Source::Binary(data) => {
                    let vec_data = data.as_ref().as_ref().to_vec();
                    let data_arc = Arc::new(vec_data);
                    // FIX: Create FontInstance to cache the parsed face
                    let instance = FontInstance::new(data_arc)?;
                    let instance_arc = Arc::new(instance);
                    self.font_data_cache.write().unwrap().insert(id, instance_arc.clone());
                    Some(instance_arc)
                }
                fontdb::Source::File(path) => {
                    if let Ok(data) = std::fs::read(path) {
                        let data_arc = Arc::new(data);
                        // FIX: Create FontInstance to cache the parsed face
                        let instance = FontInstance::new(data_arc)?;
                        let instance_arc = Arc::new(instance);
                        self.font_data_cache.write().unwrap().insert(id, instance_arc.clone());
                        Some(instance_arc)
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

/// A lightweight font context for local thread use.
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