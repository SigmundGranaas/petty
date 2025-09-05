use crate::stylesheet::*;
use std::collections::HashMap;

pub struct LayoutEngine {
    pub page_layout: PageLayout,
    pub styles: HashMap<String, ElementStyle>,
    pub current_page: usize,
    pub current_y: f32,
    pub current_x: f32,
    pub pages: Vec<Page>,
}

#[derive(Debug)]
pub struct Page {
    pub number: usize,
    pub elements: Vec<PositionedElement>,
}

#[derive(Clone, Debug)]
pub struct PositionedElement {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub element: LayoutElement,
    pub style: ComputedStyle,
}

#[derive(Clone, Debug)]
pub enum LayoutElement {
    Text(TextElement),
    Image(ImageElement),
    Rectangle(RectElement),
    Table(TableElement),
    Container(ContainerElement),
}

#[derive(Clone, Debug)]
pub struct TextElement {
    pub style_name: Option<String>,
    pub content: String,
    pub lines: Vec<TextLine>,
}

#[derive(Clone, Debug)]
pub struct TextLine {
    pub text: String,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Clone, Debug)]
pub struct ImageElement {
    pub style_name: Option<String>,
    pub src: Vec<u8>,
    pub alt: String,
}

#[derive(Clone, Debug)]
pub struct RectElement {
    pub style_name: Option<String>,
}

#[derive(Clone, Debug)]
pub struct TableElement {
    pub style_name: Option<String>,
    pub rows: Vec<TableRow>,
    pub column_widths: Vec<f32>,
}

#[derive(Clone, Debug)]
pub struct TableRow {
    pub cells: Vec<TableCell>,
    pub height: f32,
    pub is_header: bool,
}

#[derive(Clone, Debug)]
pub struct TableCell {
    pub content: Box<LayoutElement>,
    pub colspan: u32,
    pub rowspan: u32,
}

#[derive(Clone, Debug)]
pub struct ContainerElement {
    pub style_name: Option<String>,
    pub children: Vec<LayoutElement>,
}

// Computed style after inheritance and cascading
#[derive(Clone, Debug)]
pub struct ComputedStyle {
    pub font_family: String,
    pub font_size: f32,
    pub font_weight: FontWeight,
    pub font_style: FontStyle,
    pub line_height: f32,
    pub text_align: TextAlign,
    pub color: Color,
    pub margin: Margins,
    pub padding: Margins,
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub background_color: Option<Color>,
    pub border: Option<Border>,
}

impl LayoutEngine {
    pub fn new(stylesheet: &Stylesheet) -> Self {
        LayoutEngine {
            page_layout: stylesheet.page.clone(),
            styles: stylesheet.styles.clone(),
            current_page: 0,
            current_y: stylesheet.page.margins.top,
            current_x: stylesheet.page.margins.left,
            pages: vec![Page::new(0)],
        }
    }

    pub fn layout_elements(&mut self, elements: Vec<LayoutElement>) {
        for element in elements {
            self.layout_element(element);
        }
    }

    pub fn get_pages(&self) -> &Vec<Page> {
        &self.pages
    }

    pub fn force_new_page(&mut self) {
        // Avoid creating a new page if the current one is pristine (nothing has been added).
        let should_create = self.pages.last().map_or(false, |p| {
            !p.elements.is_empty() || self.current_y > self.page_layout.margins.top
        });
        if should_create {
            self.new_page();
        }
    }

    fn layout_element(&mut self, element: LayoutElement) {
        let style_name = match &element {
            LayoutElement::Text(e) => e.style_name.as_ref(),
            LayoutElement::Image(e) => e.style_name.as_ref(),
            LayoutElement::Table(e) => e.style_name.as_ref(),
            LayoutElement::Container(e) => e.style_name.as_ref(),
            LayoutElement::Rectangle(e) => e.style_name.as_ref(),
        };

        let computed_style = self.compute_style(style_name.map(|s| s.as_str()));
        let available_width = self.get_available_width() - computed_style.margin.left - computed_style.margin.right;

        match element {
            LayoutElement::Text(text_elem) => {
                self.layout_text(text_elem, computed_style, available_width);
            },
            LayoutElement::Image(img_elem) => {
                self.layout_image(img_elem, computed_style, available_width);
            },
            LayoutElement::Table(mut table_elem) => {
                self.layout_table(&mut table_elem, computed_style, available_width);
            },
            LayoutElement::Container(container_elem) => {
                self.layout_container(container_elem, computed_style, available_width);
            },
            LayoutElement::Rectangle(rect_elem) => {
                self.layout_rectangle(rect_elem, computed_style, available_width);
            },
        }
    }

