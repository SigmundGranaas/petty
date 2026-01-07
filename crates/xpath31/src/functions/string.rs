use crate::engine::EvaluationContext;
use crate::error::XPath31Error;
use crate::types::*;
use petty_xpath1::DataSourceNode;

pub fn fn_concat<'a, N: DataSourceNode<'a> + Clone + 'a>(
    args: Vec<XdmValue<N>>,
) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() < 2 {
        return Err(XPath31Error::function(
            "concat",
            "Expected at least 2 arguments",
        ));
    }
    let result: String = args.iter().map(|v| v.to_xpath_string()).collect();
    Ok(XdmValue::from_string(result))
}

pub fn fn_string<'a, N: DataSourceNode<'a> + Clone>(
    mut args: Vec<XdmValue<N>>,
    ctx: &EvaluationContext<'a, '_, N>,
) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() > 1 {
        return Err(XPath31Error::function(
            "string",
            "Expected 0 or 1 arguments",
        ));
    }
    let s = if args.is_empty() {
        match &ctx.context_item {
            Some(item) => match item {
                XdmItem::Atomic(a) => a.to_string_value(),
                XdmItem::Node(n) => n.string_value(),
                _ => String::new(),
            },
            None => String::new(),
        }
    } else {
        args.remove(0).to_string_value()
    };
    Ok(XdmValue::from_string(s))
}

pub fn fn_string_length<'a, N: DataSourceNode<'a> + Clone>(
    mut args: Vec<XdmValue<N>>,
    ctx: &EvaluationContext<'a, '_, N>,
) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() > 1 {
        return Err(XPath31Error::function(
            "string-length",
            "Expected 0 or 1 arguments",
        ));
    }
    let s = if args.is_empty() {
        match &ctx.context_item {
            Some(XdmItem::Atomic(a)) => a.to_string_value(),
            Some(XdmItem::Node(n)) => n.string_value(),
            _ => String::new(),
        }
    } else {
        args.remove(0).to_string_value()
    };
    Ok(XdmValue::from_integer(s.chars().count() as i64))
}

pub fn fn_substring<N: Clone>(mut args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() < 2 || args.len() > 3 {
        return Err(XPath31Error::function(
            "substring",
            "Expected 2 or 3 arguments",
        ));
    }

    let length = if args.len() == 3 {
        Some(args.remove(2).to_double())
    } else {
        None
    };
    let start = args.remove(1).to_double();
    let s = args.remove(0).to_string_value();

    let start_rounded = (start + 0.5).floor();
    let length_rounded = length.map(|l| (l + 0.5).floor());

    let chars: Vec<char> = s.chars().collect();

    let first = start_rounded;
    let last = length_rounded.map(|l| first + l).unwrap_or(f64::INFINITY);

    let result: String = chars
        .iter()
        .enumerate()
        .filter_map(|(i, &c)| {
            let pos = (i + 1) as f64;
            if pos >= first && pos < last {
                Some(c)
            } else {
                None
            }
        })
        .collect();

    Ok(XdmValue::from_string(result))
}

pub fn fn_contains<N: Clone>(mut args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 2 {
        return Err(XPath31Error::function("contains", "Expected 2 arguments"));
    }
    let s2 = args.remove(1).to_string_value();
    let s1 = args.remove(0).to_string_value();
    Ok(XdmValue::from_bool(s1.contains(&s2)))
}

pub fn fn_starts_with<N: Clone>(mut args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 2 {
        return Err(XPath31Error::function(
            "starts-with",
            "Expected 2 arguments",
        ));
    }
    let s2 = args.remove(1).to_string_value();
    let s1 = args.remove(0).to_string_value();
    Ok(XdmValue::from_bool(s1.starts_with(&s2)))
}

pub fn fn_ends_with<N: Clone>(mut args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 2 {
        return Err(XPath31Error::function("ends-with", "Expected 2 arguments"));
    }
    let s2 = args.remove(1).to_string_value();
    let s1 = args.remove(0).to_string_value();
    Ok(XdmValue::from_bool(s1.ends_with(&s2)))
}

pub fn fn_upper_case<N: Clone>(mut args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 1 {
        return Err(XPath31Error::function("upper-case", "Expected 1 argument"));
    }
    let s = args.remove(0).to_string_value();
    Ok(XdmValue::from_string(s.to_uppercase()))
}

