use crate::error::XPath31Error;
use crate::types::{AtomicValue, XdmArray, XdmMap, XdmValue};

pub fn fn_parse_json<N: Clone>(args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if args.is_empty() || args.len() > 2 {
        return Err(XPath31Error::function(
            "parse-json",
            "Expected 1 or 2 arguments",
        ));
    }

    let json_string = args[0].to_string_value();
    if json_string.is_empty() {
        return Ok(XdmValue::empty());
    }

    let parsed: serde_json::Value = serde_json::from_str(&json_string)
        .map_err(|e| XPath31Error::function("parse-json", e.to_string()))?;

    json_value_to_xdm(&parsed)
}

pub fn fn_json_doc<N: Clone>(args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if args.is_empty() || args.len() > 2 {
        return Err(XPath31Error::function(
            "json-doc",
            "Expected 1 or 2 arguments",
        ));
    }

    Err(XPath31Error::function(
        "json-doc",
        "json-doc requires resource provider - use parse-json with pre-loaded content instead",
    ))
}

#[allow(dead_code)]
pub fn fn_json_doc_from_content<N: Clone>(
    content: &str,
    options: Option<&XdmMap<N>>,
) -> Result<XdmValue<N>, XPath31Error> {
    let opts = parse_json_options(options)?;

    if content.is_empty() {
        if opts.fallback.is_some() {
            return opts
                .fallback
                .ok_or_else(|| XPath31Error::function("json-doc", "Empty content"));
        }
        return Ok(XdmValue::empty());
    }

    let parsed: serde_json::Value = serde_json::from_str(content).map_err(|e| {
        if opts.fallback.is_some() {
            return XPath31Error::function("json-doc", format!("Parse error: {}", e));
        }
        XPath31Error::function("json-doc", format!("Invalid JSON: {}", e))
    })?;

    json_value_to_xdm(&parsed)
}

#[allow(dead_code)]
struct JsonParseOptions<N: Clone> {
    liberal: bool,
    duplicates: DuplicateHandling,
    escape: bool,
    fallback: Option<XdmValue<N>>,
}

#[allow(dead_code)]
#[derive(Clone, Copy, PartialEq)]
enum DuplicateHandling {
    Reject,
    UseFirst,
    UseLast,
}

#[allow(dead_code)]
fn parse_json_options<N: Clone>(
    options: Option<&XdmMap<N>>,
) -> Result<JsonParseOptions<N>, XPath31Error> {
    let mut opts = JsonParseOptions {
        liberal: false,
        duplicates: DuplicateHandling::UseLast,
        escape: false,
        fallback: None,
    };

    if let Some(map) = options {
        if let Some(val) = map.get(&AtomicValue::String("liberal".to_string())) {
            opts.liberal = val.effective_boolean_value();
        }
        if let Some(val) = map.get(&AtomicValue::String("duplicates".to_string())) {
            let s = val.to_string_value();
            opts.duplicates = match s.as_str() {
                "reject" => DuplicateHandling::Reject,
                "use-first" => DuplicateHandling::UseFirst,
                _ => DuplicateHandling::UseLast,
            };
        }
        if let Some(val) = map.get(&AtomicValue::String("escape".to_string())) {
            opts.escape = val.effective_boolean_value();
        }
        if let Some(val) = map.get(&AtomicValue::String("fallback".to_string())) {
            opts.fallback = Some(val.clone());
        }
    }

    Ok(opts)
}

pub fn fn_json_to_xml<N: Clone>(args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if args.is_empty() || args.len() > 2 {
        return Err(XPath31Error::function(
            "json-to-xml",
            "Expected 1 or 2 arguments",
        ));
    }

    let json_string = args[0].to_string_value();
    if json_string.is_empty() {
        return Ok(XdmValue::empty());
    }

    let parsed: serde_json::Value = serde_json::from_str(&json_string)
        .map_err(|e| XPath31Error::function("json-to-xml", e.to_string()))?;

    let xml = json_to_xml_string(&parsed, true);
    Ok(XdmValue::from_string(xml))
}

