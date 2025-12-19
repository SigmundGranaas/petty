//! PDF composition utilities for merging and overlaying PDF documents.
//!
//! This crate provides low-level PDF manipulation using lopdf:
//! - Deep object copying with cycle detection
//! - Document merging (prepend/append pages)
//! - Content overlaying (headers/footers)

mod error;

pub use error::ComposerError;

use lopdf::{dictionary, Document, Object, ObjectId, Stream};
use std::collections::HashMap;

/// A helper struct to manage the state of copying objects between documents.
struct ObjectCopier<'a> {
    source_doc: &'a Document,
    target_doc: &'a mut Document,
    id_map: HashMap<ObjectId, ObjectId>,
}

impl<'a> ObjectCopier<'a> {
    fn new(source_doc: &'a Document, target_doc: &'a mut Document) -> Self {
        Self { source_doc, target_doc, id_map: HashMap::new() }
    }

    /// Deep copies an object from the source document to the target document.
    /// It recursively copies all referenced objects, ensuring that each object
    /// is only copied once by tracking it in the `id_map`.
    fn copy_object(&mut self, source_id: ObjectId) -> Result<ObjectId, lopdf::Error> {
        // If this object has already been mapped, return its new ID to avoid re-copying.
        if let Some(target_id) = self.id_map.get(&source_id) {
            return Ok(*target_id);
        }

        // --- THE FIX for Stack Overflow ---
        // Pre-emptively create a new object ID and add it to the map BEFORE recursing.
        // This is crucial for breaking cyclical references (e.g., Page -> Parent -> Kids -> Page).
        // We add a temporary Null object which we will replace with the real content later.
        let new_id = self.target_doc.add_object(Object::Null);
        self.id_map.insert(source_id, new_id);

        // Now, get the original object and recursively remap its references.
        let obj = self.source_doc.get_object(source_id)?.clone();
        let new_obj = self.remap_references(obj)?;

        // Replace the temporary Null object with the final, remapped object.
        if let Some(target_obj) = self.target_doc.objects.get_mut(&new_id) {
            *target_obj = new_obj;
        } else {
            // This case should be logically impossible if add_object and get_mut work correctly.
            // --- THE FIX for the E0308 error ---
            // The ObjectNotFound variant requires the ID of the object that was not found.
            return Err(lopdf::Error::ObjectNotFound(new_id));
        }

        Ok(new_id)
    }


    /// Traverses an object and replaces any `Object::Reference` with a new ID
    /// from the target document by recursively calling `copy_object`.
    fn remap_references(&mut self, obj: Object) -> Result<Object, lopdf::Error> {
        match obj {
            Object::Reference(id) => {
                let new_id = self.copy_object(id)?;
                Ok(Object::Reference(new_id))
            }
            Object::Array(arr) => {
                let new_arr = arr
                    .into_iter()
                    .map(|o| self.remap_references(o))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(Object::Array(new_arr))
            }
            Object::Dictionary(mut dict) => {
                for (_, value) in dict.iter_mut() {
                    *value = self.remap_references(value.clone())?;
                }
                Ok(Object::Dictionary(dict))
            }
            Object::Stream(mut stream) => {
                for (_, value) in stream.dict.iter_mut() {
                    *value = self.remap_references(value.clone())?;
                }
                Ok(Object::Stream(stream))
            }
            _ => Ok(obj), // Primitive objects don't have references
        }
    }
}