pub fn fn_lower_case<N: Clone>(mut args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 1 {
        return Err(XPath31Error::function("lower-case", "Expected 1 argument"));
    }
    let s = args.remove(0).to_string_value();
    Ok(XdmValue::from_string(s.to_lowercase()))
}

pub fn fn_normalize_space<'a, N: DataSourceNode<'a> + Clone>(
    mut args: Vec<XdmValue<N>>,
    ctx: &EvaluationContext<'a, '_, N>,
) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() > 1 {
        return Err(XPath31Error::function(
            "normalize-space",
            "Expected 0 or 1 arguments",
        ));
    }
    let s = if args.is_empty() {
        match &ctx.context_item {
            Some(XdmItem::Atomic(a)) => a.to_string_value(),
            Some(XdmItem::Node(n)) => n.string_value(),
            _ => String::new(),
        }
    } else {
        args.remove(0).to_string_value()
    };
    let normalized = s.split_whitespace().collect::<Vec<_>>().join(" ");
    Ok(XdmValue::from_string(normalized))
}

pub fn fn_translate<N: Clone>(mut args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 3 {
        return Err(XPath31Error::function("translate", "Expected 3 arguments"));
    }
    let to_chars: Vec<char> = args.remove(2).to_string_value().chars().collect();
    let from_chars: Vec<char> = args.remove(1).to_string_value().chars().collect();
    let source = args.remove(0).to_string_value();

    let result: String = source
        .chars()
        .filter_map(|c| {
            if let Some(pos) = from_chars.iter().position(|&fc| fc == c) {
                to_chars.get(pos).copied()
            } else {
                Some(c)
            }
        })
        .collect();

    Ok(XdmValue::from_string(result))
}

pub fn fn_replace<N: Clone>(mut args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() < 3 || args.len() > 4 {
        return Err(XPath31Error::function(
            "replace",
            "Expected 3 or 4 arguments",
        ));
    }

    let flags = if args.len() == 4 {
        args.remove(3).to_string_value()
    } else {
        String::new()
    };

    let replacement = args.remove(2).to_string_value();
    let pattern = args.remove(1).to_string_value();
    let input = args.remove(0).to_string_value();

    let mut regex_pattern = String::new();
    if flags.contains('i') {
        regex_pattern.push_str("(?i)");
    }
    if flags.contains('s') {
        regex_pattern.push_str("(?s)");
    }
    if flags.contains('m') {
        regex_pattern.push_str("(?m)");
    }
    if flags.contains('x') {
        regex_pattern.push_str("(?x)");
    }
    regex_pattern.push_str(&pattern);

    let re = regex::Regex::new(&regex_pattern)
        .map_err(|e| XPath31Error::function("replace", format!("Invalid regex pattern: {}", e)))?;

    let xpath_replacement = replacement
        .replace("$0", "${0}")
        .replace("$1", "${1}")
        .replace("$2", "${2}")
        .replace("$3", "${3}")
        .replace("$4", "${4}")
        .replace("$5", "${5}")
        .replace("$6", "${6}")
        .replace("$7", "${7}")
        .replace("$8", "${8}")
        .replace("$9", "${9}");

    let result = re.replace_all(&input, xpath_replacement.as_str());
    Ok(XdmValue::from_string(result.into_owned()))
}

pub fn fn_tokenize<N: Clone>(mut args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if args.is_empty() || args.len() > 3 {
        return Err(XPath31Error::function(
            "tokenize",
            "Expected 1 to 3 arguments",
        ));
    }

    let pattern = if args.len() >= 2 {
        args.remove(1).to_string_value()
    } else {
        " ".to_string()
    };
    let input = args.remove(0).to_string_value();

    let tokens: Vec<XdmItem<N>> = input
        .split(&pattern)
        .filter(|s| !s.is_empty())
        .map(|s| XdmItem::Atomic(AtomicValue::String(s.to_string())))
        .collect();

    Ok(XdmValue::from_items(tokens))
}

