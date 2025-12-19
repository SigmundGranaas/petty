// src/pipeline/api.rs
use serde::Serialize;
use serde_json::Value;
use std::io::{Read, Seek};
use std::sync::Arc;

// Re-export IndexEntry from petty-core for backwards compatibility
pub use petty_core::ApiIndexEntry as IndexEntry;

/// A helper trait for creating a `Box<dyn ...>` that requires multiple non-auto traits.
pub trait ReadSeekSend: Read + Seek + Send {}
impl<T: Read + Seek + Send> ReadSeekSend for T {}

/// Represents the final, analyzed document structure.
///
/// This struct is the public API for template authors who need access to
/// document-wide metadata, such as the total page count, headings for a
/// table of contents, or lists of figures.
///
/// It is versioned to allow for non-breaking additions in the future.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Document {
    /// The total number of pages in the main body of the document.
    pub page_count: usize,
    /// The ISO 8601 timestamp of when the document was built.
    pub build_timestamp: String,
    /// A hierarchical list of all headings in the document.
    pub headings: Vec<Heading>,
    /// A list of all figures in the document.
    pub figures: Vec<Figure>,
    /// A list of all index entries.
    pub index_entries: Vec<IndexEntry>,
    /// A list of all named anchors and their locations.
    pub anchors: Vec<Anchor>,
    /// A list of all internal hyperlinks.
    pub hyperlinks: Vec<Hyperlink>,
}

/// Represents a single heading within the document.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Heading {
    /// The unique ID of the heading, suitable for creating links.
    pub id: String,
    /// The hierarchical level of the heading (e.g., 1 for `<h1>`, 2 for `<h2>`).
    pub level: u8,
    /// The text content of the heading.
    pub text: String,
    /// The page number on which this heading appears.
    pub page_number: usize,
}

/// Represents a figure (e.g., an image with a caption).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Figure {
    /// The unique ID of the figure.
    pub id: String,
    /// The caption text associated with the figure.
    pub caption: Option<String>,
    /// The page number on which this figure appears.
    pub page_number: usize,
}

// IndexEntry is re-exported from petty_core at the top of this file

/// Represents a named anchor location in the document.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Anchor {
    /// The unique ID of the anchor.
    pub id: String,
    /// The page number on which this anchor is located.
    pub page_number: usize,
    /// The Y position of the anchor on the page, in points from the top.
    pub y_position: f32,
}

/// Represents an internal hyperlink.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Hyperlink {
    /// The page number where the link appears.
    pub page_number: usize,
    /// The rectangular area of the link on the page, in points. [x1, y1, x2, y2]
    pub rect: [f32; 4],
    /// The anchor ID this link points to (e.g., "section-1").
    pub target_id: String,
}

/// A handover object between a `DataSourceProvider` and a `RenderingStrategy`.
///
/// This struct contains all the necessary artifacts for the rendering stage,
/// which may have been produced during the data provider stage.
pub struct PreparedDataSources {
    /// The primary data source, represented as an iterator of `serde_json::Value`.
    /// For simple streaming, this will be the user's original data.
    /// For advanced pipelines, this might be an empty iterator if the data was
    /// already consumed to produce the `document` metadata.
    pub data_iterator: Box<dyn Iterator<Item = Value> + Send>,

    /// Optional document-wide metadata, produced by an analysis pass.
    /// This is `None` for the simple "fast path" streaming case.
    pub document: Option<Arc<Document>>,

    /// An optional temporary file handle containing a pre-rendered PDF body.
    /// This is used by advanced pipelines that render a main body and then
    /// prepend or append content (like a table of contents).
    pub body_artifact: Option<Box<dyn ReadSeekSend>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn document_struct_serializes_correctly() {
        let doc = Document {
            page_count: 10,
            build_timestamp: "2023-10-27T10:00:00Z".to_string(),
            headings: vec![
                Heading {
                    id: "sec-1".to_string(),
                    level: 1,
                    text: "Section 1".to_string(),
                    page_number: 1,
                },
                Heading {
                    id: "sec-1-1".to_string(),
                    level: 2,
                    text: "Subsection 1.1".to_string(),
                    page_number: 2,
                },
            ],
            figures: vec![Figure {
                id: "fig-1".to_string(),
                caption: Some("My Figure".to_string()),
                page_number: 3,
            }],
            index_entries: vec![IndexEntry {
                text: "topic".to_string(),
                page_number: 5,
            }],
            anchors: vec![Anchor {
                id: "anchor-1".to_string(),
                page_number: 8,
                y_position: 100.0,
            }],
            hyperlinks: vec![Hyperlink {
                page_number: 1,
                rect: [10.0, 20.0, 80.0, 40.0],
                target_id: "anchor-1".to_string(),
            }],
        };

        let expected_json = json!({
          "pageCount": 10,
          "buildTimestamp": "2023-10-27T10:00:00Z",
          "headings": [
            {
              "id": "sec-1",
              "level": 1,
              "text": "Section 1",
              "pageNumber": 1
            },
            {
              "id": "sec-1-1",
              "level": 2,
              "text": "Subsection 1.1",
              "pageNumber": 2
            }
          ],
          "figures": [
            {
              "id": "fig-1",
              "caption": "My Figure",
              "pageNumber": 3
            }
          ],
          "indexEntries": [
            {
              "text": "topic",
              "pageNumber": 5
            }
          ],
          "anchors": [
            {
              "id": "anchor-1",
              "pageNumber": 8,
              "yPosition": 100.0
            }
          ],
          "hyperlinks": [
            {
                "pageNumber": 1,
                "rect": [10.0, 20.0, 80.0, 40.0],
                "targetId": "anchor-1"
            }
          ]
        });

        let serialized_doc = serde_json::to_value(&doc).unwrap();
        assert_eq!(serialized_doc, expected_json);
    }
}
