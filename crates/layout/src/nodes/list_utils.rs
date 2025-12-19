// src/core/layout/nodes/list_utils.rs
use crate::style::ComputedStyle;
use petty_style::list::ListStyleType;
use std::sync::Arc;

pub fn get_marker_text(style: &Arc<ComputedStyle>, index: usize, depth: usize) -> String {
    let list_type_to_use = if depth > 0 && style.list.style_type == ListStyleType::Decimal {
        match depth % 3 {
            1 => &ListStyleType::LowerAlpha,
            2 => &ListStyleType::LowerRoman,
            _ => &ListStyleType::Decimal,
        }
    } else {
        &style.list.style_type
    };

    match list_type_to_use {
        ListStyleType::Disc => "•".to_string(),
        ListStyleType::Circle => "◦".to_string(),
        ListStyleType::Square => "▪".to_string(),
        ListStyleType::Decimal => format!("{}.", index),
        ListStyleType::LowerAlpha => format!("{}.", int_to_lower_alpha(index)),
        ListStyleType::UpperAlpha => format!("{}.", int_to_upper_alpha(index)),
        ListStyleType::LowerRoman => format!("{}.", int_to_lower_roman(index)),
        ListStyleType::UpperRoman => format!("{}.", int_to_upper_roman(index)),
        ListStyleType::None => String::new(),
    }
}

pub fn int_to_lower_alpha(n: usize) -> String {
    if n == 0 {
        return "a".to_string();
    }
    let mut s = String::new();
    let mut num = n - 1;
    loop {
        s.insert(0, (b'a' + (num % 26) as u8) as char);
        num /= 26;
        if num == 0 {
            break;
        }
        num -= 1;
    }
    s
}

pub fn int_to_upper_alpha(n: usize) -> String {
    int_to_lower_alpha(n).to_uppercase()
}

pub fn int_to_lower_roman(n: usize) -> String {
    if n == 0 {
        return String::new();
    }
    let mut num = n;
    let values = [
        (1000, "m"),
        (900, "cm"),
        (500, "d"),
        (400, "cd"),
        (100, "c"),
        (90, "xc"),
        (50, "l"),
        (40, "xl"),
        (10, "x"),
        (9, "ix"),
        (5, "v"),
        (4, "iv"),
        (1, "i"),
    ];
    let mut result = String::new();
    for &(val, sym) in &values {
        while num >= val {
            result.push_str(sym);
            num -= val;
        }
    }
    result
}

pub fn int_to_upper_roman(n: usize) -> String {
    int_to_lower_roman(n).to_uppercase()
}
