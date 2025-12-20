use lopdf::Document as LopdfDocument;
use std::collections::BTreeMap;

/// Extract all text content from a PDF document
pub fn extract_text(doc: &LopdfDocument) -> String {
    let mut text = String::new();
    let pages = doc.get_pages();
    for page_num in 1..=pages.len() {
        if let Ok(page_text) = doc.extract_text(&[page_num as u32]) {
            text.push_str(&page_text);
            text.push('\n');
        }
    }
    text
}

/// Extract font names from the PDF (both embedded and referenced fonts)
pub fn extract_font_names(doc: &LopdfDocument) -> Vec<String> {
    let mut fonts = std::collections::HashSet::new();

    // First, check all page Resources for font references
    let pages = doc.get_pages();
    for (_page_num, page_id) in pages.iter() {
        if let Ok(page_obj) = doc.get_object(*page_id) {
            if let Ok(page_dict) = page_obj.as_dict() {
                if let Ok(resources) = page_dict.get(b"Resources") {
                    // Try to resolve Resources as a reference or direct dictionary
                    let resources_dict = if let Ok(ref_id) = resources.as_reference() {
                        doc.get_object(ref_id).ok().and_then(|obj| obj.as_dict().ok())
                    } else {
                        resources.as_dict().ok()
                    };

                    if let Some(resources) = resources_dict {
                        if let Ok(font_dict) = resources.get(b"Font") {
                            // Try to resolve Font dictionary
                            let fonts_dict = if let Ok(ref_id) = font_dict.as_reference() {
                                doc.get_object(ref_id).ok().and_then(|obj| obj.as_dict().ok())
                            } else {
                                font_dict.as_dict().ok()
                            };

                            if let Some(fonts_dict) = fonts_dict {
                                // Iterate through font resources
                                for (_font_name, font_val) in fonts_dict.iter() {
                                    // Try as direct dictionary first
                                    let font_dict_opt = if let Ok(font_dict) = font_val.as_dict() {
                                        Some(font_dict)
                                    } else if let Ok(font_obj_id) = font_val.as_reference() {
                                        // Try as reference
                                        doc.get_object(font_obj_id).ok().and_then(|obj| obj.as_dict().ok())
                                    } else {
                                        None
                                    };

                                    if let Some(font_dict) = font_dict_opt {
                                        if let Ok(base_font) = font_dict.get(b"BaseFont") {
                                            if let Ok(font_name) = base_font.as_name() {
                                                fonts.insert(String::from_utf8_lossy(font_name).to_string());
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    fonts.into_iter().collect()
}

/// Get detailed font information from PDF
pub fn get_font_info(doc: &LopdfDocument) -> BTreeMap<String, BTreeMap<String, String>> {
    let mut font_info = BTreeMap::new();

    for (obj_id, object) in doc.objects.iter() {
        if let Ok(dict) = object.as_dict() {
            if let Ok(type_val) = dict.get(b"Type") {
                if let Ok(type_name) = type_val.as_name() {
                    if type_name == b"Font" {
                        let mut info = BTreeMap::new();

                        // Get subtype
                        if let Ok(subtype_val) = dict.get(b"Subtype") {
                            if let Ok(subtype_name) = subtype_val.as_name() {
                                info.insert(
                                    "Subtype".to_string(),
                                    String::from_utf8_lossy(subtype_name).to_string(),
                                );
                            }
                        }

                        // Get base font name
                        if let Ok(base_font_val) = dict.get(b"BaseFont") {
                            if let Ok(base_font_name) = base_font_val.as_name() {
                                info.insert(
                                    "BaseFont".to_string(),
                                    String::from_utf8_lossy(base_font_name).to_string(),
                                );
                            }
                        }

                        // Get encoding
                        if let Ok(encoding) = dict.get(b"Encoding") {
                            info.insert("Encoding".to_string(), format!("{:?}", encoding));
                        }

                        // Check for font descriptor
                        if let Ok(font_desc_ref) = dict.get(b"FontDescriptor") {
                            info.insert("HasDescriptor".to_string(), "true".to_string());

                            if let Ok(desc_ref_id) = font_desc_ref.as_reference() {
                                if let Ok(desc_obj) = doc.get_object(desc_ref_id) {
                                    if let Ok(desc_dict) = desc_obj.as_dict() {
                                        if desc_dict.has(b"FontFile")
                                            || desc_dict.has(b"FontFile2")
                                            || desc_dict.has(b"FontFile3")
                                        {
                                            info.insert("EmbeddedFont".to_string(), "true".to_string());
                                        }
                                    }
                                }
                            }
                        }

                        font_info.insert(format!("{:?}", obj_id), info);
                    }
                }
            }
        }
    }

    font_info
}

/// Information about a link annotation
#[derive(Debug)]
pub struct LinkAnnotation {
    pub rect: Option<[f32; 4]>,
    pub is_internal: bool,
    pub destination: String,
}

/// Extract link annotations from PDF pages
pub fn extract_link_annotations(doc: &LopdfDocument) -> Vec<LinkAnnotation> {
    let mut annotations = Vec::new();

    let pages = doc.get_pages();
    for (_page_num, page_id) in pages.iter() {
        if let Ok(page_obj) = doc.get_object(*page_id) {
            if let Ok(page_dict) = page_obj.as_dict() {
                if let Ok(annots_ref) = page_dict.get(b"Annots") {
                    // Try to resolve as array
                    let annots_array = if let Ok(arr) = annots_ref.as_array() {
                        arr.clone()
                    } else if let Ok(ref_id) = annots_ref.as_reference() {
                        if let Ok(annots_obj) = doc.get_object(ref_id) {
                            if let Ok(arr) = annots_obj.as_array() {
                                arr.clone()
                            } else {
                                continue;
                            }
                        } else {
                            continue;
                        }
                    } else {
                        continue;
                    };

                    // Iterate through annotations
                    for annot_ref in annots_array {
                        if let Ok(annot_id) = annot_ref.as_reference() {
                            if let Ok(annot_obj) = doc.get_object(annot_id) {
                                if let Ok(annot_dict) = annot_obj.as_dict() {
                                    // Check if it's a Link annotation
                                    if let Ok(subtype) = annot_dict.get(b"Subtype") {
                                        if let Ok(subtype_name) = subtype.as_name() {
                                            if subtype_name == b"Link" {
                                                let mut is_internal = false;
                                                let mut destination = String::new();

                                                // Check for Dest (internal link - direct destination)
                                                if let Ok(dest) = annot_dict.get(b"Dest") {
                                                    is_internal = true;
                                                    destination = format!("{:?}", dest);
                                                }

                                                // Check for Action (could be internal GoTo or external URI)
                                                if let Ok(action_ref) = annot_dict.get(b"A") {
                                                    // Try to resolve the action dictionary
                                                    let action_dict = if let Ok(ref_id) = action_ref.as_reference() {
                                                        doc.get_object(ref_id).ok().and_then(|obj| obj.as_dict().ok())
                                                    } else {
                                                        action_ref.as_dict().ok()
                                                    };

                                                    if let Some(action) = action_dict {
                                                        // Check the action type (S field)
                                                        if let Ok(action_type) = action.get(b"S") {
                                                            if let Ok(type_name) = action_type.as_name() {
                                                                // GoTo = internal link, URI = external link
                                                                is_internal = type_name == b"GoTo";
                                                            }
                                                        }
                                                        destination = format!("{:?}", action_ref);
                                                    } else {
                                                        destination = format!("{:?}", action_ref);
                                                    }
                                                }

                                                // Get rect (position)
                                                let rect = if let Ok(rect_obj) = annot_dict.get(b"Rect") {
                                                    if let Ok(arr) = rect_obj.as_array() {
                                                        if arr.len() >= 4 {
                                                            Some([
                                                                arr[0].as_f32().unwrap_or(0.0),
                                                                arr[1].as_f32().unwrap_or(0.0),
                                                                arr[2].as_f32().unwrap_or(0.0),
                                                                arr[3].as_f32().unwrap_or(0.0),
                                                            ])
                                                        } else {
                                                            None
                                                        }
                                                    } else {
                                                        None
                                                    }
                                                } else {
                                                    None
                                                };

                                                annotations.push(LinkAnnotation {
                                                    rect,
                                                    is_internal,
                                                    destination,
                                                });
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    annotations
}

/// Count internal links in the PDF
pub fn count_internal_links(doc: &LopdfDocument) -> usize {
    extract_link_annotations(doc)
        .iter()
        .filter(|a| a.is_internal)
        .count()
}

/// Count external links in the PDF
pub fn count_external_links(doc: &LopdfDocument) -> usize {
    extract_link_annotations(doc)
        .iter()
        .filter(|a| !a.is_internal)
        .count()
}

/// Get page dimensions (width, height) in points
pub fn get_page_dimensions(doc: &LopdfDocument, page_num: u32) -> Option<(f32, f32)> {
    let pages = doc.get_pages();
    let page_id = pages.get(&page_num)?;
    if let Ok(page_obj) = doc.get_object(*page_id) {
        if let Ok(page_dict) = page_obj.as_dict() {
            if let Ok(media_box) = page_dict.get(b"MediaBox") {
                if let Ok(arr) = media_box.as_array() {
                    if arr.len() >= 4 {
                        let width = arr[2].as_f32().ok()? - arr[0].as_f32().ok()?;
                        let height = arr[3].as_f32().ok()? - arr[1].as_f32().ok()?;
                        return Some((width, height));
                    }
                }
            }
        }
    }
    None
}

/// Check if PDF has outlines (bookmarks/TOC)
pub fn has_outlines(doc: &LopdfDocument) -> bool {
    if let Ok(catalog_ref) = doc.trailer.get(b"Root") {
        if let Ok(ref_id) = catalog_ref.as_reference() {
            if let Ok(catalog) = doc.get_dictionary(ref_id) {
                return catalog.has(b"Outlines");
            }
        }
    }
    false
}

// ============================================================================
// Fluent Assertion Macros
// ============================================================================

/// Assert that PDF contains specific text
#[macro_export]
macro_rules! assert_pdf_contains_text {
    ($pdf:expr, $text:expr) => {
        let extracted = $crate::common::pdf_assertions::extract_text(&$pdf.doc);
        assert!(
            extracted.contains($text),
            "PDF should contain '{}', but extracted text was:\n{}",
            $text,
            extracted
        );
    };
}

/// Assert that PDF does NOT contain specific text
#[macro_export]
macro_rules! assert_pdf_not_contains_text {
    ($pdf:expr, $text:expr) => {
        let extracted = $crate::common::pdf_assertions::extract_text(&$pdf.doc);
        assert!(
            !extracted.contains($text),
            "PDF should NOT contain '{}', but it was found in:\n{}",
            $text,
            extracted
        );
    };
}

/// Assert the number of pages in a PDF
#[macro_export]
macro_rules! assert_pdf_page_count {
    ($pdf:expr, $count:expr) => {
        assert_eq!(
            $pdf.page_count(),
            $count,
            "Expected {} pages, got {}",
            $count,
            $pdf.page_count()
        );
    };
}

/// Assert minimum number of pages
#[macro_export]
macro_rules! assert_pdf_min_pages {
    ($pdf:expr, $min:expr) => {
        assert!(
            $pdf.page_count() >= $min,
            "Expected at least {} pages, got {}",
            $min,
            $pdf.page_count()
        );
    };
}

/// Assert that PDF contains a font matching a pattern
#[macro_export]
macro_rules! assert_pdf_has_font {
    ($pdf:expr, $pattern:expr) => {
        let fonts = $crate::common::pdf_assertions::extract_font_names(&$pdf.doc);
        assert!(
            fonts.iter().any(|f| f.contains($pattern)),
            "PDF should contain font matching '{}', fonts found: {:?}",
            $pattern,
            fonts
        );
    };
}

/// Assert that PDF has a specific number of internal links
#[macro_export]
macro_rules! assert_pdf_internal_link_count {
    ($pdf:expr, $count:expr) => {
        let link_count = $crate::common::pdf_assertions::count_internal_links(&$pdf.doc);
        assert_eq!(
            link_count, $count,
            "Expected {} internal links, got {}",
            $count, link_count
        );
    };
}

/// Assert that PDF has at least a minimum number of internal links
#[macro_export]
macro_rules! assert_pdf_min_internal_links {
    ($pdf:expr, $min:expr) => {
        let link_count = $crate::common::pdf_assertions::count_internal_links(&$pdf.doc);
        assert!(
            link_count >= $min,
            "Expected at least {} internal links, got {}",
            $min,
            link_count
        );
    };
}

/// Assert page dimensions within tolerance
#[macro_export]
macro_rules! assert_pdf_page_size {
    ($pdf:expr, $page:expr, $width:expr, $height:expr) => {
        let dims = $crate::common::pdf_assertions::get_page_dimensions(&$pdf.doc, $page);
        assert!(dims.is_some(), "Could not get dimensions for page {}", $page);
        let (w, h) = dims.unwrap();
        assert!(
            (w - $width).abs() < 1.0,
            "Page {} width expected ~{}, got {}",
            $page,
            $width,
            w
        );
        assert!(
            (h - $height).abs() < 1.0,
            "Page {} height expected ~{}, got {}",
            $page,
            $height,
            h
        );
    };
}
