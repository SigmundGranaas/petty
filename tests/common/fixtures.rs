use serde_json::{json, Value};

/// Create a minimal valid JSON template
pub fn minimal_template() -> Value {
    json!({
        "_stylesheet": {
            "defaultPageMaster": "default",
            "pageMasters": {
                "default": {
                    "size": "A4",
                    "margins": "2cm"
                }
            },
            "styles": {}
        },
        "_template": {
            "type": "Block",
            "children": []
        }
    })
}

/// Create a template with specific page settings
pub fn template_with_page_settings(size: &str, margins: &str) -> Value {
    json!({
        "_stylesheet": {
            "defaultPageMaster": "default",
            "pageMasters": {
                "default": {
                    "size": size,
                    "margins": margins
                }
            },
            "styles": {}
        },
        "_template": {
            "type": "Block",
            "children": []
        }
    })
}

/// Create a template with custom styles and content
pub fn template_with_styles(styles: Value, content: Value) -> Value {
    json!({
        "_stylesheet": {
            "defaultPageMaster": "default",
            "pageMasters": {
                "default": {
                    "size": "A4",
                    "margins": "2cm"
                }
            },
            "styles": styles
        },
        "_template": content
    })
}

/// Create a simple paragraph with text
pub fn paragraph(text: &str) -> Value {
    json!({
        "type": "Paragraph",
        "children": [{ "type": "Text", "content": text }]
    })
}

/// Create a paragraph with style classes
pub fn styled_paragraph(text: &str, style_names: &[&str]) -> Value {
    json!({
        "type": "Paragraph",
        "styleNames": style_names,
        "children": [{ "type": "Text", "content": text }]
    })
}

/// Create a paragraph with inline style override
pub fn paragraph_with_style(text: &str, style: Value) -> Value {
    json!({
        "type": "Paragraph",
        "styleOverride": style,
        "children": [{ "type": "Text", "content": text }]
    })
}

/// Create a block container
pub fn block(children: Vec<Value>) -> Value {
    json!({
        "type": "Block",
        "children": children
    })
}

/// Create a styled block container
pub fn styled_block(style_names: &[&str], children: Vec<Value>) -> Value {
    json!({
        "type": "Block",
        "styleNames": style_names,
        "children": children
    })
}

/// Create a heading element
pub fn heading(level: u8, text: &str, id: Option<&str>) -> Value {
    let mut h = json!({
        "type": "Heading",
        "level": level,
        "children": [{ "type": "Text", "content": text }]
    });
    if let Some(id_val) = id {
        h["id"] = json!(id_val);
    }
    h
}

/// Create a page break
pub fn page_break() -> Value {
    json!({ "type": "PageBreak" })
}

/// Create a hyperlink element
pub fn hyperlink(href: &str, text: &str) -> Value {
    json!({
        "type": "Hyperlink",
        "href": href,
        "children": [{ "type": "Text", "content": text }]
    })
}

/// Create an image element
pub fn image(src: &str, width: &str, height: &str) -> Value {
    json!({
        "type": "Image",
        "src": src,
        "styleOverride": {
            "width": width,
            "height": height
        }
    })
}

/// Create a list element
pub fn list(items: Vec<Value>, list_style_type: Option<&str>) -> Value {
    let children: Vec<Value> = items
        .into_iter()
        .map(|item| {
            json!({
                "type": "ListItem",
                "children": [item]
            })
        })
        .collect();

    let mut list = json!({
        "type": "List",
        "children": children
    });

    if let Some(lst) = list_style_type {
        list["styleOverride"] = json!({ "listStyleType": lst });
    }
    list
}

/// Create a table cell (which is actually a Block)
pub fn table_cell(content: &str) -> Value {
    json!({
        "type": "Block",
        "children": [paragraph(content)]
    })
}

/// Create a styled table cell
pub fn styled_cell(content: &str, style_names: &[&str]) -> Value {
    json!({
        "type": "Block",
        "styleNames": style_names,
        "children": [paragraph(content)]
    })
}

/// Create a table element
pub fn table(columns: Vec<Value>, header: Option<Vec<Value>>, rows: Vec<Value>) -> Value {
    let mut tbl = json!({
        "type": "Table",
        "columns": columns,
        "body": { "rows": rows }
    });
    if let Some(h) = header {
        tbl["header"] = json!({ "rows": h });
    }
    tbl
}

/// Create a flex container
pub fn flex_container(children: Vec<Value>, style: Option<Value>) -> Value {
    let mut fc = json!({
        "type": "FlexContainer",
        "children": children
    });
    if let Some(s) = style {
        fc["styleOverride"] = s;
    }
    fc
}

/// Create a styled flex container using named styles
pub fn styled_flex_container(children: Vec<Value>, style_names: &[&str]) -> Value {
    json!({
        "type": "FlexContainer",
        "styleNames": style_names,
        "children": children
    })
}

/// Create a flex item (block with optional flex styling)
pub fn flex_item(text: &str, style: Option<Value>) -> Value {
    let mut item = json!({
        "type": "Block",
        "children": [paragraph(text)]
    });
    if let Some(s) = style {
        item["styleOverride"] = s;
    }
    item
}

/// Create a styled flex item using named styles
pub fn styled_flex_item(text: &str, style_names: &[&str]) -> Value {
    json!({
        "type": "Block",
        "styleNames": style_names,
        "children": [paragraph(text)]
    })
}

/// Create a table row (which is actually a Block containing cell Blocks)
pub fn table_row(cells: Vec<Value>) -> Value {
    json!({
        "type": "Block",
        "children": cells
    })
}
