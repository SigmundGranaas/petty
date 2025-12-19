//! Custom deserializers for ElementStyle that support CSS-like string values
//!
//! This module allows JSON templates to use either:
//! - Numeric values: `"fontSize": 24`
//! - String dimensions: `"fontSize": "24pt"`
//! - Kebab-case or camelCase field names

use petty_style::stylesheet::ElementStyle;
use petty_style::{FontWeight, TextAlign, Border, BorderStyle};
use petty_style::dimension::Margins;
use petty_types::Color;
use serde::{Deserialize, Deserializer};
use serde_json::Value;
use std::collections::HashMap;

/// Parse a dimension value that can be either a number or a string like "24pt"
fn parse_dimension(value: &Value) -> Option<f32> {
    match value {
        Value::Number(n) => n.as_f64().map(|f| f as f32),
        Value::String(s) => {
            // Parse strings like "24pt", "1.5cm", etc.
            let s = s.trim();

            // Try to extract the numeric part and unit
            let numeric_part = s.chars()
                .take_while(|c| c.is_numeric() || *c == '.' || *c == '-')
                .collect::<String>();

            if let Ok(value) = numeric_part.parse::<f32>() {
                // For now, assume all units are in points
                // Future: properly convert cm, mm, in, etc. to points
                Some(value)
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Deserialize a HashMap<String, ElementStyle> with flexible value parsing
pub fn deserialize_styles<'de, D>(deserializer: D) -> Result<HashMap<String, ElementStyle>, D::Error>
where
    D: Deserializer<'de>,
{
    let raw_styles: HashMap<String, Value> = HashMap::deserialize(deserializer)?;
    let mut styles = HashMap::new();

    for (name, style_value) in raw_styles {
        let style = parse_element_style(&style_value)
            .map_err(|e| serde::de::Error::custom(format!("Failed to parse style '{}': {}", name, e)))?;
        styles.insert(name, style);
    }

    Ok(styles)
}

/// Parse a single ElementStyle from a JSON value with flexible field parsing
fn parse_element_style(value: &Value) -> Result<ElementStyle, String> {
    let obj = value.as_object()
        .ok_or_else(|| "Style must be an object".to_string())?;

    let mut style = ElementStyle::default();

    for (key, val) in obj {
        // Handle both camelCase and kebab-case
        let normalized_key = key.replace("-", "_").to_lowercase();

        match normalized_key.as_str() {
            "font_family" | "fontfamily" => {
                if let Some(s) = val.as_str() {
                    style.font_family = Some(s.to_string());
                }
            }
            "font_size" | "fontsize" => {
                style.font_size = parse_dimension(val);
            }
            "font_weight" | "fontweight" => {
                if let Some(s) = val.as_str() {
                    style.font_weight = match s.to_lowercase().as_str() {
                        "bold" => Some(FontWeight::Bold),
                        "normal" | "regular" => Some(FontWeight::Regular),
                        "light" => Some(FontWeight::Light),
                        "thin" => Some(FontWeight::Thin),
                        "medium" => Some(FontWeight::Medium),
                        "black" => Some(FontWeight::Black),
                        _ => None,
                    };
                }
            }
            "line_height" | "lineheight" => {
                style.line_height = parse_dimension(val);
            }
            "color" => {
                if let Some(s) = val.as_str()
                    && let Ok(color) = parse_color(s) {
                        style.color = Some(color);
                    }
            }
            "text_align" | "textalign" => {
                if let Some(s) = val.as_str() {
                    style.text_align = match s.to_lowercase().as_str() {
                        "left" => Some(TextAlign::Left),
                        "right" => Some(TextAlign::Right),
                        "center" => Some(TextAlign::Center),
                        "justify" => Some(TextAlign::Justify),
                        _ => None,
                    };
                }
            }
            "margin_top" | "margintop" => {
                if let Some(val) = parse_dimension(val) {
                    let margins = style.margin.get_or_insert(Margins::default());
                    margins.top = val;
                }
            }
            "margin_bottom" | "marginbottom" => {
                if let Some(val) = parse_dimension(val) {
                    let margins = style.margin.get_or_insert(Margins::default());
                    margins.bottom = val;
                }
            }
            "margin_left" | "marginleft" => {
                if let Some(val) = parse_dimension(val) {
                    let margins = style.margin.get_or_insert(Margins::default());
                    margins.left = val;
                }
            }
            "margin_right" | "marginright" => {
                if let Some(val) = parse_dimension(val) {
                    let margins = style.margin.get_or_insert(Margins::default());
                    margins.right = val;
                }
            }
            "padding_top" | "paddingtop" => {
                if let Some(val) = parse_dimension(val) {
                    let padding = style.padding.get_or_insert(Margins::default());
                    padding.top = val;
                }
            }
            "padding_bottom" | "paddingbottom" => {
                if let Some(val) = parse_dimension(val) {
                    let padding = style.padding.get_or_insert(Margins::default());
                    padding.bottom = val;
                }
            }
            "padding_left" | "paddingleft" => {
                if let Some(val) = parse_dimension(val) {
                    let padding = style.padding.get_or_insert(Margins::default());
                    padding.left = val;
                }
            }
            "padding_right" | "paddingright" => {
                if let Some(val) = parse_dimension(val) {
                    let padding = style.padding.get_or_insert(Margins::default());
                    padding.right = val;
                }
            }
            "padding" => {
                // Parse "4pt 5pt" style padding or single value
                if let Some(padding_val) = parse_dimension(val) {
                    style.padding = Some(Margins {
                        top: padding_val,
                        right: padding_val,
                        bottom: padding_val,
                        left: padding_val,
                    });
                }
            }
            "background_color" | "backgroundcolor" => {
                if let Some(s) = val.as_str()
                    && let Ok(color) = parse_color(s) {
                        style.background_color = Some(color);
                    }
            }
            "border_top" | "bordertop" | "border" => {
                // Parse "1pt solid #cccccc" style borders
                if let Some(s) = val.as_str()
                    && let Ok(border) = parse_border(s) {
                        if normalized_key.contains("top") || normalized_key == "border" {
                            style.border_top = Some(border.clone());
                        }
                        if normalized_key == "border" {
                            style.border_bottom = Some(border.clone());
                            style.border_left = Some(border.clone());
                            style.border_right = Some(border);
                        }
                    }
            }
            "border_bottom" | "borderbottom" => {
                if let Some(s) = val.as_str()
                    && let Ok(border) = parse_border(s) {
                        style.border_bottom = Some(border);
                    }
            }
            _ => {
                // Ignore unknown fields for forward compatibility
            }
        }
    }

    Ok(style)
}

/// Parse a color string like "#2a4d69" or "red"
fn parse_color(s: &str) -> Result<Color, String> {
    let s = s.trim();

    if let Some(hex) = s.strip_prefix('#') {
        // Parse hex color #RRGGBB or #RGB
        let (r, g, b) = if hex.len() == 6 {
            (
                u8::from_str_radix(&hex[0..2], 16),
                u8::from_str_radix(&hex[2..4], 16),
                u8::from_str_radix(&hex[4..6], 16),
            )
        } else if hex.len() == 3 {
            (
                u8::from_str_radix(&hex[0..1].repeat(2), 16),
                u8::from_str_radix(&hex[1..2].repeat(2), 16),
                u8::from_str_radix(&hex[2..3].repeat(2), 16),
            )
        } else {
            return Err(format!("Invalid hex color: {}", s));
        };

        match (r, g, b) {
            (Ok(r), Ok(g), Ok(b)) => Ok(Color { r, g, b, a: 1.0 }),
            _ => Err(format!("Invalid hex color: {}", s)),
        }
    } else {
        // Named colors
        match s.to_lowercase().as_str() {
            "black" => Ok(Color { r: 0, g: 0, b: 0, a: 1.0 }),
            "white" => Ok(Color { r: 255, g: 255, b: 255, a: 1.0 }),
            "red" => Ok(Color { r: 255, g: 0, b: 0, a: 1.0 }),
            "green" => Ok(Color { r: 0, g: 128, b: 0, a: 1.0 }),
            "blue" => Ok(Color { r: 0, g: 0, b: 255, a: 1.0 }),
            _ => Err(format!("Unknown color name: {}", s)),
        }
    }
}

/// Parse a border string like "1pt solid #cccccc"
fn parse_border(s: &str) -> Result<Border, String> {
    let parts: Vec<&str> = s.split_whitespace().collect();

    if parts.is_empty() {
        return Err("Empty border specification".to_string());
    }

    let mut width = 1.0;
    let mut style = BorderStyle::Solid;
    let mut color = Color { r: 0, g: 0, b: 0, a: 1.0 };

    for part in parts {
        // Try to parse as dimension (width)
        if let Some(w) = parse_dimension(&Value::String(part.to_string())) {
            width = w;
        }
        // Try to parse as border style
        else if part.eq_ignore_ascii_case("solid") {
            style = BorderStyle::Solid;
        } else if part.eq_ignore_ascii_case("dashed") {
            style = BorderStyle::Dashed;
        } else if part.eq_ignore_ascii_case("dotted") {
            style = BorderStyle::Dotted;
        } else if part.eq_ignore_ascii_case("double") {
            style = BorderStyle::Double;
        }
        // Try to parse as color
        else if let Ok(c) = parse_color(part) {
            color = c;
        }
    }

    Ok(Border { width, style, color })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_parse_dimension_from_number() {
        let val = json!(24);
        assert_eq!(parse_dimension(&val), Some(24.0));
    }

    #[test]
    fn test_parse_dimension_from_string() {
        let val = json!("24pt");
        assert_eq!(parse_dimension(&val), Some(24.0));

        let val = json!("1.5cm");
        assert_eq!(parse_dimension(&val), Some(1.5));

        let val = json!("10");
        assert_eq!(parse_dimension(&val), Some(10.0));
    }

    #[test]
    fn test_parse_color_hex() {
        // 6-digit hex
        let color = parse_color("#2a4d69").unwrap();
        assert_eq!(color.r, 0x2a);
        assert_eq!(color.g, 0x4d);
        assert_eq!(color.b, 0x69);
        assert_eq!(color.a, 1.0);

        // 3-digit hex
        let color = parse_color("#abc").unwrap();
        assert_eq!(color.r, 0xaa);
        assert_eq!(color.g, 0xbb);
        assert_eq!(color.b, 0xcc);
    }

    #[test]
    fn test_parse_color_named() {
        let color = parse_color("black").unwrap();
        assert_eq!(color.r, 0);
        assert_eq!(color.g, 0);
        assert_eq!(color.b, 0);

        let color = parse_color("white").unwrap();
        assert_eq!(color.r, 255);
        assert_eq!(color.g, 255);
        assert_eq!(color.b, 255);

        let color = parse_color("red").unwrap();
        assert_eq!(color.r, 255);
        assert_eq!(color.g, 0);
        assert_eq!(color.b, 0);
    }

    #[test]
    fn test_parse_border() {
        let border = parse_border("1pt solid #000000").unwrap();
        assert_eq!(border.width, 1.0);
        assert_eq!(border.style, BorderStyle::Solid);
        assert_eq!(border.color.r, 0);
        assert_eq!(border.color.g, 0);
        assert_eq!(border.color.b, 0);

        let border = parse_border("2pt dashed red").unwrap();
        assert_eq!(border.width, 2.0);
        assert_eq!(border.style, BorderStyle::Dashed);
        assert_eq!(border.color.r, 255);
    }

    #[test]
    fn test_parse_element_style() {
        let style_json = json!({
            "fontSize": "24pt",
            "color": "#2a4d69",
            "lineHeight": "18pt",
            "padding": "10pt",
            "marginTop": "5pt"
        });

        let style = parse_element_style(&style_json).unwrap();
        assert_eq!(style.font_size, Some(24.0));
        assert_eq!(style.line_height, Some(18.0));
        assert!(style.color.is_some());
        assert_eq!(style.color.unwrap().r, 0x2a);
        assert!(style.padding.is_some());
        assert_eq!(style.padding.unwrap().top, 10.0);
        assert!(style.margin.is_some());
        assert_eq!(style.margin.unwrap().top, 5.0);
    }

    #[test]
    fn test_deserialize_styles() {
        let styles_json = json!({
            "header": {
                "fontSize": "24pt",
                "color": "#2a4d69",
                "fontWeight": "bold"
            },
            "body": {
                "fontSize": "12pt",
                "lineHeight": "18pt",
                "padding": "10pt"
            }
        });

        let json_string = styles_json.to_string();
        let mut deserializer = serde_json::Deserializer::from_str(&json_string);
        let styles = deserialize_styles(&mut deserializer).unwrap();

        assert_eq!(styles.len(), 2);
        assert!(styles.contains_key("header"));
        assert!(styles.contains_key("body"));

        let header = &styles["header"];
        assert_eq!(header.font_size, Some(24.0));
        assert_eq!(header.font_weight, Some(FontWeight::Bold));

        let body = &styles["body"];
        assert_eq!(body.font_size, Some(12.0));
        assert_eq!(body.line_height, Some(18.0));
    }
}
