//! Defines the `OutputBuilder` trait, which decouples the XSLT executor
//! from the specific output tree format (e.g., IDF).

use super::ast::PreparsedStyles;
use petty_style::dimension::Dimension;

/// A trait that describes the semantic actions of building the output tree,
/// without exposing the underlying concrete node types.
pub trait OutputBuilder {
    // --- Block-level elements ---
    fn start_block(&mut self, styles: &PreparsedStyles);
    fn end_block(&mut self);

    fn start_flex_container(&mut self, styles: &PreparsedStyles);
    fn end_flex_container(&mut self);

    fn start_paragraph(&mut self, styles: &PreparsedStyles);
    fn end_paragraph(&mut self);

    fn start_list(&mut self, styles: &PreparsedStyles);
    fn end_list(&mut self);

    fn start_list_item(&mut self, styles: &PreparsedStyles);
    fn end_list_item(&mut self);

    fn start_image(&mut self, styles: &PreparsedStyles);
    fn end_image(&mut self);

    // --- Table elements ---
    fn start_table(&mut self, styles: &PreparsedStyles);
    fn end_table(&mut self);
    fn start_table_header(&mut self);
    fn end_table_header(&mut self);
    fn set_table_columns(&mut self, columns: &[Dimension]);
    fn start_table_row(&mut self, styles: &PreparsedStyles);
    fn end_table_row(&mut self);
    fn start_table_cell(&mut self, styles: &PreparsedStyles);
    fn end_table_cell(&mut self);

    // --- Inline-level elements ---
    fn add_text(&mut self, text: &str);

    // --- Special elements ---
    fn start_heading(&mut self, styles: &PreparsedStyles, level: u8);
    fn end_heading(&mut self);
    fn add_page_break(&mut self, master_name: Option<String>);

    fn start_styled_span(&mut self, styles: &PreparsedStyles);
    fn end_styled_span(&mut self);

    fn start_hyperlink(&mut self, styles: &PreparsedStyles);
    fn end_hyperlink(&mut self);

    // --- Attributes ---
    /// Sets an attribute on the currently open element.
    fn set_attribute(&mut self, name: &str, value: &str);
}
