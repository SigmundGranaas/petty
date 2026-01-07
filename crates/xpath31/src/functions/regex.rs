use crate::error::XPath31Error;
use crate::types::*;
use regex::{Captures, Regex};

/// fn:analyze-string($input as xs:string?, $pattern as xs:string) as element(fn:analyze-string-result)
/// fn:analyze-string($input as xs:string?, $pattern as xs:string, $flags as xs:string) as element(fn:analyze-string-result)
///
/// Analyzes a string using a regular expression, returning an XML structure that
/// identifies which parts of the input string matched or failed to match the
/// regular expression, and in the case of matched substrings, which substrings
/// matched each capturing group in the regular expression.
pub fn fn_analyze_string<N: Clone>(
    mut args: Vec<XdmValue<N>>,
) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() < 2 || args.len() > 3 {
        return Err(XPath31Error::function(
            "analyze-string",
            "Expected 2 or 3 arguments",
        ));
    }

    let flags = if args.len() == 3 {
        args.remove(2).to_string_value()
    } else {
        String::new()
    };

    let pattern = args.remove(1).to_string_value();
    let input_val = args.remove(0);

    if input_val.is_empty() {
        return Ok(XdmValue::from_string(build_empty_result()));
    }

    let input = input_val.to_string_value();

    let regex = build_regex(&pattern, &flags)?;

    let result = analyze_with_regex(&input, &regex);
    Ok(XdmValue::from_string(result))
}

fn build_regex(pattern: &str, flags: &str) -> Result<Regex, XPath31Error> {
    let mut regex_pattern = String::new();

    if flags.contains('i') {
        regex_pattern.push_str("(?i)");
    }
    if flags.contains('m') {
        regex_pattern.push_str("(?m)");
    }
    if flags.contains('s') {
        regex_pattern.push_str("(?s)");
    }
    if flags.contains('x') {
        regex_pattern.push_str("(?x)");
    }

    regex_pattern.push_str(pattern);

    Regex::new(&regex_pattern).map_err(|e| {
        XPath31Error::function("analyze-string", format!("Invalid regex pattern: {}", e))
    })
}

fn analyze_with_regex(input: &str, regex: &Regex) -> String {
    let mut result = String::from(
        r#"<fn:analyze-string-result xmlns:fn="http://www.w3.org/2005/xpath-functions">"#,
    );

    let mut last_end = 0;

    for captures in regex.captures_iter(input) {
        if let Some(whole_match) = captures.get(0) {
            if whole_match.start() > last_end {
                let non_match = &input[last_end..whole_match.start()];
                result.push_str("<fn:non-match>");
                result.push_str(&escape_xml(non_match));
                result.push_str("</fn:non-match>");
            }

            result.push_str("<fn:match>");
            append_match_content(&mut result, &captures, whole_match.as_str());
            result.push_str("</fn:match>");

            last_end = whole_match.end();
        }
    }

    if last_end < input.len() {
        let non_match = &input[last_end..];
        result.push_str("<fn:non-match>");
        result.push_str(&escape_xml(non_match));
        result.push_str("</fn:non-match>");
    }

    result.push_str("</fn:analyze-string-result>");
    result
}

