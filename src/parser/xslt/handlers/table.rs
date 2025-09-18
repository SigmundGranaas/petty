// src/parser/xslt/handlers/table.rs
// src/parser/xslt/handlers/table.rs
use super::super::builder::TreeBuilder;
use super::super::util::{
    get_attr_owned_optional, parse_fo_attributes_to_element_style, parse_table_columns,
    OwnedAttributes,
};
use crate::error::PipelineError;
use crate::idf::{
    IRNode, TableBody, TableCell, TableColumnDefinition, TableHeader, TableRow,
};
use quick_xml::events::{Event as XmlEvent};
use quick_xml::name::QName;
use quick_xml::Reader;
use serde_json::Value;

impl<'a> TreeBuilder<'a> {
    pub(in crate::parser::xslt) fn handle_table_start(
        &mut self,
        attributes: &OwnedAttributes,
        reader: &mut Reader<&[u8]>,
        context: &'a Value,
    ) -> Result<(), PipelineError> {
        let style_name = get_attr_owned_optional(attributes, b"style")?;
        let style_override = parse_fo_attributes_to_element_style(attributes)?;

        // The entire table content is captured, then parsed in stages.
        // This is necessary to find the <columns> definition first.
        let inner_xml =
            super::super::super::xslt::capture_inner_xml(reader, QName(b"table"))?;

        // Stage 1: Find and parse <columns>
        let col_defs = self.parse_columns_from_inner_xml(&inner_xml)?;

        // Stage 2: Create the table node and push it onto the stack.
        let table_node = IRNode::Table {
            style_name,
            style_override,
            columns: col_defs,
            calculated_widths: Vec::new(),
            header: None,
            body: Box::new(TableBody { rows: Vec::new() }),
        };
        self.node_stack.push(table_node);

        // Stage 3: Re-parse the inner XML to build the header and body.
        let mut content_reader = Reader::from_str(&inner_xml);
        content_reader.config_mut().trim_text(false);
        self.parse_nodes(&mut content_reader, context)?;

        // Stage 4: Pop the completed table and add it to its parent.
        if let Some(table_node) = self.node_stack.pop() {
            self.push_block_to_parent(table_node);
        }

        Ok(())
    }

    fn parse_columns_from_inner_xml(
        &self,
        inner_xml: &str,
    ) -> Result<Vec<TableColumnDefinition>, PipelineError> {
        let mut columns_reader = Reader::from_str(inner_xml);
        let mut buf = Vec::new();
        loop {
            match columns_reader.read_event_into(&mut buf) {
                Ok(XmlEvent::Start(e)) if e.name().as_ref() == b"columns" => {
                    return Ok(parse_table_columns(&mut columns_reader, e.name())?
                        .into_iter()
                        .map(|c| TableColumnDefinition {
                            width: c.width,
                            style: c.style,
                            header_style: c.header_style,
                        })
                        .collect());
                }
                Ok(XmlEvent::Eof) => break, // No <columns> tag found
                Err(e) => return Err(e.into()),
                _ => (), // Keep searching
            }
        }
        Ok(Vec::new()) // It's valid to have a table without a <columns> definition.
    }

    pub(in crate::parser::xslt) fn handle_header_start(&mut self) -> Result<(), PipelineError> {
        if let Some(IRNode::Table { header, .. }) = self.node_stack.last_mut() {
            *header = Some(Box::new(TableHeader { rows: Vec::new() }));
            self.is_in_table_header = true;
        }
        Ok(())
    }

    pub(in crate::parser::xslt) fn handle_row_start(&mut self) -> Result<(), PipelineError> {
        self.row_column_index_stack.push(0);
        let new_row = TableRow { cells: Vec::new() };

        match self.node_stack.last_mut() {
            Some(IRNode::Table { header, body, .. }) => {
                if self.is_in_table_header {
                    if let Some(h) = header {
                        h.rows.push(new_row);
                    }
                } else {
                    body.rows.push(new_row);
                }
            }
            _ => {
                return Err(PipelineError::TemplateParseError(
                    "<row> found outside a <table>".to_string(),
                ));
            }
        }
        Ok(())
    }

    pub(in crate::parser::xslt) fn handle_cell_start(
        &mut self,
        attributes: &OwnedAttributes,
    ) -> Result<(), PipelineError> {
        let style_name = get_attr_owned_optional(attributes, b"style")?;
        let style_override = parse_fo_attributes_to_element_style(attributes)?;
        let _col_index = *self.row_column_index_stack.last().ok_or_else(|| {
            PipelineError::TemplateParseError("<cell> outside of <row>".into())
        })?;

        let new_cell = TableCell {
            style_name,
            style_override,
            children: Vec::new(),
        };

        match self.node_stack.last_mut() {
            Some(IRNode::Table { header, body, .. }) => {
                let row = if self.is_in_table_header {
                    header.as_mut().and_then(|h| h.rows.last_mut())
                } else {
                    body.rows.last_mut()
                };

                if let Some(r) = row {
                    r.cells.push(new_cell);
                } else {
                    return Err(PipelineError::TemplateParseError(
                        "<cell> found without a parent <row>".to_string(),
                    ));
                }
            }
            _ => {
                return Err(PipelineError::TemplateParseError(
                    "<cell> found outside a <table>".to_string(),
                ));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::idf::IRNode;
    use crate::parser::xslt::handlers::test_helpers::build_fragment;
    use crate::stylesheet::Dimension;
    use serde_json::json;

    #[test]
    fn test_full_table_parsing() {
        let xml = r#"
            <table>
                <columns>
                    <column width="50%" header-style="th"/>
                    <column width="50%" header-style="th"/>
                </columns>
                <header>
                    <row>
                        <cell><text>Name</text></cell>
                        <cell><text>Value</text></cell>
                    </row>
                </header>
                <tbody>
                    <row>
                        <cell style="td"><text>A</text></cell>
                        <cell style="td"><text>1</text></cell>
                    </row>
                </tbody>
            </table>
        "#;
        let nodes = build_fragment(xml, &json!({})).unwrap();

        assert_eq!(nodes.len(), 1);

        if let IRNode::Table {
            columns,
            header,
            body,
            ..
        } = &nodes[0]
        {
            // Check columns
            assert_eq!(columns.len(), 2);
            assert_eq!(columns[0].width, Some(Dimension::Percent(50.0)));
            assert_eq!(columns[0].header_style.as_deref(), Some("th"));

            // Check header
            assert!(header.is_some());
            let header = header.as_ref().unwrap();
            assert_eq!(header.rows.len(), 1);
            assert_eq!(header.rows[0].cells.len(), 2);

            // Check body
            assert_eq!(body.rows.len(), 1);
            let body_row = &body.rows[0];
            assert_eq!(body_row.cells.len(), 2);
            assert_eq!(body_row.cells[0].style_name.as_deref(), Some("td"));
        } else {
            panic!("Expected IRNode::Table");
        }
    }

    #[test]
    fn test_table_without_columns_or_header() {
        let xml = r#"
            <table>
                <tbody>
                    <row>
                        <cell><text>Cell Only</text></cell>
                    </row>
                </tbody>
            </table>
        "#;
        let nodes = build_fragment(xml, &json!({})).unwrap();
        assert_eq!(nodes.len(), 1);

        if let IRNode::Table {
            columns, header, ..
        } = &nodes[0]
        {
            assert!(columns.is_empty());
            assert!(header.is_none());
        } else {
            panic!("Expected IRNode::Table");
        }
    }
}