    fn layout_text(&mut self, text: TextElement, style: ComputedStyle, max_width: f32) {
        let content_width = max_width - style.padding.left - style.padding.right;
        let lines = self.wrap_text(&text.content, &style, content_width);
        let mut line_cursor = 0;

        while line_cursor < lines.len() {
            let page_height = self.get_page_height();
            let available_space = page_height - self.current_y - self.page_layout.margins.bottom;
            let required_space_for_first_line = style.margin.top + style.padding.top + style.line_height + style.padding.bottom + style.margin.bottom;

            if self.needs_page_break(required_space_for_first_line) {
                self.new_page();
                continue;
            }

            let space_for_lines = available_space - style.margin.top - style.padding.top - style.padding.bottom - style.margin.bottom;
            let lines_that_fit = ((space_for_lines + 0.001) / style.line_height).floor() as usize;

            let num_lines_to_draw = std::cmp::min(lines.len() - line_cursor, lines_that_fit.max(1));

            let chunk_of_lines = &lines[line_cursor..line_cursor + num_lines_to_draw];
            let text_block_height = chunk_of_lines.len() as f32 * style.line_height;
            let total_height = text_block_height + style.padding.top + style.padding.bottom;

            let x = self.current_x + style.margin.left;
            let y = self.current_y + style.margin.top;

            let positioned = PositionedElement {
                x,
                y,
                width: max_width,
                height: total_height,
                element: LayoutElement::Text(TextElement {
                    style_name: text.style_name.clone(),
                    content: chunk_of_lines.join("\n"),
                    lines: chunk_of_lines.iter().enumerate().map(|(i, line)| {
                        TextLine {
                            text: line.clone(),
                            x: x + style.padding.left,
                            y: y + style.padding.top + (i as f32 * style.line_height),
                            width: content_width,
                            height: style.line_height,
                        }
                    }).collect(),
                }),
                style: style.clone(),
            };

            self.add_element(positioned);
            self.current_y += total_height + style.margin.top + style.margin.bottom;
            line_cursor += num_lines_to_draw;

            if line_cursor < lines.len() {
                self.new_page();
            }
        }
    }

    fn layout_table(&mut self, table: &mut TableElement, style: ComputedStyle, max_width: f32) {
        table.column_widths = self.calculate_column_widths(table, max_width);

        let mut laid_out_rows = Vec::new();
        let mut total_height = 0.0;

        // First, calculate all row heights based on the content and style of their cells
        let row_heights: Vec<f32> = table.rows.iter().map(|row| {
            let mut max_cell_height_in_row = 0.0f32;
            for (i, cell) in row.cells.iter().enumerate() {
                if let LayoutElement::Text(text) = &*cell.content {
                    let cell_width = table.column_widths[i];
                    let cell_style_name = text.style_name.as_ref().map(|s| s.as_str());
                    let cell_style = self.compute_style(cell_style_name);

                    let lines = self.wrap_text(&text.content, &cell_style, cell_width - cell_style.padding.left - cell_style.padding.right);
                    let num_lines = lines.len().max(1) as f32;
                    let content_height = num_lines * cell_style.line_height;

                    let current_cell_total_height = content_height + cell_style.padding.top + cell_style.padding.bottom;
                    max_cell_height_in_row = max_cell_height_in_row.max(current_cell_total_height);
                }
            }
            max_cell_height_in_row.max(1.0) // Ensure a minimum height for empty rows
        }).collect();

        total_height = row_heights.iter().sum();

        // Check if the entire table fits on the current page, otherwise start a new one.
        // Note: This does not yet support splitting a table across pages.
        if self.needs_page_break(total_height + style.margin.top + style.margin.bottom) {
            self.new_page();
        }

        // Populate the final rows with their calculated heights
        for (idx, row) in table.rows.iter().enumerate() {
            laid_out_rows.push(TableRow {
                cells: row.cells.clone(),
                height: row_heights[idx],
                is_header: row.is_header,
            });
        }
        table.rows = laid_out_rows;

        let positioned = PositionedElement {
            x: self.current_x + style.margin.left,
            y: self.current_y + style.margin.top,
            width: max_width,
            height: total_height,
            element: LayoutElement::Table(table.clone()),
            style: style.clone(),
        };

        self.add_element(positioned);
        self.current_y += total_height + style.margin.top + style.margin.bottom;
    }