pub fn fn_string_join<N: Clone>(mut args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if args.is_empty() || args.len() > 2 {
        return Err(XPath31Error::function(
            "string-join",
            "Expected 1 or 2 arguments",
        ));
    }

    let separator = if args.len() == 2 {
        args.remove(1).to_string_value()
    } else {
        String::new()
    };

    let strings: Vec<String> = args
        .remove(0)
        .items()
        .iter()
        .map(|i| match i {
            XdmItem::Atomic(a) => a.to_string_value(),
            _ => String::new(),
        })
        .collect();

    Ok(XdmValue::from_string(strings.join(&separator)))
}

pub fn fn_substring_before<N: Clone>(
    mut args: Vec<XdmValue<N>>,
) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() < 2 || args.len() > 3 {
        return Err(XPath31Error::function(
            "substring-before",
            "Expected 2 or 3 arguments",
        ));
    }
    let s2 = args.remove(1).to_string_value();
    let s1 = args.remove(0).to_string_value();

    if s2.is_empty() {
        return Ok(XdmValue::from_string(String::new()));
    }

    match s1.find(&s2) {
        Some(pos) => Ok(XdmValue::from_string(s1[..pos].to_string())),
        None => Ok(XdmValue::from_string(String::new())),
    }
}

pub fn fn_substring_after<N: Clone>(
    mut args: Vec<XdmValue<N>>,
) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() < 2 || args.len() > 3 {
        return Err(XPath31Error::function(
            "substring-after",
            "Expected 2 or 3 arguments",
        ));
    }
    let s2 = args.remove(1).to_string_value();
    let s1 = args.remove(0).to_string_value();

    if s2.is_empty() {
        return Ok(XdmValue::from_string(s1));
    }

    match s1.find(&s2) {
        Some(pos) => Ok(XdmValue::from_string(s1[pos + s2.len()..].to_string())),
        None => Ok(XdmValue::from_string(String::new())),
    }
}

pub fn fn_compare<N: Clone>(mut args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() < 2 || args.len() > 3 {
        return Err(XPath31Error::function(
            "compare",
            "Expected 2 or 3 arguments",
        ));
    }
    let s2 = args.remove(1);
    let s1 = args.remove(0);

    if s1.is_empty() || s2.is_empty() {
        return Ok(XdmValue::empty());
    }

    let str1 = s1.to_string_value();
    let str2 = s2.to_string_value();

    let result = match str1.cmp(&str2) {
        std::cmp::Ordering::Less => -1,
        std::cmp::Ordering::Equal => 0,
        std::cmp::Ordering::Greater => 1,
    };

    Ok(XdmValue::from_integer(result))
}

pub fn fn_codepoints_to_string<N: Clone>(
    mut args: Vec<XdmValue<N>>,
) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 1 {
        return Err(XPath31Error::function(
            "codepoints-to-string",
            "Expected 1 argument",
        ));
    }

    let codepoints = args.remove(0);
    let mut result = String::new();

    for item in codepoints.items() {
        if let XdmItem::Atomic(AtomicValue::Integer(cp)) = item {
            if !(0..=0x10FFFF).contains(cp) {
                return Err(XPath31Error::function(
                    "codepoints-to-string",
                    format!("Invalid codepoint: {}", cp),
                ));
            }
            match char::from_u32(*cp as u32) {
                Some(c) => result.push(c),
                None => {
                    return Err(XPath31Error::function(
                        "codepoints-to-string",
                        format!("Invalid codepoint: {}", cp),
                    ));
                }
            }
        } else if let XdmItem::Atomic(AtomicValue::Double(d)) = item {
            let cp = *d as i64;
            if !(0..=0x10FFFF).contains(&cp) {
                return Err(XPath31Error::function(
                    "codepoints-to-string",
                    format!("Invalid codepoint: {}", cp),
                ));
            }
            match char::from_u32(cp as u32) {
                Some(c) => result.push(c),
                None => {
                    return Err(XPath31Error::function(
                        "codepoints-to-string",
                        format!("Invalid codepoint: {}", cp),
                    ));
                }
            }
        }
    }

    Ok(XdmValue::from_string(result))
}