pub fn fn_xml_to_json<N: Clone>(args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if args.is_empty() || args.len() > 2 {
        return Err(XPath31Error::function(
            "xml-to-json",
            "Expected 1 or 2 arguments",
        ));
    }

    let xml_string = args[0].to_string_value();
    if xml_string.is_empty() {
        return Ok(XdmValue::empty());
    }

    let options = if args.len() > 1 {
        parse_xml_to_json_options(&args[1])?
    } else {
        XmlToJsonOptions::default()
    };

    let json = xml_string_to_json(&xml_string, &options)?;
    Ok(XdmValue::from_string(json))
}

#[derive(Default)]
struct XmlToJsonOptions {
    indent: bool,
}

fn parse_xml_to_json_options<N: Clone>(
    opts: &XdmValue<N>,
) -> Result<XmlToJsonOptions, XPath31Error> {
    let mut result = XmlToJsonOptions::default();

    if let Some(crate::types::XdmItem::Map(map)) = opts.first()
        && let Some(indent_val) = map.get(&AtomicValue::String("indent".into()))
    {
        result.indent = indent_val.effective_boolean_value();
    }

    Ok(result)
}

fn xml_string_to_json(xml: &str, options: &XmlToJsonOptions) -> Result<String, XPath31Error> {
    use quick_xml::Reader;
    use quick_xml::events::Event;

    let mut reader = Reader::from_str(xml);
    let mut buf = Vec::new();
    let mut json_value = serde_json::Value::Null;
    let mut stack: Vec<(String, Option<String>, serde_json::Value)> = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let local_name = String::from_utf8_lossy(e.local_name().as_ref()).to_string();
                let key = e
                    .attributes()
                    .filter_map(|a: Result<quick_xml::events::attributes::Attribute<'_>, _>| a.ok())
                    .find(|a| a.key.local_name().as_ref() == b"key")
                    .map(|a| String::from_utf8_lossy(&a.value).to_string());

                let initial_value = match local_name.as_str() {
                    "map" => serde_json::Value::Object(serde_json::Map::new()),
                    "array" => serde_json::Value::Array(Vec::new()),
                    _ => serde_json::Value::Null,
                };

                stack.push((local_name, key, initial_value));
            }
            Ok(Event::Empty(e)) => {
                let local_name = String::from_utf8_lossy(e.local_name().as_ref()).to_string();
                let key = e
                    .attributes()
                    .filter_map(|a: Result<quick_xml::events::attributes::Attribute<'_>, _>| a.ok())
                    .find(|a| a.key.local_name().as_ref() == b"key")
                    .map(|a| String::from_utf8_lossy(&a.value).to_string());

                let value = match local_name.as_str() {
                    "null" => serde_json::Value::Null,
                    "string" => serde_json::Value::String(String::new()),
                    "map" => serde_json::Value::Object(serde_json::Map::new()),
                    "array" => serde_json::Value::Array(Vec::new()),
                    _ => serde_json::Value::Null,
                };

                add_value_to_parent(&mut stack, &mut json_value, key, value);
            }
            Ok(Event::End(_)) => {
                if let Some((tag_name, key, value)) = stack.pop() {
                    let final_value = match tag_name.as_str() {
                        "map" | "array" => value,
                        _ => value,
                    };
                    add_value_to_parent(&mut stack, &mut json_value, key, final_value);
                }
            }
            Ok(Event::Text(e)) => {
                let text = String::from_utf8_lossy(&e).to_string();
                let trimmed = text.trim();
                if !trimmed.is_empty()
                    && let Some((tag_name, _, current_value)) = stack.last_mut()
                {
                    let parsed_value = match tag_name.as_str() {
                        "string" => serde_json::Value::String(trimmed.to_string()),
                        "number" => {
                            if let Ok(n) = trimmed.parse::<i64>() {
                                serde_json::Value::Number(n.into())
                            } else if let Ok(f) = trimmed.parse::<f64>() {
                                serde_json::Number::from_f64(f)
                                    .map(serde_json::Value::Number)
                                    .unwrap_or(serde_json::Value::Null)
                            } else {
                                serde_json::Value::Null
                            }
                        }
                        "boolean" => serde_json::Value::Bool(trimmed == "true"),
                        _ => continue,
                    };
                    *current_value = parsed_value;
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(e) => {
                return Err(XPath31Error::function(
                    "xml-to-json",
                    format!("XML parse error: {}", e),
                ));
            }
        }
        buf.clear();
    }

    if options.indent {
        serde_json::to_string_pretty(&json_value)
    } else {
        serde_json::to_string(&json_value)
    }
    .map_err(|e| XPath31Error::function("xml-to-json", e.to_string()))
}

fn add_value_to_parent(
    stack: &mut [(String, Option<String>, serde_json::Value)],
    root: &mut serde_json::Value,
    key: Option<String>,
    value: serde_json::Value,
) {
    if let Some((parent_tag, _, parent_value)) = stack.last_mut() {
        match parent_tag.as_str() {
            "map" => {
                if let (serde_json::Value::Object(map), Some(k)) = (parent_value, key) {
                    map.insert(k, value);
                }
            }
            "array" => {
                if let serde_json::Value::Array(arr) = parent_value {
                    arr.push(value);
                }
            }
            _ => {}
        }
    } else {
        *root = value;
    }
}

fn json_value_to_xdm<N: Clone>(value: &serde_json::Value) -> Result<XdmValue<N>, XPath31Error> {
    match value {
        serde_json::Value::Null => Ok(XdmValue::empty()),
        serde_json::Value::Bool(b) => Ok(XdmValue::from_bool(*b)),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(XdmValue::from_integer(i))
            } else if let Some(f) = n.as_f64() {
                Ok(XdmValue::from_double(f))
            } else {
                Err(XPath31Error::function("parse-json", "Invalid number"))
            }
        }
        serde_json::Value::String(s) => Ok(XdmValue::from_string(s.clone())),
        serde_json::Value::Array(arr) => {
            let members: Result<Vec<XdmValue<N>>, _> = arr.iter().map(json_value_to_xdm).collect();
            Ok(XdmValue::from_array(XdmArray::from_members(members?)))
        }
        serde_json::Value::Object(obj) => {
            let mut entries: Vec<(AtomicValue, XdmValue<N>)> = Vec::with_capacity(obj.len());
            for (key, val) in obj {
                let xdm_val = json_value_to_xdm(val)?;
                entries.push((AtomicValue::String(key.clone()), xdm_val));
            }
            Ok(XdmValue::from_map(XdmMap::from_entries(entries)))
        }
    }
}

