// src/idf/mod.rs
use crate::stylesheet::TableColumn;
use serde_json::Value;
use std::borrow::Cow;
use std::sync::Arc;

pub type SharedData = Arc<Vec<u8>>;

// Add this new enum for flexbox
#[derive(Debug, Clone)]
pub enum FlexDirection {
    Row,
    Column,
}

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
    StartCell {
        column_index: usize,
        style_override: Option<String>,
    },
    EndCell, // NEW: End of a cell
    EndRow,
    EndTable,

    ForcePageBreak,

    // --- NEW: Inline-Level Content ---
    StartInline {
        style: Option<Cow<'a, str>>,
    },
    EndInline,
    AddLineBreak,

    // --- NEW: Interactive Elements ---
    AddHyperlink {
        href: Cow<'a, str>,
        style: Option<Cow<'a, str>>,
    },
    EndHyperlink,

    // --- NEW: Advanced Layout ---
    StartFlexContainer {
        style: Option<Cow<'a, str>>,
        direction: FlexDirection,
    },
    EndFlexContainer,

    // --- NEW: Structural Elements (Lists) ---
    StartList {
        style: Option<Cow<'a, str>>,
    },
    EndList,
    // ListItem is now handled by the parser, which emits generic container events.
}