use crate::core::style::dimension::Dimension;
use crate::core::style::stylesheet::ElementStyle;
use crate::parser::json::ast::{
    JsonContainer, JsonNode, JsonTable, JsonTableBody, JsonTableColumn, JsonTableHeader, TemplateNode,
};
use crate::templating::node::TemplateBuilder;
use crate::templating::style::impl_styled_widget;

/// Builder for a table cell. A cell is a block-level container.
#[derive(Default, Clone)]
pub struct Cell {
    style_names: Vec<String>,
    style_override: ElementStyle,
    children: Vec<Box<dyn TemplateBuilder>>,
}

impl Cell {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn child(mut self, child: impl TemplateBuilder + 'static) -> Self {
        self.children.push(Box::new(child));
        self
    }

    pub fn style_name(mut self, name: &str) -> Self {
        self.style_names.push(name.to_string());
        self
    }
}

impl TemplateBuilder for Cell {
    /// A cell is represented as a "Block" in the JSON AST
    fn build(self: Box<Self>) -> TemplateNode {
        TemplateNode::Static(JsonNode::Block(JsonContainer {
            style_names: self.style_names,
            style_override: self.style_override,
            children: self.children.into_iter().map(|c| c.build()).collect(),
        }))
    }
}

impl_styled_widget!(Cell);

/// Builder for a table row.
#[derive(Default, Clone)]
pub struct Row {
    cells: Vec<Cell>,
}

impl Row {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cell(mut self, cell: Cell) -> Self {
        self.cells.push(cell);
        self
    }
}

impl TemplateBuilder for Row {
    /// A row is represented as a "Block" containing cell "Blocks" in the JSON AST
    fn build(self: Box<Self>) -> TemplateNode {
        TemplateNode::Static(JsonNode::Block(JsonContainer {
            style_names: vec![],
            style_override: Default::default(),
            children: self.cells.into_iter().map(|c| Box::new(c).build()).collect(),
        }))
    }
}

/// Builder for a table column definition.
#[derive(Default, Clone)]
pub struct Column {
    width: Option<Dimension>,
    style: Option<String>,
    header_style: Option<String>,
}

impl Column {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn width(mut self, width: Dimension) -> Self {
        self.width = Some(width);
        self
    }
    pub fn style(mut self, style_name: &str) -> Self {
        self.style = Some(style_name.to_string());
        self
    }
    pub fn header_style(mut self, style_name: &str) -> Self {
        self.header_style = Some(style_name.to_string());
        self
    }
}

/// Builder for a `<Table>` node.
#[derive(Default, Clone)]
pub struct Table {
    style_names: Vec<String>,
    style_override: ElementStyle,
    columns: Vec<Column>,
    header_children: Vec<Box<dyn TemplateBuilder>>,
    body_children: Vec<Box<dyn TemplateBuilder>>,
}

impl Table {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn style_name(mut self, name: &str) -> Self {
        self.style_names.push(name.to_string());
        self
    }

    pub fn column(mut self, column: Column) -> Self {
        self.columns.push(column);
        self
    }

    /// Adds a static row to the table's header section.
    pub fn header_row(mut self, row: Row) -> Self {
        self.header_children.push(Box::new(row));
        self
    }

    /// Adds a static row to the table's body section.
    pub fn body_row(mut self, row: Row) -> Self {
        self.body_children.push(Box::new(row));
        self
    }

    /// Adds a child to the table body. Can be a `Row`, or a control flow
    /// element like `Each` or `If` that generates rows.
    pub fn child(mut self, child: impl TemplateBuilder + 'static) -> Self {
        self.body_children.push(Box::new(child));
        self
    }
}

impl_styled_widget!(Table);

impl TemplateBuilder for Table {
    fn build(self: Box<Self>) -> TemplateNode {
        let header = if self.header_children.is_empty() {
            None
        } else {
            Some(JsonTableHeader {
                rows: self
                    .header_children
                    .into_iter()
                    .map(|r| r.build())
                    .collect(),
            })
        };

        TemplateNode::Static(JsonNode::Table(JsonTable {
            style_names: self.style_names,
            style_override: self.style_override,
            columns: self
                .columns
                .into_iter()
                .map(|c| JsonTableColumn {
                    width: c.width,
                    style: c.style,
                    header_style: c.header_style,
                })
                .collect(),
            header,
            body: JsonTableBody {
                rows: self
                    .body_children
                    .into_iter()
                    .map(|r| r.build())
                    .collect(),
            },
        }))
    }
}