/// Merges the pages from a source document into a target document.
///
/// This function is complex. It copies all page objects and their dependent
/// objects (resources, content streams, etc.) from `source` to `target`,
/// creating new object IDs to avoid collisions.
///
/// # Arguments
/// * `target` - The document to merge into.
/// * `source` - The document to take pages from.
/// * `prepend` - If `true`, pages from `source` are added to the beginning.
///               If `false`, they are appended.
///
/// **Note:** This function does not yet adjust hyperlinks or outlines in the
/// target document when prepending pages. This is a significant limitation
/// that needs to be addressed for features like prepending a table of contents.
pub fn merge_documents(
    target: &mut Document,
    source: Document,
    prepend: bool,
) -> Result<(), ComposerError> {
    let source_pages = source.get_pages();
    if source_pages.is_empty() {
        return Ok(());
    }

    let mut copier = ObjectCopier::new(&source, target);
    let mut new_page_ids = Vec::new();
    let mut copied_page_ids = Vec::new();

    // Sort source pages to maintain order
    let mut sorted_source_pages: Vec<_> = source_pages.into_iter().collect();
    sorted_source_pages.sort_by_key(|(page_num, _)| *page_num);

    for (_, page_id) in sorted_source_pages {
        // `copy_object` is recursive and will copy the page dictionary and all
        // objects it references (content streams, resources, fonts, etc.).
        let new_page_id = copier.copy_object(page_id)?;
        new_page_ids.push(Object::Reference(new_page_id));
        copied_page_ids.push(new_page_id);
    }

    // Now, manipulate the page tree in the target document
    let root_id = target.trailer.get(b"Root")?.as_reference()?;
    let root_dict = target.get_object_mut(root_id)?.as_dict_mut()?;
    let pages_id = root_dict.get(b"Pages")?.as_reference()?;
    let pages_dict = target.get_object_mut(pages_id)?.as_dict_mut()?;

    let mut kids = pages_dict.get(b"Kids")?.as_array()?.clone();
    let original_count = pages_dict.get(b"Count")?.as_i64()?;

    if prepend {
        // TODO: This is where link/outline adjustment for the *original* pages would need to happen.
        // For now, we just prepend the new pages.
        let mut final_kids = new_page_ids;
        final_kids.extend(kids);
        kids = final_kids;
    } else {
        kids.extend(new_page_ids);
    }

    pages_dict.set("Kids", Object::Array(kids));
    pages_dict.set("Count", original_count + (source.get_pages().len() as i64));

    // Parent references in the copied page objects need to be updated to point to the target's page tree.
    for page_id in copied_page_ids {
        if let Ok(Object::Dictionary(page_dict)) = target.get_object_mut(page_id) {
            page_dict.set("Parent", Object::Reference(pages_id));
        }
    }

    Ok(())
}

