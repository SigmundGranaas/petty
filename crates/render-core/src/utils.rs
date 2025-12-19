use petty_style::FontWeight;

/// Get PDF font name with style suffix
pub fn get_styled_font_name(base_name: &str, weight: FontWeight, italic: bool) -> String {
    let mut name = base_name.to_string();
    if weight == FontWeight::Bold || italic {
        name.push('-');
        if weight == FontWeight::Bold && italic {
            name.push_str("BoldItalic");
        } else if weight == FontWeight::Bold {
            name.push_str("Bold");
        } else {
            name.push_str("Italic");
        }
    }
    name
}

/// Convert layout Y coordinate to PDF Y coordinate (flip origin)
pub fn flip_y(y: f32, page_height: f32) -> f32 {
    page_height - y
}