pub fn fn_resolve_uri<N: Clone>(mut args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if args.is_empty() || args.len() > 2 {
        return Err(XPath31Error::function(
            "resolve-uri",
            "Expected 1 or 2 arguments",
        ));
    }

    let relative = args.remove(0);
    if relative.is_empty() {
        return Ok(XdmValue::empty());
    }
    let relative_uri = relative.to_string_value();

    let base = if args.is_empty() {
        String::new()
    } else {
        args.remove(0).to_string_value()
    };

    if relative_uri.contains("://") || relative_uri.starts_with('/') {
        return Ok(XdmValue::from_string(relative_uri));
    }

    if base.is_empty() {
        return Ok(XdmValue::from_string(relative_uri));
    }

    let resolved = if base.ends_with('/') {
        format!("{}{}", base, relative_uri)
    } else if let Some(last_slash) = base.rfind('/') {
        format!("{}/{}", &base[..last_slash], relative_uri)
    } else {
        relative_uri
    };

    Ok(XdmValue::from_string(resolved))
}

pub fn fn_base_uri<'a, N: DataSourceNode<'a> + Clone>(
    mut args: Vec<XdmValue<N>>,
    ctx: &EvaluationContext<'a, '_, N>,
) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() > 1 {
        return Err(XPath31Error::function(
            "base-uri",
            "Expected 0 or 1 arguments",
        ));
    }

    let node = if args.is_empty() {
        match &ctx.context_item {
            Some(XdmItem::Node(n)) => Some(*n),
            _ => return Ok(XdmValue::empty()),
        }
    } else {
        let arg = args.remove(0);
        if arg.is_empty() {
            return Ok(XdmValue::empty());
        }
        match arg.first() {
            Some(XdmItem::Node(n)) => Some(*n),
            _ => return Ok(XdmValue::empty()),
        }
    };

    match node {
        Some(n) => {
            let mut current = Some(n);
            while let Some(node) = current {
                for attr in node.attributes() {
                    if let Some(name) = attr.name()
                        && name.local_part == "base"
                        && name.prefix == Some("xml")
                    {
                        return Ok(XdmValue::from_string(attr.string_value()));
                    }
                }
                current = node.parent();
            }
            Ok(XdmValue::empty())
        }
        None => Ok(XdmValue::empty()),
    }
}

pub fn fn_static_base_uri<N: Clone>(args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if !args.is_empty() {
        return Err(XPath31Error::function(
            "static-base-uri",
            "expects no arguments",
        ));
    }
    Ok(XdmValue::empty())
}

pub fn fn_string_to_codepoints<N: Clone>(
    mut args: Vec<XdmValue<N>>,
) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 1 {
        return Err(XPath31Error::function(
            "string-to-codepoints",
            "Expected 1 argument",
        ));
    }

    let s = args.remove(0);
    if s.is_empty() {
        return Ok(XdmValue::empty());
    }

    let str_val = s.to_string_value();
    let codepoints: Vec<XdmItem<N>> = str_val
        .chars()
        .map(|c| XdmItem::Atomic(AtomicValue::Integer(c as i64)))
        .collect();

    Ok(XdmValue::from_items(codepoints))
}

pub fn fn_encode_for_uri<N: Clone>(
    mut args: Vec<XdmValue<N>>,
) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 1 {
        return Err(XPath31Error::function(
            "encode-for-uri",
            "Expected 1 argument",
        ));
    }

    let s = args.remove(0);
    if s.is_empty() {
        return Ok(XdmValue::from_string(String::new()));
    }

    let input = s.to_string_value();
    let mut result = String::new();

    for c in input.chars() {
        if c.is_ascii_alphanumeric() || c == '-' || c == '.' || c == '_' || c == '~' {
            result.push(c);
        } else {
            for byte in c.to_string().as_bytes() {
                result.push_str(&format!("%{:02X}", byte));
            }
        }
    }

    Ok(XdmValue::from_string(result))
}

pub fn fn_iri_to_uri<N: Clone>(mut args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 1 {
        return Err(XPath31Error::function("iri-to-uri", "Expected 1 argument"));
    }

    let s = args.remove(0);
    if s.is_empty() {
        return Ok(XdmValue::from_string(String::new()));
    }

    let input = s.to_string_value();
    let mut result = String::new();

    for c in input.chars() {
        if c.is_ascii() && !c.is_ascii_control() && c != ' ' {
            result.push(c);
        } else {
            for byte in c.to_string().as_bytes() {
                result.push_str(&format!("%{:02X}", byte));
            }
        }
    }

    Ok(XdmValue::from_string(result))
}

