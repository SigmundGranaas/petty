// FILE: src/templating/tests.rs

#![cfg(test)]

use super::builders::*;
use super::{p, Template, TemplateBuilder};
use crate::core::style::color::Color;
use crate::core::style::dimension::{Dimension, Margins, PageSize};
use crate::core::style::font::FontWeight;
use crate::core::style::stylesheet::{ElementStyle, PageLayout};
use crate::parser::json::ast::TemplateNode;

// --- New Reusable Component Pattern ---
/// A reusable component implemented as a struct.
/// This is analogous to a StatelessWidget in Flutter.
#[derive(Clone)]
struct UserBadge {
    name_path: String,
    is_premium_path: String,
}

impl UserBadge {
    fn new(name_path: &str, is_premium_path: &str) -> Self {
        Self {
            name_path: name_path.to_string(),
            is_premium_path: is_premium_path.to_string(),
        }
    }
}

/// By implementing TemplateBuilder, UserBadge can be used anywhere
/// a built-in builder (like Block, Paragraph, etc.) can be used.
impl TemplateBuilder for UserBadge {
    fn build(self: Box<Self>) -> TemplateNode {
        let component = Flex::new()
            .style_name("badge")
            // FIX: Explicitly convert &String to &str using .as_str()
            .child(Paragraph::new(self.name_path.as_str()))
            .child(
                If::new(
                    &self.is_premium_path,
                    Paragraph::empty()
                        .child(Span::new().text("Premium").style_name("premium-text")),
                )
                    .with_else(Paragraph::new("Standard")),
            );

        // The component's build logic returns another builder, so we must
        // call .build() on it to get the final TemplateNode.
        Box::new(component).build()
    }
}

#[test]
fn test_inline_styling() {
    let template = Template::new(Block::new().padding(Margins::all(20.0)).child(
        p("Hello Inline Styles")
            .font_size(24.0)
            .font_weight(FontWeight::Bold)
            .color(Color::gray(50)),
    ));

    let json_string = template.to_json().unwrap();
    let produced_value: serde_json::Value = serde_json::from_str(&json_string).unwrap();

    let expected_value = serde_json::json!({
      "_stylesheet": {},
      "_template": {
        "type": "Block",
        "styleOverride": {
          "padding": { "top": 20.0, "right": 20.0, "bottom": 20.0, "left": 20.0 }
        },
        "children": [
          {
            "type": "Paragraph",
            "styleOverride": {
              "fontSize": 24.0,
              "fontWeight": "Bold",
              "color": { "r": 50, "g": 50, "b": 50 }
            },
            "children": [
              { "type": "Text", "content": "Hello Inline Styles" }
            ]
          }
        ]
      }
    });

    assert_eq!(produced_value, expected_value);
}

#[test]
fn test_full_template_with_control_flow() {
    let template = Template::new(
        Block::new()
            .child(UserBadge::new("{{customer.name}}", "customer.is_premium"))
            .child(Each::new(
                "products",
                Flex::new().child(Paragraph::new("Product: {{this.name}}")),
            )),
    )
        .add_style(
            "badge",
            ElementStyle {
                padding: Some(Margins::all(10.0)),
                ..Default::default()
            },
        )
        .add_style(
            "premium-text",
            ElementStyle {
                color: Some(Color {
                    r: 212,
                    g: 175,
                    b: 55,
                    a: 1.0,
                }),
                ..Default::default()
            },
        );

    let json_string = template.to_json().unwrap();
    let produced_value: serde_json::Value = serde_json::from_str(&json_string).unwrap();

    let expected_value = serde_json::json!({
      "_stylesheet": {
        "styles": {
          "badge": { "padding": { "top": 10.0, "right": 10.0, "bottom": 10.0, "left": 10.0 } },
          "premium-text": { "color": { "r": 212, "g": 175, "b": 55 } }
        }
      },
      "_template": {
        "type": "Block",
        "children": [
          {
            "type": "FlexContainer",
            "styleNames": ["badge"],
            "children": [
              {
                "type": "Paragraph",
                "children": [{ "type": "Text", "content": "{{customer.name}}" }]
              },
              {
                "if": "customer.is_premium",
                "then": {
                  "type": "Paragraph",
                  "children": [
                    {
                      "type": "StyledSpan",
                      "styleNames": ["premium-text"],
                      "children": [{ "type": "Text", "content": "Premium" }]
                    }
                  ]
                },
                "else": {
                  "type": "Paragraph",
                  "children": [{ "type": "Text", "content": "Standard" }]
                }
              }
            ]
          },
          {
            "each": "products",
            "template": {
              "type": "FlexContainer",
              "children": [
                {
                  "type": "Paragraph",
                  "children": [{ "type": "Text", "content": "Product: {{this.name}}" }]
                }
              ]
            }
          }
        ]
      }
    });

    assert_eq!(produced_value, expected_value);
}