fn append_match_content(result: &mut String, captures: &Captures, whole_match: &str) {
    let group_count = captures.len();

    if group_count <= 1 {
        result.push_str(&escape_xml(whole_match));
        return;
    }

    let mut positions: Vec<(usize, usize, usize)> = Vec::new();
    for i in 1..group_count {
        if let Some(m) = captures.get(i) {
            positions.push((m.start(), m.end(), i));
        }
    }

    positions.sort_by_key(|p| p.0);

    let match_start = captures.get(0).map(|m| m.start()).unwrap_or(0);
    let mut current_pos = match_start;

    for (start, end, group_num) in positions {
        if start > current_pos {
            let text = &whole_match[(current_pos - match_start)..(start - match_start)];
            result.push_str(&escape_xml(text));
        }

        result.push_str(&format!(r#"<fn:group nr="{}">"#, group_num));
        let group_text = &whole_match[(start - match_start)..(end - match_start)];
        result.push_str(&escape_xml(group_text));
        result.push_str("</fn:group>");

        current_pos = end;
    }

    let match_end = captures
        .get(0)
        .map(|m| m.end())
        .unwrap_or(whole_match.len());
    if current_pos < match_end {
        let text = &whole_match[(current_pos - match_start)..];
        result.push_str(&escape_xml(text));
    }
}

fn build_empty_result() -> String {
    r#"<fn:analyze-string-result xmlns:fn="http://www.w3.org/2005/xpath-functions"/>"#.to_string()
}

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// fn:matches($input as xs:string?, $pattern as xs:string) as xs:boolean
/// fn:matches($input as xs:string?, $pattern as xs:string, $flags as xs:string) as xs:boolean
///
/// Returns true if the input string matches the regular expression pattern.
pub fn fn_matches<N: Clone>(mut args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() < 2 || args.len() > 3 {
        return Err(XPath31Error::function(
            "matches",
            "Expected 2 or 3 arguments",
        ));
    }

    let flags = if args.len() == 3 {
        args.remove(2).to_string_value()
    } else {
        String::new()
    };

    let pattern = args.remove(1).to_string_value();
    let input_val = args.remove(0);

    if input_val.is_empty() {
        return Ok(XdmValue::from_bool(false));
    }

    let input = input_val.to_string_value();
    let regex = build_regex(&pattern, &flags)?;

    Ok(XdmValue::from_bool(regex.is_match(&input)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_analyze_string_simple() {
        let result: XdmValue<()> = fn_analyze_string(vec![
            XdmValue::from_string("The cat sat on the mat"),
            XdmValue::from_string("cat|mat"),
        ])
        .unwrap();

        let xml = result.to_string_value();
        assert!(xml.contains("<fn:match>cat</fn:match>"));
        assert!(xml.contains("<fn:match>mat</fn:match>"));
        assert!(xml.contains("<fn:non-match>The </fn:non-match>"));
    }

    #[test]
    fn test_analyze_string_with_groups() {
        let result: XdmValue<()> = fn_analyze_string(vec![
            XdmValue::from_string("2024-01-15"),
            XdmValue::from_string(r"(\d{4})-(\d{2})-(\d{2})"),
        ])
        .unwrap();

        let xml = result.to_string_value();
        assert!(xml.contains(r#"<fn:group nr="1">2024</fn:group>"#));
        assert!(xml.contains(r#"<fn:group nr="2">01</fn:group>"#));
        assert!(xml.contains(r#"<fn:group nr="3">15</fn:group>"#));
    }

    #[test]
    fn test_analyze_string_no_match() {
        let result: XdmValue<()> = fn_analyze_string(vec![
            XdmValue::from_string("hello world"),
            XdmValue::from_string("xyz"),
        ])
        .unwrap();

        let xml = result.to_string_value();
        assert!(xml.contains("<fn:non-match>hello world</fn:non-match>"));
        assert!(!xml.contains("<fn:match>"));
    }

    #[test]
    fn test_analyze_string_empty_input() {
        let result: XdmValue<()> =
            fn_analyze_string(vec![XdmValue::empty(), XdmValue::from_string("pattern")]).unwrap();

        let xml = result.to_string_value();
        assert!(xml.contains("fn:analyze-string-result"));
        assert!(!xml.contains("<fn:match>"));
        assert!(!xml.contains("<fn:non-match>"));
    }

    #[test]
    fn test_analyze_string_case_insensitive() {
        let result: XdmValue<()> = fn_analyze_string(vec![
            XdmValue::from_string("Hello HELLO hello"),
            XdmValue::from_string("hello"),
            XdmValue::from_string("i"),
        ])
        .unwrap();

        let xml = result.to_string_value();
        assert!(xml.contains("<fn:match>Hello</fn:match>"));
        assert!(xml.contains("<fn:match>HELLO</fn:match>"));
        assert!(xml.contains("<fn:match>hello</fn:match>"));
    }

    #[test]
    fn test_analyze_string_xml_escaping() {
        let result: XdmValue<()> = fn_analyze_string(vec![
            XdmValue::from_string("a<b>c"),
            XdmValue::from_string("b"),
        ])
        .unwrap();

        let xml = result.to_string_value();
        assert!(xml.contains("&lt;"));
        assert!(xml.contains("&gt;"));
    }

    #[test]
    fn test_matches_true() {
        let result: XdmValue<()> = fn_matches(vec![
            XdmValue::from_string("hello world"),
            XdmValue::from_string("world"),
        ])
        .unwrap();

        assert!(result.effective_boolean_value());
    }

    #[test]
    fn test_matches_false() {
        let result: XdmValue<()> = fn_matches(vec![
            XdmValue::from_string("hello world"),
            XdmValue::from_string("xyz"),
        ])
        .unwrap();

        assert!(!result.effective_boolean_value());
    }

    #[test]
    fn test_matches_case_insensitive() {
        let result: XdmValue<()> = fn_matches(vec![
            XdmValue::from_string("Hello World"),
            XdmValue::from_string("hello"),
            XdmValue::from_string("i"),
        ])
        .unwrap();

        assert!(result.effective_boolean_value());
    }

    #[test]
    fn test_matches_empty_input() {
        let result: XdmValue<()> =
            fn_matches(vec![XdmValue::empty(), XdmValue::from_string("pattern")]).unwrap();

        assert!(!result.effective_boolean_value());
    }
}
