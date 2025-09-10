use crate::stylesheet::TableColumn;
use serde_json::Value;
use std::borrow::Cow;
use std::sync::Arc;

pub type SharedData = Arc<Vec<u8>>;

#[derive(Debug, Clone)]
pub enum IDFEvent<'a> {
    StartDocument,
    EndDocument,

    BeginPageSequence {
        context: &'a Value,
    },
    EndPageSequence,

    StartBlock {
        style: Option<Cow<'a, str>>,
    },
    EndBlock,

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
        data: Option<SharedData>, // Prepared for async fetching
    },

    StartTable {
        style: Option<Cow<'a, str>>,
        columns: Cow<'a, [TableColumn]>,
    },
    StartHeader,
    EndHeader,
    StartRow {
        context: &'a Value,
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