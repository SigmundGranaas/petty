use crate::stylesheet::TableColumn;
use serde_json::Value;
use std::borrow::Cow;

/// Represents a high-level command for the layout engine, forming an event stream.
/// This enum is the "language" between the parser and the layout processor.
#[derive(Debug, Clone, PartialEq)]
pub enum Event<'a> {
    StartDocument,
    EndDocument,
    BeginPageSequenceItem {
        context: &'a Value,
    },
    EndPageSequenceItem,
    StartContainer {
        style: Option<Cow<'a, str>>,
    },
    EndContainer,
    AddText {
        content: Cow<'a, str>,
        style: Option<Cow<'a, str>>,
    },
    AddRectangle {
        style: Option<Cow<'a, str>>,
    },
    AddImage {
        src: Cow<'a, str>,
        style: Option<Cow<'a, str>>,
    },
    StartTable {
        style: Option<Cow<'a, str>>,
        columns: Cow<'a, [TableColumn]>,
    },
    StartHeader,
    EndHeader,
    StartRow {
        context: &'a Value,
        row_style_prefix: Option<String>,
    },
    AddCell {
        column_index: usize,
        content: Cow<'a, str>,
        style_override: Option<String>,
    },
    EndRow,
    EndTable,
    ForcePageBreak,
}