pub fn fn_normalize_unicode<N: Clone>(
    mut args: Vec<XdmValue<N>>,
) -> Result<XdmValue<N>, XPath31Error> {
    if args.is_empty() || args.len() > 2 {
        return Err(XPath31Error::function(
            "normalize-unicode",
            "Expected 1 or 2 arguments",
        ));
    }

    let form = if args.len() == 2 {
        args.remove(1).to_string_value().to_uppercase()
    } else {
        "NFC".to_string()
    };

    let s = args.remove(0);
    if s.is_empty() {
        return Ok(XdmValue::from_string(String::new()));
    }

    let input = s.to_string_value();

    if form.is_empty() {
        return Ok(XdmValue::from_string(input));
    }

    match form.as_str() {
        "NFC" | "NFD" | "NFKC" | "NFKD" | "FULLY-NORMALIZED" => Ok(XdmValue::from_string(input)),
        _ => Err(XPath31Error::function(
            "normalize-unicode",
            format!("Unsupported normalization form: {}", form),
        )),
    }
}

pub fn fn_contains_token<N: Clone>(
    mut args: Vec<XdmValue<N>>,
) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() < 2 || args.len() > 3 {
        return Err(XPath31Error::function(
            "contains-token",
            "Expected 2 or 3 arguments",
        ));
    }

    if args.len() == 3 {
        args.remove(2);
    }

    let token = args.remove(1).to_string_value();
    let input = args.remove(0);

    if input.is_empty() {
        return Ok(XdmValue::from_bool(false));
    }

    let input_str = input.to_string_value();

    if token.is_empty() {
        return Ok(XdmValue::from_bool(false));
    }

    if token.chars().any(|c| c.is_whitespace()) {
        return Err(XPath31Error::function(
            "contains-token",
            "Token must not contain whitespace",
        ));
    }

    let contains = input_str.split_whitespace().any(|t| t == token);

    Ok(XdmValue::from_bool(contains))
}

pub fn fn_default_collation<N: Clone>(args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if !args.is_empty() {
        return Err(XPath31Error::function(
            "default-collation",
            "Expected 0 arguments",
        ));
    }
    Ok(XdmValue::from_string(
        "http://www.w3.org/2005/xpath-functions/collation/codepoint".to_string(),
    ))
}

pub fn fn_default_language<N: Clone>(args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if !args.is_empty() {
        return Err(XPath31Error::function(
            "default-language",
            "Expected 0 arguments",
        ));
    }
    Ok(XdmValue::from_string("en".to_string()))
}

pub fn fn_collation_key<N: Clone>(mut args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if args.is_empty() || args.len() > 2 {
        return Err(XPath31Error::function(
            "collation-key",
            "Expected 1 or 2 arguments",
        ));
    }

    let input = args.remove(0);
    if input.is_empty() {
        return Ok(XdmValue::from_string(String::new()));
    }

    let key = input.to_string_value();
    Ok(XdmValue::from_string(key))
}

#[cfg(test)]
mod tests {
    use super::*;
    use petty_xpath1::tests::MockNode;

