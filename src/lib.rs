pub mod stylesheet;
pub mod layout_engine;
pub mod pdf_renderer;

pub use stylesheet::*;
pub use layout_engine::*;
pub use pdf_renderer::*;

use serde_json::Value;
use handlebars::Handlebars;
use std::collections::HashMap;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum PipelineError {
    #[error("JSON parsing error: {0}")]
    JsonParseError(#[from] serde_json::Error),

    #[error("Template error: {0}")]
    TemplateError(String),

    #[error("PDF generation error: {0}")]
    PdfError(String),

    #[error("Stylesheet error: {0}")]
    StylesheetError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

pub struct JsonToPdfPipeline {
    stylesheet: Stylesheet,
    template_engine: Handlebars<'static>,
}

impl JsonToPdfPipeline {
    pub fn new(stylesheet: Stylesheet) -> Self {
        let mut template_engine = Handlebars::new();
        template_engine.set_strict_mode(true);

        JsonToPdfPipeline {
            stylesheet,
            template_engine,
        }
    }

    pub fn from_stylesheet_json(json: &str) -> Result<Self, PipelineError> {
        let stylesheet = Stylesheet::from_json(json).map_err(PipelineError::JsonParseError)?;
        Ok(Self::new(stylesheet))
    }

    pub fn generate_pdf(&self, json_data: &Value) -> Result<Vec<u8>, PipelineError> {
        let mut layout_engine = LayoutEngine::new(&self.stylesheet);
        self.process_sequences(&mut layout_engine, json_data)?;

        let pages = layout_engine.get_pages();
        let mut renderer = PdfRenderer::new("Generated Document");
        let pdf_bytes = renderer.render(pages, &self.stylesheet.page, &layout_engine)
            .map_err(|e| PipelineError::PdfError(format!("{:?}", e)))?;

        Ok(pdf_bytes)
    }

    pub fn generate_pdf_file(&self, json_data: &Value, output_path: &str) -> Result<(), PipelineError> {
        let pdf_bytes = self.generate_pdf(json_data)?;
        std::fs::write(output_path, pdf_bytes)?;
        Ok(())
    }

    fn process_sequences(&self, engine: &mut LayoutEngine, data: &Value) -> Result<(), PipelineError> {
        if self.stylesheet.page_sequences.is_empty() {
            return Err(PipelineError::StylesheetError("No page_sequences defined in stylesheet.".to_string()));
        }

        for (_seq_name, sequence) in &self.stylesheet.page_sequences {
            let template = self.stylesheet.templates.get(&sequence.template)
                .ok_or_else(|| PipelineError::TemplateError(format!("Template '{}' not found", sequence.template)))?;

            let data_items: Vec<Value> = if sequence.data_source == "/" {
                vec![data.clone()]
            } else {
                data.pointer(&sequence.data_source)
                    .and_then(|v| v.as_array())
                    .cloned()
                    .ok_or_else(|| PipelineError::TemplateError(format!("Data source '{}' not found or not an array", sequence.data_source)))?
            };

            for (index, item_data) in data_items.iter().enumerate() {
                if index > 0 {
                    engine.force_new_page();
                }
                let elements = self.parse_template_children(&template.children, item_data)?;
                engine.layout_elements(elements);
            }
        }
        Ok(())
    }

    fn parse_template_children(&self, children: &[TemplateElement], data: &Value) -> Result<Vec<LayoutElement>, PipelineError> {
        let mut elements = Vec::new();
        for template_element in children {
            match template_element {
                TemplateElement::Text { content, style } => {
                    let rendered_content = self.template_engine.render_template(content, data)
                        .map_err(|e| PipelineError::TemplateError(e.to_string()))?;
                    elements.push(LayoutElement::Text(TextElement {
                        style_name: style.clone(),
                        content: rendered_content,
                        lines: Vec::new(),
                    }));
                },
                TemplateElement::Container { children: container_children, style } => {
                    let parsed_children = self.parse_template_children(container_children, data)?;
                    elements.push(LayoutElement::Container(ContainerElement {
                        style_name: style.clone(),
                        children: parsed_children,
                    }));
                },
                TemplateElement::Rectangle { style } => {
                    elements.push(LayoutElement::Rectangle(RectElement {
                        style_name: style.clone()
                    }));
                },
                TemplateElement::PageBreak => {},
                TemplateElement::Table { data_source, columns, style } => {
                    elements.push(self.create_table_from_data(data_source, columns, style.clone(), data)?);
                },
                _ => {} // Other elements like List, Image, etc.
            }
        }
        Ok(elements)
    }

    fn create_table_from_data(&self, data_source: &str, columns: &[TableColumn], style: Option<String>, context_data: &Value) -> Result<LayoutElement, PipelineError> {
        let rows_data = context_data.pointer(data_source)
            .and_then(|v| v.as_array())
            .ok_or_else(|| PipelineError::TemplateError(format!("Table data source '{}' not found or is not an array in context", data_source)))?;

        let mut table_rows = Vec::new();

        // Header row
        let header_cells: Vec<TableCell> = columns.iter().map(|col| TableCell {
            content: Box::new(LayoutElement::Text(TextElement {
                style_name: col.header_style.clone(),
                content: col.header.clone(),
                lines: Vec::new()
            })),
            colspan: 1, rowspan: 1,
        }).collect();
        if !header_cells.is_empty() {
            table_rows.push(TableRow { cells: header_cells, height: 0.0, is_header: true });
        }

        // Data rows
        for row_item in rows_data {
            let mut cells = Vec::new();
            for col in columns {
                let cell_value = row_item.pointer(&col.data_field).unwrap_or(&Value::Null);

                let cell_text = match cell_value {
                    Value::String(s) => s.clone(),
                    Value::Number(n) => n.to_string(),
                    Value::Bool(b) => b.to_string(),
                    _ => "".to_string(),
                };

                let rendered_text = self.template_engine.render_template(&cell_text, row_item)
                    .unwrap_or(cell_text);

                cells.push(TableCell {
                    content: Box::new(LayoutElement::Text(TextElement {
                        style_name: col.style.clone(),
                        content: rendered_text,
                        lines: Vec::new()
                    })),
                    colspan: 1, rowspan: 1,
                });
            }
            table_rows.push(TableRow { cells, height: 0.0, is_header: false });
        }

        Ok(LayoutElement::Table(TableElement {
            style_name: style,
            rows: table_rows,
            column_widths: Vec::new(),
        }))
    }
}

pub struct PipelineBuilder {
    stylesheet: Option<Stylesheet>,
    templates: HashMap<String, String>,
}

impl PipelineBuilder {
    pub fn new() -> Self {
        PipelineBuilder {
            stylesheet: None,
            templates: HashMap::new(),
        }
    }

    pub fn with_stylesheet_json(mut self, json: &str) -> Result<Self, PipelineError> {
        self.stylesheet = Some(Stylesheet::from_json(json)?);
        Ok(self)
    }

    pub fn build(self) -> Result<JsonToPdfPipeline, PipelineError> {
        let stylesheet = self.stylesheet
            .ok_or_else(|| PipelineError::StylesheetError("No stylesheet provided".to_string()))?;
        let pipeline = JsonToPdfPipeline::new(stylesheet);
        Ok(pipeline)
    }
}