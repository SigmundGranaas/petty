//! Font library abstraction for the layout engine.
//!
//! This module provides `SharedFontLibrary`, which manages font loading and caching
//! for the layout and rendering pipeline.
//!
//! ## Platform Abstraction
//!
//! The font library can operate in two modes:
//! - **System fonts mode** (feature: `system-fonts`): Uses fontdb for font discovery
//! - **Provider mode**: Uses injected `FontProvider` for custom font loading
//!
//! For WASM targets, only provider mode is available.

use crate::ComputedStyle;
use petty_style::font::{FontStyle, FontWeight};
use petty_traits::{FontProvider, FontQuery, SharedFontData};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

#[cfg(feature = "system-fonts")]
use fontdb;

/// A thread-safe handle to font data with rustybuzz Face creation.
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

/// Key for the font cache.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct FontCacheKey {
    family: String,
    weight: u16,
    style: u8, // 0=Normal, 1=Italic, 2=Oblique
}

impl FontCacheKey {
    fn new(family: &str, weight: FontWeight, style: FontStyle) -> Self {
        Self {
            family: family.to_lowercase(),
            weight: weight.numeric_value(),
            style: style_to_u8(&style),
        }
    }
}

fn style_to_u8(s: &FontStyle) -> u8 {
    match s {
        FontStyle::Normal => 0,
        FontStyle::Italic => 1,
        FontStyle::Oblique => 2,
    }
}

/// Metadata about a registered font face for PDF embedding.
#[derive(Debug, Clone)]
pub struct FontFaceInfo {
    pub postscript_name: String,
    pub family: String,
    pub weight: FontWeight,
    pub style: FontStyle,
    pub data: FontData,
}

/// Holds configuration and raw data to initialize font systems.
///
/// This struct provides a unified interface for font management that works
/// across different platforms:
/// - On native platforms with `system-fonts` feature: uses fontdb for system font discovery
/// - On WASM or without `system-fonts`: uses injected FontProvider
#[derive(Clone)]
pub struct SharedFontLibrary {
    /// fontdb database for system fonts (only available with system-fonts feature)
    #[cfg(feature = "system-fonts")]
    db: Arc<RwLock<fontdb::Database>>,

    /// Optional external font provider for custom/in-memory fonts
    external_provider: Option<Arc<dyn FontProvider>>,

    /// Cache of loaded font binaries, keyed by normalized (family, weight, style).
    font_data_cache: Arc<RwLock<HashMap<FontCacheKey, FontData>>>,

    /// Registry of font face metadata for PDF embedding.
    font_registry: Arc<RwLock<Vec<FontFaceInfo>>>,
}