/// Adds a new content stream to an existing page, overlaying it on top.
///
/// This is useful for adding headers, footers, or watermarks. It wraps the
/// existing page content and the new content in a new array of content streams.
///
/// # Arguments
/// * `doc` - The document containing the page to modify.
/// * `page_id` - The `ObjectId` of the page to add the overlay to.
/// * `content_stream` - The raw bytes of the new content stream.
pub fn overlay_content(
    doc: &mut Document,
    page_id: ObjectId,
    content_stream: Vec<u8>,
) -> Result<(), ComposerError> {
    let stream = Stream::new(dictionary! {}, content_stream);
    let new_content_id = doc.add_object(Object::Stream(stream));

    let page_dict = doc.get_object_mut(page_id)?.as_dict_mut()?;

    // The `get_mut` method on a `lopdf::Dictionary` returns a `Result`, not an `Option`.
    // We match on `Ok` for success (key exists) and `Err` for failure (key not found).
    match page_dict.get_mut(b"Contents") {
        Ok(contents_obj) => {
            let mut new_contents_array = match contents_obj.as_array() {
                Ok(arr) => arr.clone(),
                Err(_) => {
                    // It's not an array, so it must be a single reference. Wrap it.
                    vec![contents_obj.clone()]
                }
            };

            // Add the new content stream to the end, so it's drawn on top.
            new_contents_array.push(Object::Reference(new_content_id));

            page_dict.set("Contents", Object::Array(new_contents_array));
        }
        Err(_) => {
            return Err(ComposerError::Other(format!(
                "Page {:?} is missing a /Contents key.",
                page_id
            )));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::content::{Content, Operation};
    use lopdf::{dictionary, Document, Object, StringFormat};

    /// Creates a simple dummy PDF document with a specified number of pages.
    /// Each page has a unique text content "Page X".
    fn create_dummy_pdf(num_pages: u32, text_prefix: &str) -> Document {
        let mut doc = Document::with_version("1.7");
        let pages_id = doc.new_object_id();
        let font_id = doc.add_object(dictionary! {
            "Type" => "Font",
            "Subtype" => "Type1",
            "BaseFont" => "Helvetica",
        });
        let resources_id = doc.add_object(dictionary! {
            "Font" => dictionary! { "F1" => font_id },
        });

        let mut page_ids = vec![];
        for i in 1..=num_pages {
            let content = Content {
                operations: vec![
                    Operation::new("BT", vec![]),
                    Operation::new("Tf", vec!["F1".into(), 12.into()]),
                    Operation::new("Td", vec![100.into(), 700.into()]),
                    Operation::new(
                        "Tj",
                        vec![Object::String(
                            format!("{} {}", text_prefix, i).into_bytes(),
                            StringFormat::Literal,
                        )],
                    ),
                    Operation::new("ET", vec![]),
                ],
            };
            let content_id = doc.add_object(Stream::new(dictionary! {}, content.encode().unwrap()));
            let page_id = doc.add_object(dictionary! {
                "Type" => "Page",
                "Parent" => pages_id,
                "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
                "Contents" => content_id,
                "Resources" => resources_id,
            });
            page_ids.push(page_id.into());
        }

        let pages_dict = dictionary! {
            "Type" => "Pages",
            "Kids" => page_ids,
            "Count" => num_pages as i64,
        };
        doc.objects.insert(pages_id, pages_dict.into());

        let catalog_id = doc.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        });
        doc.trailer.set("Root", catalog_id);

        doc
    }

    #[test]
    fn test_merge_documents_append() {
        let mut target_doc = create_dummy_pdf(2, "Target Page");
        let source_doc = create_dummy_pdf(3, "Source Page");

        let original_target_pages = target_doc.get_pages().len();
        let source_pages = source_doc.get_pages().len();

        merge_documents(&mut target_doc, source_doc, false).unwrap();

        assert_eq!(
            target_doc.get_pages().len(),
            original_target_pages + source_pages
        );

        // Check page content to verify order
        let pages = target_doc.get_pages();
        let page_3_content = target_doc
            .get_page_content(*pages.get(&3).unwrap())
            .unwrap();
        assert!(String::from_utf8_lossy(&page_3_content).contains("Source Page 1"));
    }

    #[test]
    fn test_merge_documents_prepend() {
        let mut target_doc = create_dummy_pdf(2, "Target Page");
        let source_doc = create_dummy_pdf(3, "Source Page");

        let original_target_pages = target_doc.get_pages().len();
        let source_pages = source_doc.get_pages().len();

        merge_documents(&mut target_doc, source_doc, true).unwrap();

        assert_eq!(
            target_doc.get_pages().len(),
            original_target_pages + source_pages
        );

        // Check page content to verify order
        let pages = target_doc.get_pages();
        let page_1_content = target_doc
            .get_page_content(*pages.get(&1).unwrap())
            .unwrap();
        let page_4_content = target_doc
            .get_page_content(*pages.get(&4).unwrap())
            .unwrap();

        assert!(String::from_utf8_lossy(&page_1_content).contains("Source Page 1"));
        assert!(String::from_utf8_lossy(&page_4_content).contains("Target Page 1"));
    }

    #[test]
    fn test_overlay_content() {
        let mut doc = create_dummy_pdf(1, "Original Content");
        let page_id = doc.get_pages().get(&1).cloned().unwrap();

        let overlay_stream = Content {
            operations: vec![
                Operation::new("BT", vec![]),
                Operation::new("Tf", vec!["F1".into(), 12.into()]),
                Operation::new("Td", vec![100.into(), 100.into()]),
                Operation::new(
                    "Tj",
                    vec![Object::String(
                        "Overlay Content".to_string().into_bytes(),
                        StringFormat::Literal,
                    )],
                ),
                Operation::new("ET", vec![]),
            ],
        }
            .encode()
            .unwrap();

        overlay_content(&mut doc, page_id, overlay_stream).unwrap();

        // Check that the page now has two content streams
        let page_dict = doc.get_object(page_id).unwrap().as_dict().unwrap();
        let contents_array = page_dict.get(b"Contents").unwrap().as_array().unwrap();
        assert_eq!(contents_array.len(), 2);

        // Check that the full page content contains both strings
        let full_content = doc.get_page_content(page_id).unwrap();
        let content_str = String::from_utf8_lossy(&full_content);
        assert!(content_str.contains("Original Content"));
        assert!(content_str.contains("Overlay Content"));
    }
}