#[test]
fn test_template_with_definitions_and_roles() {
    let template = Template::new(
        Block::new()
            .child(Paragraph::new("Rendering user badge:"))
            .child(Render::new("user_badge_def")), // Use the Render builder
    )
        .add_style(
            "badge",
            ElementStyle {
                padding: Some(Margins::all(5.0)),
                ..Default::default()
            },
        )
        .add_style(
            "premium-text",
            ElementStyle {
                color: Some(Color {
                    r: 212,
                    g: 175,
                    b: 55,
                    a: 1.0,
                }),
                ..Default::default()
            },
        )
        // Add a reusable definition
        .add_definition(
            "user_badge_def",
            // The definition can be any builder, including our custom component
            UserBadge::new("{{user.name}}", "user.is_admin"),
        )
        .add_role("page-header", Paragraph::new("This is a header"));

    let json_string = template.to_json().unwrap();
    let produced_value: serde_json::Value = serde_json::from_str(&json_string).unwrap();

    let expected_value = serde_json::json!({
      "_stylesheet": {
        "styles": {
          "badge": { "padding": { "top": 5.0, "right": 5.0, "bottom": 5.0, "left": 5.0 } },
          "premium-text": { "color": { "r": 212, "g": 175, "b": 55 } }
        },
        "definitions": {
            "user_badge_def": {
                "type": "FlexContainer",
                "styleNames": ["badge"],
                "children": [
                  { "type": "Paragraph", "children": [{ "type": "Text", "content": "{{user.name}}" }] },
                  {
                    "if": "user.is_admin",
                    "then": {
                      "type": "Paragraph",
                      "children": [ { "type": "StyledSpan", "styleNames": ["premium-text"], "children": [{ "type": "Text", "content": "Premium" }] } ]
                    },
                    "else": { "type": "Paragraph", "children": [{ "type": "Text", "content": "Standard" }] }
                  }
                ]
            }
        }
      },
      "_template": {
        "type": "Block",
        "children": [
            { "type": "Paragraph", "children": [{ "type": "Text", "content": "Rendering user badge:" }] },
            { "type": "RenderTemplate", "name": "user_badge_def" }
        ]
      },
      "_roles": {
        "page-header": {
          "type": "Paragraph",
          "children": [{ "type": "Text", "content": "This is a header" }]
        }
      }
    });

    assert_eq!(produced_value, expected_value);
}

#[test]
fn test_template_builder_serialization() {
    let mut template = Template::new(
        Block::new()
            .child(Paragraph::new("Invoice #12345").style_name("title"))
            .child(PageBreak::new().master_name("landscape"))
            .child(
                Table::new()
                    .column(Column::new().width(Dimension::Percent(50.0)))
                    .column(Column::new().width(Dimension::Percent(50.0)))
                    .header_row(
                        Row::new()
                            .cell(Cell::new().child(Paragraph::new("Item")))
                            .cell(Cell::new().child(Paragraph::new("Price"))),
                    )
                    .body_row(
                        Row::new()
                            .cell(Cell::new().child(Paragraph::new("Anvil")))
                            .cell(Cell::new().child(Paragraph::new("100.00"))),
                    ),
            )
            .child(List::new().item(
                ListItem::new()
                    .child(Paragraph::new("Note 1"))
                    .child(Paragraph::new("Note 2")),
            )),
    );

    template = template
        .add_style(
            "title",
            ElementStyle {
                font_size: Some(24.0),
                font_weight: Some(FontWeight::Bold),
                ..Default::default()
            },
        )
        .add_style(
            "body",
            ElementStyle {
                font_family: Some("Helvetica".to_string()),
                ..Default::default()
            },
        )
        .add_page_master(
            "default",
            PageLayout {
                size: PageSize::A4,
                margins: Some(Margins::all(20.0)),
                ..Default::default()
            },
        )
        .add_page_master(
            "landscape",
            PageLayout {
                size: PageSize::Custom {
                    width: 842.0,
                    height: 595.0,
                },
                ..Default::default()
            },
        );

    let json_string = template.to_json().unwrap();
    let produced_value: serde_json::Value = serde_json::from_str(&json_string).unwrap();

    let expected_value = serde_json::json!({
      "_stylesheet": {
        "pageMasters": {
          "default": {
            "size": "A4",
            "margins": { "top": 20.0, "right": 20.0, "bottom": 20.0, "left": 20.0 }
          },
          "landscape": {
            "size": { "width": 842.0, "height": 595.0 }
          }
        },
        "styles": {
          "title": { "fontSize": 24.0, "fontWeight": "Bold" },
          "body": { "fontFamily": "Helvetica" }
        }
      },
      "_template": {
        "type": "Block",
        "children": [
          {
            "type": "Paragraph",
            "styleNames": ["title"],
            "children": [{ "type": "Text", "content": "Invoice #12345" }]
          },
          { "type": "PageBreak", "masterName": "landscape" },
          {
            "type": "Table",
            "columns": [
              { "width": { "percent": 50.0 } },
              { "width": { "percent": 50.0 } }
            ],
            "header": {
              "rows": [
                {
                  "type": "Block",
                  "children": [
                    { "type": "Block", "children": [{ "type": "Paragraph", "children": [{ "type": "Text", "content": "Item" }] }] },
                    { "type": "Block", "children": [{ "type": "Paragraph", "children": [{ "type": "Text", "content": "Price" }] }] }
                  ]
                }
              ]
            },
            "body": {
              "rows": [
                {
                  "type": "Block",
                  "children": [
                    { "type": "Block", "children": [{ "type": "Paragraph", "children": [{ "type": "Text", "content": "Anvil" }] }] },
                    { "type": "Block", "children": [{ "type": "Paragraph", "children": [{ "type": "Text", "content": "100.00" }] }] }
                  ]
                }
              ]
            }
          },
          {
            "type": "List",
            "children": [
              {
                "type": "ListItem",
                "children": [
                  { "type": "Paragraph", "children": [{ "type": "Text", "content": "Note 1" }] },
                  { "type": "Paragraph", "children": [{ "type": "Text", "content": "Note 2" }] }
                ]
              }
            ]
          }
        ]
      }
    });

    assert_eq!(produced_value, expected_value);
}