impl SharedFontLibrary {
    /// Creates a new empty font library.
    pub fn new() -> Self {
        Self {
            #[cfg(feature = "system-fonts")]
            db: Arc::new(RwLock::new(fontdb::Database::new())),
            external_provider: None,
            font_data_cache: Arc::new(RwLock::new(HashMap::new())),
            font_registry: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Returns a clone of the fontdb database handle.
    ///
    /// # Deprecated
    ///
    /// This is internal API exposed for backward compatibility.
    /// Prefer using `registered_fonts()` for font enumeration.
    ///
    /// Only available with the `system-fonts` feature enabled.
    #[cfg(feature = "system-fonts")]
    pub(crate) fn font_db(&self) -> Arc<RwLock<fontdb::Database>> {
        self.db.clone()
    }

    /// Creates a font library using only the provided FontProvider.
    ///
    /// This is the recommended constructor for WASM or environments without
    /// system font access. No fontdb is used.
    pub fn from_provider(provider: Arc<dyn FontProvider>) -> Self {
        let lib = Self::new();
        lib.with_provider(provider)
    }

    /// Adds an external font provider.
    ///
    /// Fonts from the external provider take precedence over system fonts.
    pub fn with_provider(mut self, provider: Arc<dyn FontProvider>) -> Self {
        self.external_provider = Some(provider);
        self
    }

    /// Enables system font loading (native platforms only).
    ///
    /// Only available with the `system-fonts` feature enabled.
    #[cfg(feature = "system-fonts")]
    pub fn with_system_fonts(self, enable: bool) -> Self {
        if enable
            && let Ok(mut db) = self.db.write() {
                db.load_system_fonts();
            }
        self
    }

    /// Adds font data directly to the fontdb database.
    ///
    /// Only available with the `system-fonts` feature enabled.
    #[cfg(feature = "system-fonts")]
    pub fn add_fallback_font(&self, data: Vec<u8>) {
        log::debug!("add_fallback_font called with {} bytes", data.len());

        // Add to fontdb FIRST
        if let Ok(mut db) = self.db.write() {
            db.load_font_data(data.clone());
        }

        // Now query fontdb to get the CORRECT metadata (including PostScript name)
        // that fontdb extracted from the font
        if let Ok(db) = self.db.read() {
            // Get the last added face (most recent)
            let faces: Vec<_> = db.faces().collect();
            if let Some(face_info) = faces.last() {
                let ps_name = face_info.post_script_name.clone();
                let family = face_info.families.first()
                    .map(|(name, _)| name.clone())
                    .unwrap_or_else(|| "Unknown".to_string());

                // Map fontdb weight/style to our types
                let weight = match face_info.weight {
                    fontdb::Weight::THIN => FontWeight::Thin,
                    fontdb::Weight::LIGHT => FontWeight::Light,
                    fontdb::Weight::NORMAL => FontWeight::Regular,
                    fontdb::Weight::MEDIUM => FontWeight::Medium,
                    fontdb::Weight::SEMIBOLD => FontWeight::Bold,
                    fontdb::Weight::BOLD => FontWeight::Bold,
                    fontdb::Weight::BLACK => FontWeight::Black,
                    w => FontWeight::Numeric(w.0),
                };

                let style = match face_info.style {
                    fontdb::Style::Normal => FontStyle::Normal,
                    fontdb::Style::Italic => FontStyle::Italic,
                    fontdb::Style::Oblique => FontStyle::Italic,
                };

                log::debug!("Font metadata from fontdb: family='{}', ps_name='{}', weight={:?}, style={:?}",
                           family, ps_name, weight, style);

                // CRITICAL FIX: Register the font for PDF embedding with the SAME PostScript name
                // that fontdb uses, so they match during PDF generation
                log::debug!("Registering font '{}' (family: {}) for PDF embedding", ps_name, family);
                let font_data = Arc::new(FontInstance::new(Arc::new(data)));

                if let Ok(mut registry) = self.font_registry.write() {
                    // Check if already registered
                    if !registry.iter().any(|f| f.postscript_name == ps_name) {
                        registry.push(FontFaceInfo {
                            postscript_name: ps_name.clone(),
                            family: family.clone(),
                            weight,
                            style,
                            data: font_data,
                        });
                        log::debug!("Font '{}' registered successfully. Registry now has {} fonts", ps_name, registry.len());
                    } else {
                        log::debug!("Font '{}' already registered", ps_name);
                    }
                } else {
                    log::warn!("Failed to acquire write lock on font_registry");
                }
            } else {
                log::warn!("No face found after loading font data");
            }
        } else {
            log::warn!("Failed to acquire read lock on fontdb");
        }
    }

    /// Adds fonts from a directory (native platforms only).
    ///
    /// Only available with the `system-fonts` feature enabled.
    #[cfg(all(feature = "system-fonts", not(target_arch = "wasm32")))]
    pub fn add_font_dir<P: AsRef<std::path::Path>>(&self, path: P) {
        if let Ok(mut db) = self.db.write() {
            db.load_fonts_dir(path);
        }
    }

    /// Loads fallback fonts from the filesystem.
    ///
    /// This is a convenience method for native platforms. For WASM,
    /// use `from_provider` with pre-loaded fonts instead.
    ///
    /// Only available with the `system-fonts` feature enabled on native platforms.
    #[cfg(all(feature = "system-fonts", not(target_arch = "wasm32")))]
    pub fn load_fallback_font(&self) {
        // Try multiple paths for fonts - supports workspace root and nested crate directories
        let font_paths = [
            "assets/fonts/Helvetica.ttf",           // From workspace root
            "../assets/fonts/Helvetica.ttf",        // From petty-core or single-level crate
            "../../assets/fonts/Helvetica.ttf",     // From crates/*/
            "../../../assets/fonts/Helvetica.ttf",  // From deeper nesting
        ];
        let mut loaded_regular = false;
        for path in &font_paths {
            match std::fs::read(path) {
                Ok(regular) => {
                    log::debug!("Successfully loaded fallback font from: {}", path);
                    self.add_fallback_font(regular);
                    loaded_regular = true;
                    break;
                }
                Err(e) => {
                    log::debug!("Failed to load font from {}: {}", path, e);
                }
            }
        }

        if !loaded_regular {
            log::warn!("Failed to load regular fallback font from any path");
        }

        let bold_paths = [
            "assets/fonts/helvetica-bold.ttf",           // From workspace root
            "../assets/fonts/helvetica-bold.ttf",        // From petty-core or single-level crate
            "../../assets/fonts/helvetica-bold.ttf",     // From crates/*/
            "../../../assets/fonts/helvetica-bold.ttf",  // From deeper nesting
        ];
        let mut loaded_bold = false;
        for path in &bold_paths {
            match std::fs::read(path) {
                Ok(bold) => {
                    log::debug!("Successfully loaded bold fallback font from: {}", path);
                    self.add_fallback_font(bold);
                    loaded_bold = true;
                    break;
                }
                Err(e) => {
                    log::debug!("Failed to load bold font from {}: {}", path, e);
                }
            }
        }

        if !loaded_bold {
            log::warn!("Failed to load bold fallback font from any path");
        }
    }

    /// Stub for WASM or when system-fonts is disabled - does nothing since filesystem isn't available.
    #[cfg(any(target_arch = "wasm32", not(feature = "system-fonts")))]
    pub fn load_fallback_font(&self) {
        // No-op on WASM or without system-fonts - fonts must be provided via FontProvider
    }

    /// Resolves the raw font data for a given style.
    ///
    /// Resolution order:
    /// 1. External FontProvider (if set)
    /// 2. fontdb database
    ///
    /// # Errors
    ///
    /// Returns `FontError::NotFound` if no matching font is found in any source.
    pub fn resolve_font_data(&self, style: &ComputedStyle) -> Result<FontData, petty_traits::FontError> {
        let family = style.text.font_family.as_str();
        let weight = style.text.font_weight.clone();
        let font_style = style.text.font_style.clone();

        log::debug!("Resolving font: family='{}', weight={:?}, style={:?}", family, weight, font_style);

        let cache_key = FontCacheKey::new(family, weight.clone(), font_style.clone());

        // Fast path: check unified cache
        {
            if let Ok(cache) = self.font_data_cache.read()
                && let Some(data) = cache.get(&cache_key) {
                    log::debug!("  → Found in cache");
                    return Ok(data.clone());
                }
        }

        // Try external provider first
        if let Some(ref provider) = self.external_provider {
            log::debug!("  → Trying external provider");
            let query = FontQuery::new(family)
                .with_weight(weight.clone())
                .with_style(font_style.clone())
                .with_fallbacks(&["sans-serif"]);

            if let Ok(font_bytes) = provider.load_font(&query) {
                log::debug!("  → Found via external provider");
                return self.cache_font_data(cache_key, font_bytes, family, &weight, &font_style);
            }
            log::debug!("  → Not found in external provider");
        }

        // Fall back to fontdb (if available)
        #[cfg(feature = "system-fonts")]
        {
            self.resolve_from_fontdb(family, &weight, &font_style, cache_key)
        }
        #[cfg(not(feature = "system-fonts"))]
        {
            Err(petty_traits::FontError::NotFound {
                family: family.to_string(),
                weight: weight.clone(),
                style: font_style.clone(),
            })
        }
    }

    /// Resolves a font from fontdb database.
    ///
    /// Only available with the `system-fonts` feature enabled.
    #[cfg(feature = "system-fonts")]
    fn resolve_from_fontdb(
        &self,
        family: &str,
        weight: &FontWeight,
        font_style: &FontStyle,
        cache_key: FontCacheKey,
    ) -> Result<FontData, petty_traits::FontError> {
        log::debug!("  → Trying fontdb");
        let fontdb_weight = map_weight(weight.clone());
        let fontdb_style = map_style(font_style.clone());

        let query = fontdb::Query {
            families: &[fontdb::Family::Name(family), fontdb::Family::SansSerif],
            weight: fontdb_weight,
            stretch: fontdb::Stretch::Normal,
            style: fontdb_style,
        };

        let id = {
            let db = self.db.read()
                .map_err(|_| petty_traits::FontError::LoadFailed {
                    path: family.to_string(),
                    message: "fontdb lock poisoned".to_string(),
                })?;
            db.query(&query).or_else(|| {
                log::debug!("  → Primary query failed, trying SansSerif fallback");
                db.query(&fontdb::Query {
                    families: &[fontdb::Family::SansSerif],
                    weight: fontdb_weight,
                    stretch: fontdb::Stretch::Normal,
                    style: fontdb_style,
                })
            }).ok_or_else(|| {
                log::warn!("  → FONT NOT FOUND in fontdb: {} {:?} {:?}", family, weight, font_style);
                petty_traits::FontError::NotFound {
                    family: family.to_string(),
                    weight: weight.clone(),
                    style: font_style.clone(),
                }
            })?
        };

        log::debug!("  → Found in fontdb: ID={:?}", id);

        // Load data from fontdb
        let db = self.db.read()
            .map_err(|_| petty_traits::FontError::LoadFailed {
                path: family.to_string(),
                message: "fontdb lock poisoned".to_string(),
            })?;
        let face_info = db.face(id)
            .ok_or_else(|| petty_traits::FontError::NotFound {
                family: family.to_string(),
                weight: weight.clone(),
                style: font_style.clone(),
            })?;

        log::debug!("  → Matched font: {:?} ({})", face_info.families, face_info.post_script_name);

        let font_bytes: SharedFontData = match &face_info.source {
            fontdb::Source::Binary(data) => {
                log::debug!("  → Loading from binary ({} bytes)", data.as_ref().as_ref().len());
                Arc::new(data.as_ref().as_ref().to_vec())
            }
            #[cfg(not(target_arch = "wasm32"))]
            fontdb::Source::File(path) => {
                log::debug!("  → Loading from file: {}", path.display());
                Arc::new(std::fs::read(path)
                    .map_err(|e| petty_traits::FontError::LoadFailed {
                        path: path.display().to_string(),
                        message: e.to_string(),
                    })?)
            }
            #[cfg(target_arch = "wasm32")]
            fontdb::Source::File(path) => {
                return Err(petty_traits::FontError::LoadFailed {
                    path: path.display().to_string(),
                    message: "file system not available on WASM".to_string(),
                });
            }
            _ => return Err(petty_traits::FontError::InvalidData(
                "unsupported font source type".to_string()
            )),
        };

        // Get postscript name for registry
        let postscript_name = face_info.post_script_name.clone();

        // Drop the db lock before caching
        drop(db);

        self.cache_font_data_with_psname(
            cache_key,
            font_bytes,
            family,
            weight,
            font_style,
            postscript_name,
        )
    }

    /// Caches font data and registers it for PDF embedding.
    fn cache_font_data(
        &self,
        cache_key: FontCacheKey,
        font_bytes: SharedFontData,
        family: &str,
        weight: &FontWeight,
        style: &FontStyle,
    ) -> Result<FontData, petty_traits::FontError> {
        // Try to extract postscript name from font data
        let postscript_name = self.extract_postscript_name(&font_bytes)
            .unwrap_or_else(|| format!("{}-{:?}-{:?}", family, weight, style));

        self.cache_font_data_with_psname(cache_key, font_bytes, family, weight, style, postscript_name)
    }

    fn cache_font_data_with_psname(
        &self,
        cache_key: FontCacheKey,
        font_bytes: SharedFontData,
        family: &str,
        weight: &FontWeight,
        style: &FontStyle,
        postscript_name: String,
    ) -> Result<FontData, petty_traits::FontError> {
        let instance = Arc::new(FontInstance::new(font_bytes));

        // Cache the FontData
        if let Ok(mut cache) = self.font_data_cache.write() {
            cache.insert(cache_key, instance.clone());
        }

        // Register for PDF embedding
        if let Ok(mut registry) = self.font_registry.write() {
            // Check if already registered
            if !registry.iter().any(|f| f.postscript_name == postscript_name) {
                registry.push(FontFaceInfo {
                    postscript_name,
                    family: family.to_string(),
                    weight: weight.clone(),
                    style: style.clone(),
                    data: instance.clone(),
                });
            }
        }

        Ok(instance)
    }

    /// Extracts the PostScript name from font data using ttf-parser.
    /// Tries multiple name IDs as fallback if PostScript name is not available.
    fn extract_postscript_name(&self, data: &[u8]) -> Option<String> {
        let face = ttf_parser::Face::parse(data, 0).ok()?;

        // Try PostScript name first (nameID 6)
        if let Some(ps_name) = face.names().into_iter()
            .find(|n| n.name_id == ttf_parser::name_id::POST_SCRIPT_NAME)
            .and_then(|n| n.to_string()) {
            log::debug!("Found PostScript name (ID 6): {}", ps_name);
            return Some(ps_name);
        }

        // Fallback to Full Font Name (nameID 4)
        if let Some(full_name) = face.names().into_iter()
            .find(|n| n.name_id == ttf_parser::name_id::FULL_NAME)
            .and_then(|n| n.to_string()) {
            log::debug!("Using Full Name (ID 4) as fallback: {}", full_name);
            return Some(full_name.replace(" ", ""));  // Remove spaces for PostScript compatibility
        }

        // Last resort: Family name (nameID 1)
        if let Some(family) = face.names().into_iter()
            .find(|n| n.name_id == ttf_parser::name_id::FAMILY)
            .and_then(|n| n.to_string()) {
            log::debug!("Using Family Name (ID 1) as fallback: {}", family);
            return Some(family.replace(" ", ""));
        }

        log::warn!("Could not extract any usable name from font data");
        None
    }

    /// Returns an iterator over all registered font faces.
    ///
    /// This is used by PDF renderers to enumerate fonts for embedding.
    pub fn registered_fonts(&self) -> Vec<FontFaceInfo> {
        self.font_registry.read()
            .map(|r| r.clone())
            .unwrap_or_default()
    }

    /// Returns font face info by postscript name.
    pub fn get_font_by_postscript_name(&self, name: &str) -> Option<FontFaceInfo> {
        self.font_registry.read().ok()?
            .iter()
            .find(|f| f.postscript_name == name)
            .cloned()
    }
}

impl Default for SharedFontLibrary {
    fn default() -> Self {
        let lib = Self::new();
        lib.load_fallback_font();
        lib
    }
}

#[cfg(feature = "system-fonts")]
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

#[cfg(feature = "system-fonts")]
fn map_style(s: FontStyle) -> fontdb::Style {
    match s {
        FontStyle::Normal => fontdb::Style::Normal,
        FontStyle::Italic => fontdb::Style::Italic,
        FontStyle::Oblique => fontdb::Style::Oblique,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use petty_traits::InMemoryFontProvider;

    #[test]
    fn test_font_library_with_provider() {
        let provider = InMemoryFontProvider::new();
        // Add a fake font (won't actually work for shaping, but tests the plumbing)
        provider.add_font("TestFont", FontWeight::Regular, FontStyle::Normal, vec![0, 1, 2, 3]).unwrap();

        let library = SharedFontLibrary::from_provider(Arc::new(provider));

        // The provider should have the font
        assert!(library.external_provider.is_some());
    }

    #[test]
    fn test_font_cache_key() {
        let key1 = FontCacheKey::new("Arial", FontWeight::Bold, FontStyle::Normal);
        let key2 = FontCacheKey::new("arial", FontWeight::Bold, FontStyle::Normal);
        let key3 = FontCacheKey::new("Arial", FontWeight::Regular, FontStyle::Normal);

        // Case insensitive
        assert_eq!(key1, key2);
        // Different weight = different key
        assert_ne!(key1, key3);
    }
}