    fn layout_rectangle(&mut self, rect: RectElement, style: ComputedStyle, max_width: f32) {
        let width = style.width.unwrap_or(max_width);
        let height = style.height.unwrap_or(1.0); // Default to a 1pt high line

        if self.needs_page_break(height + style.margin.top + style.margin.bottom) {
            self.new_page();
        }

        let positioned = PositionedElement {
            x: self.current_x + style.margin.left,
            y: self.current_y + style.margin.top,
            width,
            height,
            element: LayoutElement::Rectangle(rect),
            style: style.clone(),
        };

        self.add_element(positioned);
        self.current_y += height + style.margin.top + style.margin.bottom;
    }

    fn layout_image(&mut self, image: ImageElement, style: ComputedStyle, max_width: f32) {
        let img_width = style.width.unwrap_or(max_width);
        let img_height = style.height.unwrap_or(100.0); // Default height

        if self.needs_page_break(img_height + style.margin.top + style.margin.bottom) {
            self.new_page();
        }

        let positioned = PositionedElement {
            x: self.current_x + style.margin.left,
            y: self.current_y + style.margin.top,
            width: img_width,
            height: img_height,
            element: LayoutElement::Image(image.clone()),
            style: style.clone(),
        };

        self.add_element(positioned);
        self.current_y += img_height + style.margin.top + style.margin.bottom;
    }

    fn layout_container(&mut self, container: ContainerElement, style: ComputedStyle, max_width: f32) {
        let start_y = self.current_y;

        let container_height = style.height.unwrap_or(0.0);
        let total_height = container_height + style.padding.top + style.padding.bottom;

        if self.needs_page_break(total_height + style.margin.top + style.margin.bottom) {
            self.new_page();
        }

        let positioned_container = PositionedElement {
            x: self.current_x + style.margin.left,
            y: self.current_y + style.margin.top,
            width: max_width,
            height: 0.0, // We'll update this later
            element: LayoutElement::Container(ContainerElement { style_name: container.style_name.clone(), children: vec![] }),
            style: style.clone(),
        };

        // Temporarily add container to get its index
        self.add_element(positioned_container);
        let container_index = self.pages.last().unwrap().elements.len() - 1;

        let saved_x = self.current_x;
        let saved_y = self.current_y;

        self.current_x += style.margin.left + style.padding.left;
        self.current_y += style.margin.top + style.padding.top;

        // We're now inside the container's padding box
        for child in container.children {
            self.layout_element(child);
        }

        // Restore coordinates to outside the container
        self.current_x = saved_x;
        let final_y = self.current_y;
        self.current_y = saved_y;

        let inner_height = final_y - (start_y + style.margin.top + style.padding.top);
        let final_height = inner_height + style.padding.top + style.padding.bottom;

        // Update the container's height now that we know it
        self.pages.last_mut().unwrap().elements[container_index].height = final_height;

        // Move the main cursor past the container
        self.current_y += final_height + style.margin.top + style.margin.bottom;
    }

    // MODIFIED: Made public to allow access from pdf_renderer for cell content wrapping.
    pub fn wrap_text(&self, text: &str, style: &ComputedStyle, max_width: f32) -> Vec<String> {
        if max_width <= 0.0 { return text.lines().map(|s| s.to_string()).collect(); }
        let mut lines = Vec::new();
        for paragraph in text.lines() {
            if paragraph.trim().is_empty() {
                lines.push("".to_string());
                continue;
            }
            let words = paragraph.split_whitespace();
            let mut current_line = String::new();
            let char_width = style.font_size * 0.6; // Rough approximation

            for word in words {
                let test_line = if current_line.is_empty() {
                    word.to_string()
                } else {
                    format!("{} {}", current_line, word)
                };

                let line_width = test_line.len() as f32 * char_width;

                if line_width > max_width && !current_line.is_empty() {
                    lines.push(current_line);
                    current_line = String::from(word);
                } else {
                    current_line = test_line;
                }
            }

            if !current_line.is_empty() {
                lines.push(current_line);
            }
        }
        lines
    }

