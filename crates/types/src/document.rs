use serde::Serialize;

/// An entry in the table of contents.
#[derive(Debug, Clone, Default)]
pub struct TocEntry {
    /// Heading level (1 = h1, 2 = h2, etc.)
    pub level: u8,
    /// The text content of the heading.
    pub text: String,
    /// The anchor ID to link to this heading.
    pub target_id: String,
}

/// Represents an entry for a document index (for API/serialization).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiIndexEntry {
    /// The text of the index entry.
    pub text: String,
    /// The page number on which this entry is referenced.
    pub page_number: usize,
}