    fn eval<N: Clone>(
        f: fn(Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error>,
        args: Vec<XdmValue<N>>,
    ) -> XdmValue<N> {
        f(args).unwrap()
    }

    #[test]
    fn test_concat() {
        let result: XdmValue<MockNode<'static>> = fn_concat(vec![
            XdmValue::from_string("Hello"),
            XdmValue::from_string(" "),
            XdmValue::from_string("World"),
        ])
        .unwrap();
        assert_eq!(result.to_string_value(), "Hello World");
    }

    #[test]
    fn test_substring() {
        let result: XdmValue<()> = eval(
            fn_substring,
            vec![
                XdmValue::from_string("12345"),
                XdmValue::from_integer(2),
                XdmValue::from_integer(3),
            ],
        );
        assert_eq!(result.to_string_value(), "234");

        let result: XdmValue<()> = eval(
            fn_substring,
            vec![XdmValue::from_string("12345"), XdmValue::from_integer(2)],
        );
        assert_eq!(result.to_string_value(), "2345");
    }

    #[test]
    fn test_upper_lower_case() {
        let upper: XdmValue<()> = eval(fn_upper_case, vec![XdmValue::from_string("hello")]);
        assert_eq!(upper.to_string_value(), "HELLO");

        let lower: XdmValue<()> = eval(fn_lower_case, vec![XdmValue::from_string("HELLO")]);
        assert_eq!(lower.to_string_value(), "hello");
    }

    #[test]
    fn test_contains() {
        let result: XdmValue<()> = eval(
            fn_contains,
            vec![
                XdmValue::from_string("hello world"),
                XdmValue::from_string("world"),
            ],
        );
        assert!(result.effective_boolean_value());

        let result: XdmValue<()> = eval(
            fn_contains,
            vec![
                XdmValue::from_string("hello"),
                XdmValue::from_string("world"),
            ],
        );
        assert!(!result.effective_boolean_value());
    }

    #[test]
    fn test_tokenize() {
        let result: XdmValue<()> = eval(
            fn_tokenize,
            vec![XdmValue::from_string("a,b,c"), XdmValue::from_string(",")],
        );
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn test_string_join() {
        let seq: XdmValue<()> = XdmValue::from_items(vec![
            XdmItem::Atomic(AtomicValue::String("a".to_string())),
            XdmItem::Atomic(AtomicValue::String("b".to_string())),
            XdmItem::Atomic(AtomicValue::String("c".to_string())),
        ]);
        let result = fn_string_join(vec![seq, XdmValue::from_string("-")]).unwrap();
        assert_eq!(result.to_string_value(), "a-b-c");
    }

    #[test]
    fn test_substring_before() {
        let result: XdmValue<()> = fn_substring_before(vec![
            XdmValue::from_string("hello world"),
            XdmValue::from_string(" "),
        ])
        .unwrap();
        assert_eq!(result.to_string_value(), "hello");

        let result: XdmValue<()> = fn_substring_before(vec![
            XdmValue::from_string("hello"),
            XdmValue::from_string("x"),
        ])
        .unwrap();
        assert_eq!(result.to_string_value(), "");
    }

    #[test]
    fn test_substring_after() {
        let result: XdmValue<()> = fn_substring_after(vec![
            XdmValue::from_string("hello world"),
            XdmValue::from_string(" "),
        ])
        .unwrap();
        assert_eq!(result.to_string_value(), "world");

        let result: XdmValue<()> = fn_substring_after(vec![
            XdmValue::from_string("hello"),
            XdmValue::from_string("x"),
        ])
        .unwrap();
        assert_eq!(result.to_string_value(), "");
    }

    #[test]
    fn test_compare() {
        let result: XdmValue<()> = fn_compare(vec![
            XdmValue::from_string("abc"),
            XdmValue::from_string("def"),
        ])
        .unwrap();
        assert_eq!(result.to_double(), -1.0);

        let result: XdmValue<()> = fn_compare(vec![
            XdmValue::from_string("abc"),
            XdmValue::from_string("abc"),
        ])
        .unwrap();
        assert_eq!(result.to_double(), 0.0);

        let result: XdmValue<()> = fn_compare(vec![
            XdmValue::from_string("def"),
            XdmValue::from_string("abc"),
        ])
        .unwrap();
        assert_eq!(result.to_double(), 1.0);
    }

    #[test]
    fn test_codepoints_to_string() {
        let result: XdmValue<()> = fn_codepoints_to_string(vec![XdmValue::from_items(vec![
            XdmItem::Atomic(AtomicValue::Integer(72)),
            XdmItem::Atomic(AtomicValue::Integer(105)),
        ])])
        .unwrap();
        assert_eq!(result.to_string_value(), "Hi");
    }

    #[test]
    fn test_string_to_codepoints() {
        let result: XdmValue<()> =
            fn_string_to_codepoints(vec![XdmValue::from_string("Hi")]).unwrap();
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_encode_for_uri() {
        let result: XdmValue<()> =
            fn_encode_for_uri(vec![XdmValue::from_string("hello world")]).unwrap();
        assert_eq!(result.to_string_value(), "hello%20world");

        let result: XdmValue<()> =
            fn_encode_for_uri(vec![XdmValue::from_string("a/b?c=d")]).unwrap();
        assert_eq!(result.to_string_value(), "a%2Fb%3Fc%3Dd");
    }

    #[test]
    fn test_iri_to_uri() {
        let result: XdmValue<()> =
            fn_iri_to_uri(vec![XdmValue::from_string("http://example.com/test")]).unwrap();
        assert_eq!(result.to_string_value(), "http://example.com/test");
    }
}