fn json_to_xml_string(value: &serde_json::Value, is_root: bool) -> String {
    let mut result = String::new();

    if is_root {
        result.push_str(r#"<?xml version="1.0" encoding="UTF-8"?>"#);
        result.push_str(r#"<map xmlns="http://www.w3.org/2005/xpath-functions">"#);
    }

    match value {
        serde_json::Value::Null => {
            result.push_str("<null/>");
        }
        serde_json::Value::Bool(b) => {
            result.push_str(&format!("<boolean>{}</boolean>", b));
        }
        serde_json::Value::Number(n) => {
            result.push_str(&format!("<number>{}</number>", n));
        }
        serde_json::Value::String(s) => {
            let escaped = escape_xml(s);
            result.push_str(&format!("<string>{}</string>", escaped));
        }
        serde_json::Value::Array(arr) => {
            result.push_str("<array>");
            for item in arr {
                result.push_str(&json_to_xml_string(item, false));
            }
            result.push_str("</array>");
        }
        serde_json::Value::Object(obj) => {
            if !is_root {
                result.push_str("<map>");
            }
            for (key, val) in obj {
                let escaped_key = escape_xml(key);
                match val {
                    serde_json::Value::Null => {
                        result.push_str(&format!(r#"<null key="{}"/>"#, escaped_key));
                    }
                    serde_json::Value::Bool(b) => {
                        result.push_str(&format!(
                            r#"<boolean key="{}">{}</boolean>"#,
                            escaped_key, b
                        ));
                    }
                    serde_json::Value::Number(n) => {
                        result
                            .push_str(&format!(r#"<number key="{}">{}</number>"#, escaped_key, n));
                    }
                    serde_json::Value::String(s) => {
                        let escaped_val = escape_xml(s);
                        result.push_str(&format!(
                            r#"<string key="{}">{}</string>"#,
                            escaped_key, escaped_val
                        ));
                    }
                    serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
                        let inner = json_to_xml_inner(val);
                        let tag = if val.is_array() { "array" } else { "map" };
                        result.push_str(&format!(
                            r#"<{} key="{}">{}</{}>"#,
                            tag, escaped_key, inner, tag
                        ));
                    }
                }
            }
            if !is_root {
                result.push_str("</map>");
            }
        }
    }

    if is_root {
        result.push_str("</map>");
    }

    result
}

fn json_to_xml_inner(value: &serde_json::Value) -> String {
    let mut result = String::new();
    match value {
        serde_json::Value::Array(arr) => {
            for item in arr {
                result.push_str(&json_to_xml_string(item, false));
            }
        }
        serde_json::Value::Object(obj) => {
            for (key, val) in obj {
                let escaped_key = escape_xml(key);
                match val {
                    serde_json::Value::Null => {
                        result.push_str(&format!(r#"<null key="{}"/>"#, escaped_key));
                    }
                    serde_json::Value::Bool(b) => {
                        result.push_str(&format!(
                            r#"<boolean key="{}">{}</boolean>"#,
                            escaped_key, b
                        ));
                    }
                    serde_json::Value::Number(n) => {
                        result
                            .push_str(&format!(r#"<number key="{}">{}</number>"#, escaped_key, n));
                    }
                    serde_json::Value::String(s) => {
                        let escaped_val = escape_xml(s);
                        result.push_str(&format!(
                            r#"<string key="{}">{}</string>"#,
                            escaped_key, escaped_val
                        ));
                    }
                    _ => {
                        let inner = json_to_xml_inner(val);
                        let tag = if val.is_array() { "array" } else { "map" };
                        result.push_str(&format!(
                            r#"<{} key="{}">{}</{}>"#,
                            tag, escaped_key, inner, tag
                        ));
                    }
                }
            }
        }
        _ => {}
    }
    result
}

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::XdmItem;

    fn extract_integer<N: Clone>(value: &XdmValue<N>) -> Option<i64> {
        if let Some(XdmItem::Atomic(AtomicValue::Integer(n))) = value.first() {
            Some(*n)
        } else {
            None
        }
    }

    type TestNode = ();

    #[test]
    fn test_parse_json_string() {
        let args: Vec<XdmValue<TestNode>> = vec![XdmValue::from_string(r#""hello""#.to_string())];
        let result = fn_parse_json(args).unwrap();
        assert_eq!(result.to_string_value(), "hello");
    }

    #[test]
    fn test_parse_json_number() {
        let args: Vec<XdmValue<TestNode>> = vec![XdmValue::from_string("42".to_string())];
        let result = fn_parse_json(args).unwrap();
        assert_eq!(extract_integer(&result), Some(42));
    }

    #[test]
    fn test_parse_json_boolean() {
        let args: Vec<XdmValue<TestNode>> = vec![XdmValue::from_string("true".to_string())];
        let result = fn_parse_json(args).unwrap();
        assert!(result.effective_boolean_value());
    }

    #[test]
    fn test_parse_json_array() {
        let args: Vec<XdmValue<TestNode>> = vec![XdmValue::from_string("[1, 2, 3]".to_string())];
        let result = fn_parse_json(args).unwrap();
        if let Some(XdmItem::Array(arr)) = result.first() {
            assert_eq!(arr.size(), 3);
        } else {
            panic!("Expected array");
        }
    }

    #[test]
    fn test_parse_json_object() {
        let args: Vec<XdmValue<TestNode>> =
            vec![XdmValue::from_string(r#"{"a": 1, "b": 2}"#.to_string())];
        let result = fn_parse_json(args).unwrap();
        if let Some(XdmItem::Map(map)) = result.first() {
            assert_eq!(map.size(), 2);
            let val = map.get(&AtomicValue::String("a".to_string())).unwrap();
            assert_eq!(extract_integer(val), Some(1));
        } else {
            panic!("Expected map");
        }
    }

    #[test]
    fn test_parse_json_null() {
        let args: Vec<XdmValue<TestNode>> = vec![XdmValue::from_string("null".to_string())];
        let result = fn_parse_json(args).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_json_nested() {
        let json = r#"{"users": [{"name": "Alice"}, {"name": "Bob"}]}"#;
        let args: Vec<XdmValue<TestNode>> = vec![XdmValue::from_string(json.to_string())];
        let result = fn_parse_json(args).unwrap();
        if let Some(XdmItem::Map(map)) = result.first() {
            let users = map.get(&AtomicValue::String("users".to_string())).unwrap();
            if let Some(XdmItem::Array(arr)) = users.first() {
                assert_eq!(arr.size(), 2);
            } else {
                panic!("Expected array for users");
            }
        } else {
            panic!("Expected map");
        }
    }

    #[test]
    fn test_json_to_xml_simple() {
        let args: Vec<XdmValue<TestNode>> =
            vec![XdmValue::from_string(r#"{"key": "value"}"#.to_string())];
        let result = fn_json_to_xml(args).unwrap();
        let xml = result.to_string_value();
        assert!(xml.contains("<string key=\"key\">value</string>"));
    }

    #[test]
    fn test_xml_to_json_string() {
        let xml = r#"<string xmlns="http://www.w3.org/2005/xpath-functions">hello</string>"#;
        let args: Vec<XdmValue<TestNode>> = vec![XdmValue::from_string(xml.to_string())];
        let result = fn_xml_to_json(args).unwrap();
        assert_eq!(result.to_string_value(), r#""hello""#);
    }

    #[test]
    fn test_xml_to_json_number() {
        let xml = r#"<number xmlns="http://www.w3.org/2005/xpath-functions">42</number>"#;
        let args: Vec<XdmValue<TestNode>> = vec![XdmValue::from_string(xml.to_string())];
        let result = fn_xml_to_json(args).unwrap();
        assert_eq!(result.to_string_value(), "42");
    }

    #[test]
    fn test_xml_to_json_boolean() {
        let xml = r#"<boolean xmlns="http://www.w3.org/2005/xpath-functions">true</boolean>"#;
        let args: Vec<XdmValue<TestNode>> = vec![XdmValue::from_string(xml.to_string())];
        let result = fn_xml_to_json(args).unwrap();
        assert_eq!(result.to_string_value(), "true");
    }

    #[test]
    fn test_xml_to_json_null() {
        let xml = r#"<null xmlns="http://www.w3.org/2005/xpath-functions"/>"#;
        let args: Vec<XdmValue<TestNode>> = vec![XdmValue::from_string(xml.to_string())];
        let result = fn_xml_to_json(args).unwrap();
        assert_eq!(result.to_string_value(), "null");
    }

    #[test]
    fn test_xml_to_json_map() {
        let xml = r#"
            <map xmlns="http://www.w3.org/2005/xpath-functions">
                <string key="name">Alice</string>
                <number key="age">30</number>
            </map>
        "#;
        let args: Vec<XdmValue<TestNode>> = vec![XdmValue::from_string(xml.to_string())];
        let result = fn_xml_to_json(args).unwrap();
        let json: serde_json::Value = serde_json::from_str(&result.to_string_value()).unwrap();
        assert_eq!(json["name"], "Alice");
        assert_eq!(json["age"], 30);
    }

    #[test]
    fn test_xml_to_json_array() {
        let xml = r#"
            <array xmlns="http://www.w3.org/2005/xpath-functions">
                <number>1</number>
                <number>2</number>
                <number>3</number>
            </array>
        "#;
        let args: Vec<XdmValue<TestNode>> = vec![XdmValue::from_string(xml.to_string())];
        let result = fn_xml_to_json(args).unwrap();
        let json: serde_json::Value = serde_json::from_str(&result.to_string_value()).unwrap();
        assert_eq!(json, serde_json::json!([1, 2, 3]));
    }

    #[test]
    fn test_xml_to_json_roundtrip() {
        let original = r#"{"users":[{"name":"Alice","age":30},{"name":"Bob","age":25}]}"#;

        let xml_args: Vec<XdmValue<TestNode>> = vec![XdmValue::from_string(original.to_string())];
        let xml_result = fn_json_to_xml(xml_args).unwrap();

        let json_args: Vec<XdmValue<TestNode>> =
            vec![XdmValue::from_string(xml_result.to_string_value())];
        let json_result = fn_xml_to_json(json_args).unwrap();

        let original_json: serde_json::Value = serde_json::from_str(original).unwrap();
        let result_json: serde_json::Value =
            serde_json::from_str(&json_result.to_string_value()).unwrap();
        assert_eq!(original_json, result_json);
    }

    #[test]
    fn test_xml_to_json_with_indent() {
        let xml = r#"<map xmlns="http://www.w3.org/2005/xpath-functions"><string key="a">b</string></map>"#;
        let options = XdmValue::from_map(XdmMap::from_entries(vec![(
            AtomicValue::String("indent".into()),
            XdmValue::from_bool(true),
        )]));
        let args: Vec<XdmValue<TestNode>> = vec![XdmValue::from_string(xml.to_string()), options];
        let result = fn_xml_to_json(args).unwrap();
        assert!(result.to_string_value().contains('\n'));
    }

    #[test]
    fn test_xml_to_json_empty() {
        let args: Vec<XdmValue<TestNode>> = vec![XdmValue::from_string("".to_string())];
        let result = fn_xml_to_json(args).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_json_doc_from_content_object() {
        let content = r#"{"name": "Alice", "age": 30}"#;
        let result: XdmValue<TestNode> = fn_json_doc_from_content(content, None).unwrap();
        if let Some(XdmItem::Map(map)) = result.first() {
            assert_eq!(map.size(), 2);
            let name = map.get(&AtomicValue::String("name".to_string())).unwrap();
            assert_eq!(name.to_string_value(), "Alice");
        } else {
            panic!("Expected map");
        }
    }

    #[test]
    fn test_json_doc_from_content_array() {
        let content = r#"[1, 2, 3, 4, 5]"#;
        let result: XdmValue<TestNode> = fn_json_doc_from_content(content, None).unwrap();
        if let Some(XdmItem::Array(arr)) = result.first() {
            assert_eq!(arr.size(), 5);
        } else {
            panic!("Expected array");
        }
    }

    #[test]
    fn test_json_doc_from_content_empty() {
        let result: XdmValue<TestNode> = fn_json_doc_from_content("", None).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_json_doc_from_content_invalid() {
        let result: Result<XdmValue<TestNode>, _> =
            fn_json_doc_from_content("not valid json", None);
        assert!(result.is_err());
    }

    #[test]
    fn test_json_doc_from_content_nested() {
        let content = r#"{
            "company": "Acme",
            "employees": [
                {"name": "Alice", "role": "Engineer"},
                {"name": "Bob", "role": "Manager"}
            ]
        }"#;
        let result: XdmValue<TestNode> = fn_json_doc_from_content(content, None).unwrap();
        if let Some(XdmItem::Map(map)) = result.first() {
            let employees = map
                .get(&AtomicValue::String("employees".to_string()))
                .unwrap();
            if let Some(XdmItem::Array(arr)) = employees.first() {
                assert_eq!(arr.size(), 2);
            } else {
                panic!("Expected array for employees");
            }
        } else {
            panic!("Expected map");
        }
    }
}