    fn calculate_column_widths(&self, table: &TableElement, max_width: f32) -> Vec<f32> {
        let num_columns = table.rows.first().map(|row| row.cells.len()).unwrap_or(0);
        if num_columns == 0 { return vec![]; }
        vec![max_width / num_columns as f32; num_columns]
    }

    fn needs_page_break(&self, required_height: f32) -> bool {
        let page_height = self.get_page_height();
        let available = page_height - self.current_y - self.page_layout.margins.bottom;
        available < required_height
    }

    fn new_page(&mut self) {
        self.current_page += 1;
        self.pages.push(Page::new(self.current_page));
        self.current_y = self.page_layout.margins.top;
        self.current_x = self.page_layout.margins.left;
    }

    fn add_element(&mut self, element: PositionedElement) {
        if let Some(page) = self.pages.last_mut() {
            page.elements.push(element);
        }
    }

    fn get_available_width(&self) -> f32 {
        let page_width = self.get_page_width();
        page_width - self.page_layout.margins.left - self.page_layout.margins.right
    }

    fn get_page_width(&self) -> f32 {
        match self.page_layout.size {
            PageSize::A4 => 595.0,
            PageSize::Letter => 612.0,
            PageSize::Legal => 612.0,
            PageSize::Custom { width, .. } => width,
        }
    }

    fn get_page_height(&self) -> f32 {
        match self.page_layout.size {
            PageSize::A4 => 842.0,
            PageSize::Letter => 792.0,
            PageSize::Legal => 1008.0,
            PageSize::Custom { height, .. } => height,
        }
    }

    // FIX: Changed to pub fn
    pub fn compute_style(&self, style_name: Option<&str>) -> ComputedStyle {
        let mut computed = ComputedStyle {
            font_family: "Helvetica".to_string(),
            font_size: 12.0,
            font_weight: FontWeight::Regular,
            font_style: FontStyle::Normal,
            line_height: 14.4,
            text_align: TextAlign::Left,
            color: Color { r: 0, g: 0, b: 0, a: 1.0 },
            margin: Margins { top: 0.0, right: 0.0, bottom: 10.0, left: 0.0 },
            padding: Margins { top: 2.0, right: 2.0, bottom: 2.0, left: 2.0 },
            width: None,
            height: None,
            background_color: None,
            border: None,
        };

        if let Some(name) = style_name {
            if let Some(style_def) = self.styles.get(name) {
                if let Some(ff) = &style_def.font_family { computed.font_family = ff.clone(); }
                if let Some(fs) = style_def.font_size {
                    computed.font_size = fs;
                    // Auto-update line height if not specified
                    if style_def.line_height.is_none() {
                        computed.line_height = fs * 1.2;
                    }
                }
                if let Some(fw) = &style_def.font_weight { computed.font_weight = fw.clone(); }
                if let Some(fs) = &style_def.font_style { computed.font_style = fs.clone(); }
                if let Some(lh) = style_def.line_height { computed.line_height = lh; }
                if let Some(ta) = &style_def.text_align { computed.text_align = ta.clone(); }
                if let Some(c) = &style_def.color { computed.color = c.clone(); }
                if let Some(m) = &style_def.margin { computed.margin = m.clone(); }
                if let Some(p) = &style_def.padding { computed.padding = p.clone(); }
                if let Some(Dimension::Pt(w)) = style_def.width { computed.width = Some(w); }
                if let Some(Dimension::Pt(h)) = style_def.height { computed.height = Some(h); }
                if let Some(bg) = &style_def.background_color { computed.background_color = Some(bg.clone()); }
                if let Some(b) = &style_def.border { computed.border = Some(b.clone()); }
            }
        }
        computed
    }
}

impl Page {
    pub fn new(number: usize) -> Self {
        Page { number, elements: Vec::new() }
    }
}