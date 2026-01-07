use crate::engine::EvaluationContext;
use crate::error::XPath31Error;
use crate::types::*;
use petty_xpath1::DataSourceNode;

pub fn fn_position<'a, N: DataSourceNode<'a> + Clone>(
    ctx: &EvaluationContext<'a, '_, N>,
) -> Result<XdmValue<N>, XPath31Error> {
    Ok(XdmValue::from_integer(ctx.context_position as i64))
}

pub fn fn_last<'a, N: DataSourceNode<'a> + Clone>(
    ctx: &EvaluationContext<'a, '_, N>,
) -> Result<XdmValue<N>, XPath31Error> {
    Ok(XdmValue::from_integer(ctx.context_size as i64))
}

pub fn fn_local_name<'a, N: DataSourceNode<'a> + Clone>(
    mut args: Vec<XdmValue<N>>,
    ctx: &EvaluationContext<'a, '_, N>,
) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() > 1 {
        return Err(XPath31Error::function(
            "local-name",
            "Expected 0 or 1 arguments",
        ));
    }

    let node = if args.is_empty() {
        match &ctx.context_item {
            Some(XdmItem::Node(n)) => Some(*n),
            _ => None,
        }
    } else {
        let arg = args.remove(0);
        if arg.is_empty() {
            return Ok(XdmValue::from_string(String::new()));
        }
        match arg.first() {
            Some(XdmItem::Node(n)) => Some(*n),
            _ => None,
        }
    };

    match node {
        Some(n) => {
            let local = n
                .name()
                .map(|q| q.local_part.to_string())
                .unwrap_or_default();
            Ok(XdmValue::from_string(local))
        }
        None => Ok(XdmValue::from_string(String::new())),
    }
}

pub fn fn_namespace_uri<'a, N: DataSourceNode<'a> + Clone>(
    mut args: Vec<XdmValue<N>>,
    ctx: &EvaluationContext<'a, '_, N>,
) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() > 1 {
        return Err(XPath31Error::function(
            "namespace-uri",
            "Expected 0 or 1 arguments",
        ));
    }

    let node = if args.is_empty() {
        match &ctx.context_item {
            Some(XdmItem::Node(n)) => Some(*n),
            _ => None,
        }
    } else {
        let arg = args.remove(0);
        if arg.is_empty() {
            return Ok(XdmValue::from_string(String::new()));
        }
        match arg.first() {
            Some(XdmItem::Node(n)) => Some(*n),
            _ => None,
        }
    };

    match node {
        Some(_n) => Ok(XdmValue::from_string(String::new())),
        None => Ok(XdmValue::from_string(String::new())),
    }
}

pub fn fn_name<'a, N: DataSourceNode<'a> + Clone>(
    mut args: Vec<XdmValue<N>>,
    ctx: &EvaluationContext<'a, '_, N>,
) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() > 1 {
        return Err(XPath31Error::function("name", "Expected 0 or 1 arguments"));
    }

    let node = if args.is_empty() {
        match &ctx.context_item {
            Some(XdmItem::Node(n)) => Some(*n),
            _ => None,
        }
    } else {
        let arg = args.remove(0);
        if arg.is_empty() {
            return Ok(XdmValue::from_string(String::new()));
        }
        match arg.first() {
            Some(XdmItem::Node(n)) => Some(*n),
            _ => None,
        }
    };

    match node {
        Some(n) => {
            let name = match n.name() {
                Some(q) => {
                    if let Some(prefix) = q.prefix {
                        format!("{}:{}", prefix, q.local_part)
                    } else {
                        q.local_part.to_string()
                    }
                }
                None => String::new(),
            };
            Ok(XdmValue::from_string(name))
        }
        None => Ok(XdmValue::from_string(String::new())),
    }
}

pub fn fn_root<'a, N: DataSourceNode<'a> + Clone>(
    mut args: Vec<XdmValue<N>>,
    ctx: &EvaluationContext<'a, '_, N>,
) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() > 1 {
        return Err(XPath31Error::function("root", "Expected 0 or 1 arguments"));
    }

    let node = if args.is_empty() {
        match &ctx.context_item {
            Some(XdmItem::Node(n)) => Some(*n),
            _ => None,
        }
    } else {
        let arg = args.remove(0);
        if arg.is_empty() {
            return Ok(XdmValue::from_string(String::new()));
        }
        match arg.first() {
            Some(XdmItem::Node(n)) => Some(*n),
            _ => None,
        }
    };

    match node {
        Some(n) => {
            let mut current = n;
            while let Some(parent) = current.parent() {
                current = parent;
            }
            Ok(XdmValue::from_node(current))
        }
        None => Ok(XdmValue::empty()),
    }
}

pub fn fn_generate_id<'a, N: DataSourceNode<'a> + Clone + std::hash::Hash>(
    mut args: Vec<XdmValue<N>>,
    ctx: &EvaluationContext<'a, '_, N>,
) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() > 1 {
        return Err(XPath31Error::function(
            "generate-id",
            "Expected 0 or 1 arguments",
        ));
    }

    let node = if args.is_empty() {
        match &ctx.context_item {
            Some(XdmItem::Node(n)) => Some(*n),
            _ => None,
        }
    } else {
        let arg = args.remove(0);
        if arg.is_empty() {
            return Ok(XdmValue::from_string(String::new()));
        }
        match arg.first() {
            Some(XdmItem::Node(n)) => Some(*n),
            _ => None,
        }
    };

    match node {
        Some(n) => {
            use std::hash::Hasher;
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            n.hash(&mut hasher);
            let hash = hasher.finish();
            Ok(XdmValue::from_string(format!("id_{:x}", hash)))
        }
        None => Ok(XdmValue::from_string(String::new())),
    }
}

pub fn fn_error<N: Clone>(args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    let code = args
        .first()
        .map(|v| v.to_string_value())
        .unwrap_or_default();
    let description = args.get(1).map(|v| v.to_string_value()).unwrap_or_default();

    let msg = if code.is_empty() && description.is_empty() {
        "FOER0000: Error raised by fn:error".to_string()
    } else if description.is_empty() {
        format!("{}: Error raised by fn:error", code)
    } else {
        format!("{}: {}", code, description)
    };

    Err(XPath31Error::dynamic_error(msg))
}

pub fn fn_trace<N: Clone>(mut args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if args.is_empty() || args.len() > 2 {
        return Err(XPath31Error::function("trace", "Expected 1 or 2 arguments"));
    }

    let value = args.remove(0);
    let _label = if args.is_empty() {
        String::new()
    } else {
        args.remove(0).to_string_value()
    };

    Ok(value)
}

pub fn fn_data<'a, N: DataSourceNode<'a> + Clone>(
    mut args: Vec<XdmValue<N>>,
    ctx: &EvaluationContext<'a, '_, N>,
) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() > 1 {
        return Err(XPath31Error::function("data", "Expected 0 or 1 arguments"));
    }

    let seq = if args.is_empty() {
        match &ctx.context_item {
            Some(item) => XdmValue::from_item(item.clone()),
            None => return Ok(XdmValue::empty()),
        }
    } else {
        args.remove(0)
    };

    let atomized: Vec<XdmItem<N>> = seq
        .items()
        .iter()
        .filter_map(|item| match item {
            XdmItem::Atomic(a) => Some(XdmItem::Atomic(a.clone())),
            XdmItem::Node(n) => Some(XdmItem::Atomic(AtomicValue::String(n.string_value()))),
            _ => None,
        })
        .collect();

    Ok(XdmValue::from_items(atomized))
}

pub fn fn_node_name<'a, N: DataSourceNode<'a> + Clone>(
    mut args: Vec<XdmValue<N>>,
    ctx: &EvaluationContext<'a, '_, N>,
) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() > 1 {
        return Err(XPath31Error::function(
            "node-name",
            "Expected 0 or 1 arguments",
        ));
    }

    let node = if args.is_empty() {
        match &ctx.context_item {
            Some(XdmItem::Node(n)) => Some(*n),
            _ => None,
        }
    } else {
        let arg = args.remove(0);
        if arg.is_empty() {
            return Ok(XdmValue::empty());
        }
        match arg.first() {
            Some(XdmItem::Node(n)) => Some(*n),
            _ => None,
        }
    };

    match node {
        Some(n) => match n.name() {
            Some(qname) => Ok(XdmValue::from_atomic(AtomicValue::QName {
                prefix: qname.prefix.map(|s| s.to_string()),
                local: qname.local_part.to_string(),
                namespace: None,
            })),
            None => Ok(XdmValue::empty()),
        },
        None => Ok(XdmValue::empty()),
    }
}

pub fn fn_nilled<'a, N: DataSourceNode<'a> + Clone>(
    mut args: Vec<XdmValue<N>>,
    ctx: &EvaluationContext<'a, '_, N>,
) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() > 1 {
        return Err(XPath31Error::function(
            "nilled",
            "Expected 0 or 1 arguments",
        ));
    }

    let node = if args.is_empty() {
        match &ctx.context_item {
            Some(XdmItem::Node(n)) => Some(*n),
            _ => None,
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
            use petty_xpath1::NodeType;
            if n.node_type() == NodeType::Element {
                Ok(XdmValue::from_bool(false))
            } else {
                Ok(XdmValue::empty())
            }
        }
        None => Ok(XdmValue::empty()),
    }
}

pub fn fn_in_scope_prefixes<'a, N: DataSourceNode<'a> + Clone>(
    mut args: Vec<XdmValue<N>>,
) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 1 {
        return Err(XPath31Error::function(
            "in-scope-prefixes",
            "Expected 1 argument",
        ));
    }

    let arg = args.remove(0);
    if arg.is_empty() {
        return Ok(XdmValue::empty());
    }

    match arg.first() {
        Some(XdmItem::Node(_n)) => {
            let prefixes = vec![
                XdmItem::Atomic(AtomicValue::String("xml".to_string())),
                XdmItem::Atomic(AtomicValue::String(String::new())),
            ];
            Ok(XdmValue::from_items(prefixes))
        }
        _ => Ok(XdmValue::empty()),
    }
}

pub fn fn_namespace_uri_for_prefix<'a, N: DataSourceNode<'a> + Clone>(
    mut args: Vec<XdmValue<N>>,
) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 2 {
        return Err(XPath31Error::function(
            "namespace-uri-for-prefix",
            "Expected 2 arguments",
        ));
    }

    let prefix = args.remove(0).to_string_value();
    let _element = args.remove(0);

    if prefix == "xml" {
        return Ok(XdmValue::from_string(
            "http://www.w3.org/XML/1998/namespace".to_string(),
        ));
    }

    Ok(XdmValue::empty())
}

pub fn fn_qname<N: Clone>(mut args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 2 {
        return Err(XPath31Error::function("QName", "Expected 2 arguments"));
    }

    let qname_str = args.remove(1).to_string_value();
    let namespace = args.remove(0);

    let namespace_uri = if namespace.is_empty() {
        None
    } else {
        let uri = namespace.to_string_value();
        if uri.is_empty() { None } else { Some(uri) }
    };

    let (prefix, local) = if let Some(colon_pos) = qname_str.find(':') {
        let prefix = qname_str[..colon_pos].to_string();
        let local = qname_str[colon_pos + 1..].to_string();

        if namespace_uri.is_none() && !prefix.is_empty() {
            return Err(XPath31Error::function(
                "QName",
                "Prefix requires a non-empty namespace URI",
            ));
        }

        (Some(prefix), local)
    } else {
        (None, qname_str)
    };

    Ok(XdmValue::from_atomic(AtomicValue::QName {
        prefix,
        local,
        namespace: namespace_uri,
    }))
}

pub fn fn_prefix_from_qname<N: Clone>(
    mut args: Vec<XdmValue<N>>,
) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 1 {
        return Err(XPath31Error::function(
            "prefix-from-QName",
            "Expected 1 argument",
        ));
    }

    let arg = args.remove(0);
    if arg.is_empty() {
        return Ok(XdmValue::empty());
    }

    match arg.first() {
        Some(XdmItem::Atomic(AtomicValue::QName { prefix, .. })) => match prefix {
            Some(p) if !p.is_empty() => Ok(XdmValue::from_string(p.clone())),
            _ => Ok(XdmValue::empty()),
        },
        _ => Err(XPath31Error::type_error(
            "prefix-from-QName requires a QName argument",
        )),
    }
}

pub fn fn_local_name_from_qname<N: Clone>(
    mut args: Vec<XdmValue<N>>,
) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 1 {
        return Err(XPath31Error::function(
            "local-name-from-QName",
            "Expected 1 argument",
        ));
    }

    let arg = args.remove(0);
    if arg.is_empty() {
        return Ok(XdmValue::empty());
    }

    match arg.first() {
        Some(XdmItem::Atomic(AtomicValue::QName { local, .. })) => {
            Ok(XdmValue::from_string(local.clone()))
        }
        _ => Err(XPath31Error::type_error(
            "local-name-from-QName requires a QName argument",
        )),
    }
}

pub fn fn_namespace_uri_from_qname<N: Clone>(
    mut args: Vec<XdmValue<N>>,
) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 1 {
        return Err(XPath31Error::function(
            "namespace-uri-from-QName",
            "Expected 1 argument",
        ));
    }

    let arg = args.remove(0);
    if arg.is_empty() {
        return Ok(XdmValue::empty());
    }

    match arg.first() {
        Some(XdmItem::Atomic(AtomicValue::QName { namespace, .. })) => match namespace {
            Some(ns) => Ok(XdmValue::from_string(ns.clone())),
            None => Ok(XdmValue::from_string(String::new())),
        },
        _ => Err(XPath31Error::type_error(
            "namespace-uri-from-QName requires a QName argument",
        )),
    }
}

pub fn fn_lang<'a, N: DataSourceNode<'a> + Clone>(
    mut args: Vec<XdmValue<N>>,
    ctx: &EvaluationContext<'a, '_, N>,
) -> Result<XdmValue<N>, XPath31Error> {
    if args.is_empty() || args.len() > 2 {
        return Err(XPath31Error::function("lang", "Expected 1 or 2 arguments"));
    }

    let test_lang = args.remove(0).to_string_value().to_lowercase();

    let node = if args.is_empty() {
        match &ctx.context_item {
            Some(XdmItem::Node(n)) => Some(*n),
            _ => None,
        }
    } else {
        let arg = args.remove(0);
        match arg.first() {
            Some(XdmItem::Node(n)) => Some(*n),
            _ => None,
        }
    };

    match node {
        Some(n) => {
            let mut current = Some(n);
            while let Some(node) = current {
                for attr in node.attributes() {
                    if let Some(name) = attr.name()
                        && name.local_part == "lang"
                        && (name.prefix == Some("xml") || name.prefix.is_none())
                    {
                        let lang = attr.string_value().to_lowercase();
                        let matches =
                            lang == test_lang || lang.starts_with(&format!("{}-", test_lang));
                        return Ok(XdmValue::from_bool(matches));
                    }
                }
                current = node.parent();
            }
            Ok(XdmValue::from_bool(false))
        }
        None => Ok(XdmValue::from_bool(false)),
    }
}

// SPEC: fn:id requires DTD/Schema ID type info - returns empty without it
pub fn fn_id<'a, N: DataSourceNode<'a> + Clone>(
    args: Vec<XdmValue<N>>,
    _ctx: &EvaluationContext<'a, '_, N>,
) -> Result<XdmValue<N>, XPath31Error> {
    if args.is_empty() || args.len() > 2 {
        return Err(XPath31Error::function("id", "Expected 1 or 2 arguments"));
    }
    Ok(XdmValue::empty())
}

// SPEC: fn:idref requires DTD/Schema IDREF type info - returns empty without it
pub fn fn_idref<'a, N: DataSourceNode<'a> + Clone>(
    args: Vec<XdmValue<N>>,
    _ctx: &EvaluationContext<'a, '_, N>,
) -> Result<XdmValue<N>, XPath31Error> {
    if args.is_empty() || args.len() > 2 {
        return Err(XPath31Error::function("idref", "Expected 1 or 2 arguments"));
    }
    Ok(XdmValue::empty())
}

// SPEC: fn:element-with-id is XPath 3.1 version of fn:id - same limitation
pub fn fn_element_with_id<'a, N: DataSourceNode<'a> + Clone>(
    args: Vec<XdmValue<N>>,
    _ctx: &EvaluationContext<'a, '_, N>,
) -> Result<XdmValue<N>, XPath31Error> {
    if args.is_empty() || args.len() > 2 {
        return Err(XPath31Error::function(
            "element-with-id",
            "Expected 1 or 2 arguments",
        ));
    }
    Ok(XdmValue::empty())
}

pub fn fn_system_property<N: Clone>(
    mut args: Vec<XdmValue<N>>,
) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 1 {
        return Err(XPath31Error::function(
            "system-property",
            "Expected 1 argument",
        ));
    }

    let property_name = args.remove(0).to_string_value();
    let local_name = property_name.strip_prefix("xsl:").unwrap_or(&property_name);

    let value = match local_name {
        "version" => "3.0",
        "vendor" => "Petty PDF Engine",
        "vendor-url" => "https://github.com/nickkuk/petty",
        "product-name" => "Petty",
        "product-version" => env!("CARGO_PKG_VERSION"),
        "is-schema-aware" => "no",
        "supports-serialization" => "yes",
        "supports-backwards-compatibility" => "yes",
        "supports-namespace-axis" => "no",
        _ => "",
    };

    Ok(XdmValue::from_string(value.to_string()))
}

pub fn fn_environment_variable<N: Clone>(
    mut args: Vec<XdmValue<N>>,
) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 1 {
        return Err(XPath31Error::function(
            "environment-variable",
            "Expected 1 argument",
        ));
    }
    let _name = args.remove(0).to_string_value();
    // WASM-safe: return empty sequence (env vars not available in browser)
    Ok(XdmValue::empty())
}

pub fn fn_available_environment_variables<N: Clone>(
    args: Vec<XdmValue<N>>,
) -> Result<XdmValue<N>, XPath31Error> {
    if !args.is_empty() {
        return Err(XPath31Error::function(
            "available-environment-variables",
            "Expected 0 arguments",
        ));
    }
    // WASM-safe: return empty sequence
    Ok(XdmValue::empty())
}

pub fn fn_random_number_generator<N: Clone>(
    args: Vec<XdmValue<N>>,
) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() > 1 {
        return Err(XPath31Error::function(
            "random-number-generator",
            "Expected 0 or 1 arguments",
        ));
    }

    use std::hash::{Hash, Hasher};
    let seed: u64 = if args.is_empty() {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        "default-seed".hash(&mut hasher);
        hasher.finish()
    } else {
        let seed_str = args[0].to_string_value();
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        seed_str.hash(&mut hasher);
        hasher.finish()
    };

    let number = (seed as f64) / (u64::MAX as f64);

    let next_fn = XdmFunction::builtin("random-number-generator", 0);
    let permute_fn = XdmFunction::builtin("random-number-generator-permute", 1);

    let map = XdmMap::from_entries(vec![
        (
            AtomicValue::String("number".to_string()),
            XdmValue::from_double(number),
        ),
        (
            AtomicValue::String("next".to_string()),
            XdmValue::from_function(next_fn),
        ),
        (
            AtomicValue::String("permute".to_string()),
            XdmValue::from_function(permute_fn),
        ),
    ]);

    Ok(XdmValue::from_map(map))
}

pub fn fn_random_number_generator_permute<N: Clone>(
    mut args: Vec<XdmValue<N>>,
) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 1 {
        return Err(XPath31Error::function(
            "random-number-generator-permute",
            "Expected 1 argument",
        ));
    }

    let input = args.remove(0);
    let mut items: Vec<XdmItem<N>> = input.into_items();

    use std::hash::{Hash, Hasher};
    let mut seed = {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        items.len().hash(&mut hasher);
        hasher.finish()
    };

    for i in (1..items.len()).rev() {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let j = (seed as usize) % (i + 1);
        items.swap(i, j);
    }

    Ok(XdmValue::from_items(items))
}

pub fn fn_doc<N: Clone>(args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 1 {
        return Err(XPath31Error::function("doc", "Expected 1 argument"));
    }
    if args[0].is_empty() {
        return Ok(XdmValue::empty());
    }
    let uri = args[0].to_string_value();
    Err(XPath31Error::function(
        "doc",
        format!(
            "External document loading not available (requested: {})",
            uri
        ),
    ))
}

pub fn fn_doc_available<N: Clone>(args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 1 {
        return Err(XPath31Error::function(
            "doc-available",
            "Expected 1 argument",
        ));
    }
    Ok(XdmValue::from_bool(false))
}

pub fn fn_collection<N: Clone>(args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() > 1 {
        return Err(XPath31Error::function(
            "collection",
            "Expected 0 or 1 arguments",
        ));
    }
    Ok(XdmValue::empty())
}

pub fn fn_uri_collection<N: Clone>(args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() > 1 {
        return Err(XPath31Error::function(
            "uri-collection",
            "Expected 0 or 1 arguments",
        ));
    }
    Ok(XdmValue::empty())
}

pub fn fn_unparsed_text<N: Clone>(args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if args.is_empty() || args.len() > 2 {
        return Err(XPath31Error::function(
            "unparsed-text",
            "Expected 1 or 2 arguments",
        ));
    }
    let uri = args[0].to_string_value();
    Err(XPath31Error::function(
        "unparsed-text",
        format!("External text loading not available (requested: {})", uri),
    ))
}

pub fn fn_unparsed_text_available<N: Clone>(
    args: Vec<XdmValue<N>>,
) -> Result<XdmValue<N>, XPath31Error> {
    if args.is_empty() || args.len() > 2 {
        return Err(XPath31Error::function(
            "unparsed-text-available",
            "Expected 1 or 2 arguments",
        ));
    }
    Ok(XdmValue::from_bool(false))
}

pub fn fn_unparsed_text_lines<N: Clone>(
    args: Vec<XdmValue<N>>,
) -> Result<XdmValue<N>, XPath31Error> {
    if args.is_empty() || args.len() > 2 {
        return Err(XPath31Error::function(
            "unparsed-text-lines",
            "Expected 1 or 2 arguments",
        ));
    }
    let uri = args[0].to_string_value();
    Err(XPath31Error::function(
        "unparsed-text-lines",
        format!("External text loading not available (requested: {})", uri),
    ))
}

pub fn fn_parse_xml<N: Clone>(args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 1 {
        return Err(XPath31Error::function("parse-xml", "Expected 1 argument"));
    }
    Err(XPath31Error::function(
        "parse-xml",
        "XML parsing not available in this context",
    ))
}

pub fn fn_parse_xml_fragment<N: Clone>(
    args: Vec<XdmValue<N>>,
) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 1 {
        return Err(XPath31Error::function(
            "parse-xml-fragment",
            "Expected 1 argument",
        ));
    }
    Err(XPath31Error::function(
        "parse-xml-fragment",
        "XML parsing not available in this context",
    ))
}

pub fn fn_resolve_qname<'a, N: DataSourceNode<'a> + Clone>(
    mut args: Vec<XdmValue<N>>,
) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 2 {
        return Err(XPath31Error::function(
            "resolve-QName",
            "Expected 2 arguments",
        ));
    }

    let element_arg = args.remove(1);
    let qname_arg = args.remove(0);

    if qname_arg.is_empty() {
        return Ok(XdmValue::empty());
    }

    let qname_str = qname_arg.to_string_value();

    let _element = match element_arg.first() {
        Some(XdmItem::Node(n)) => {
            use petty_xpath1::NodeType;
            if n.node_type() != NodeType::Element {
                return Err(XPath31Error::type_error(
                    "resolve-QName requires an element node as second argument",
                ));
            }
            n
        }
        _ => {
            return Err(XPath31Error::type_error(
                "resolve-QName requires an element node as second argument",
            ));
        }
    };

    let (prefix, local) = if let Some(colon_pos) = qname_str.find(':') {
        let prefix = qname_str[..colon_pos].to_string();
        let local = qname_str[colon_pos + 1..].to_string();
        (Some(prefix), local)
    } else {
        (None, qname_str)
    };

    let namespace = match prefix.as_deref() {
        Some("xml") => Some("http://www.w3.org/XML/1998/namespace".to_string()),
        Some("xs") => Some("http://www.w3.org/2001/XMLSchema".to_string()),
        Some("xsl") => Some("http://www.w3.org/1999/XSL/Transform".to_string()),
        Some("fn") => Some("http://www.w3.org/2005/xpath-functions".to_string()),
        None | Some(_) => None,
    };

    Ok(XdmValue::from_atomic(AtomicValue::QName {
        prefix,
        local,
        namespace,
    }))
}

pub fn fn_has_children<'a, N: DataSourceNode<'a> + Clone>(
    mut args: Vec<XdmValue<N>>,
    ctx: &EvaluationContext<'a, '_, N>,
) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() > 1 {
        return Err(XPath31Error::function(
            "has-children",
            "Expected 0 or 1 arguments",
        ));
    }

    let node = if args.is_empty() {
        match &ctx.context_item {
            Some(XdmItem::Node(n)) => Some(*n),
            _ => return Ok(XdmValue::from_bool(false)),
        }
    } else {
        let arg = args.remove(0);
        if arg.is_empty() {
            return Ok(XdmValue::from_bool(false));
        }
        match arg.first() {
            Some(XdmItem::Node(n)) => Some(*n),
            _ => return Ok(XdmValue::from_bool(false)),
        }
    };

    match node {
        Some(n) => {
            let has_children = n.children().next().is_some();
            Ok(XdmValue::from_bool(has_children))
        }
        None => Ok(XdmValue::from_bool(false)),
    }
}

pub fn fn_serialize<'a, N: DataSourceNode<'a> + Clone>(
    args: Vec<XdmValue<N>>,
) -> Result<XdmValue<N>, XPath31Error> {
    if args.is_empty() || args.len() > 2 {
        return Err(XPath31Error::function(
            "serialize",
            "Expected 1 or 2 arguments",
        ));
    }

    let mut result = String::new();
    for item in args[0].items() {
        match item {
            XdmItem::Node(n) => {
                result.push_str(&n.string_value());
            }
            XdmItem::Atomic(a) => {
                result.push_str(&a.to_string_value());
            }
            XdmItem::Map(m) => {
                result.push_str(&format!("{}", m));
            }
            XdmItem::Array(a) => {
                result.push_str(&format!("{}", a));
            }
            XdmItem::Function(f) => {
                result.push_str(&format!("{}", f));
            }
        }
    }

    Ok(XdmValue::from_string(result))
}

pub fn fn_path<'a, N: DataSourceNode<'a> + Clone>(
    mut args: Vec<XdmValue<N>>,
    ctx: &EvaluationContext<'a, '_, N>,
) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() > 1 {
        return Err(XPath31Error::function("path", "Expected 0 or 1 arguments"));
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
            let path = build_path(n);
            Ok(XdmValue::from_string(path))
        }
        None => Ok(XdmValue::empty()),
    }
}

fn build_path<'a, N: DataSourceNode<'a> + Clone>(node: N) -> String {
    use petty_xpath1::NodeType;

    let mut steps: Vec<String> = Vec::new();
    let mut current = Some(node);

    while let Some(n) = current {
        let step = match n.node_type() {
            NodeType::Root => {
                steps.push(String::new());
                break;
            }
            NodeType::Element => {
                if let Some(name) = n.name() {
                    let local = &name.local_part;
                    let position = compute_sibling_position(&n);
                    if let Some(prefix) = name.prefix {
                        format!("Q{{{}}}{local}[{position}]", prefix)
                    } else {
                        format!("{local}[{position}]")
                    }
                } else {
                    "*[1]".to_string()
                }
            }
            NodeType::Attribute => {
                if let Some(name) = n.name() {
                    format!("@{}", name.local_part)
                } else {
                    "@*".to_string()
                }
            }
            NodeType::Text => {
                let position = compute_text_position(&n);
                format!("text()[{position}]")
            }
            NodeType::Comment => {
                let position = compute_comment_position(&n);
                format!("comment()[{position}]")
            }
            NodeType::ProcessingInstruction => {
                if let Some(name) = n.name() {
                    format!("processing-instruction({})[1]", name.local_part)
                } else {
                    "processing-instruction()[1]".to_string()
                }
            }
        };
        steps.push(step);
        current = n.parent();
    }

    steps.reverse();
    if steps.is_empty() || (steps.len() == 1 && steps[0].is_empty()) {
        "/".to_string()
    } else {
        steps.join("/")
    }
}

fn compute_sibling_position<'a, N: DataSourceNode<'a> + Clone>(node: &N) -> usize {
    use petty_xpath1::NodeType;

    let name = node.name();
    let mut position = 1;

    if let Some(parent) = node.parent() {
        for child in parent.children() {
            if std::ptr::eq(
                &child as *const _ as *const (),
                node as *const _ as *const (),
            ) {
                break;
            }
            if child.node_type() == NodeType::Element
                && let (Some(child_name), Some(node_name)) = (child.name(), name.as_ref())
                && child_name.local_part == node_name.local_part
                && child_name.prefix == node_name.prefix
            {
                position += 1;
            }
        }
    }
    position
}

fn compute_text_position<'a, N: DataSourceNode<'a> + Clone>(node: &N) -> usize {
    use petty_xpath1::NodeType;

    let mut position = 1;
    if let Some(parent) = node.parent() {
        for child in parent.children() {
            if std::ptr::eq(
                &child as *const _ as *const (),
                node as *const _ as *const (),
            ) {
                break;
            }
            if child.node_type() == NodeType::Text {
                position += 1;
            }
        }
    }
    position
}

fn compute_comment_position<'a, N: DataSourceNode<'a> + Clone>(node: &N) -> usize {
    use petty_xpath1::NodeType;

    let mut position = 1;
    if let Some(parent) = node.parent() {
        for child in parent.children() {
            if std::ptr::eq(
                &child as *const _ as *const (),
                node as *const _ as *const (),
            ) {
                break;
            }
            if child.node_type() == NodeType::Comment {
                position += 1;
            }
        }
    }
    position
}

pub fn map_size<N: Clone>(mut args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 1 {
        return Err(XPath31Error::function("map:size", "Expected 1 argument"));
    }
    let arg = args.remove(0);
    match arg.first() {
        Some(XdmItem::Map(m)) => Ok(XdmValue::from_integer(m.size() as i64)),
        _ => Err(XPath31Error::type_error("map:size requires a map argument")),
    }
}

pub fn map_keys<N: Clone>(mut args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 1 {
        return Err(XPath31Error::function("map:keys", "Expected 1 argument"));
    }
    let arg = args.remove(0);
    match arg.first() {
        Some(XdmItem::Map(m)) => {
            let keys: Vec<XdmItem<N>> = m.keys().map(|k| XdmItem::Atomic(k.clone())).collect();
            Ok(XdmValue::from_items(keys))
        }
        _ => Err(XPath31Error::type_error("map:keys requires a map argument")),
    }
}

pub fn map_contains<N: Clone>(mut args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 2 {
        return Err(XPath31Error::function(
            "map:contains",
            "Expected 2 arguments",
        ));
    }
    let key = args.remove(1);
    let map_val = args.remove(0);

    let key_atomic = match key.first() {
        Some(XdmItem::Atomic(a)) => a.clone(),
        _ => return Err(XPath31Error::type_error("map:contains key must be atomic")),
    };

    match map_val.first() {
        Some(XdmItem::Map(m)) => Ok(XdmValue::from_bool(m.contains_key(&key_atomic))),
        _ => Err(XPath31Error::type_error(
            "map:contains requires a map argument",
        )),
    }
}

pub fn map_get<N: Clone>(mut args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 2 {
        return Err(XPath31Error::function("map:get", "Expected 2 arguments"));
    }
    let key = args.remove(1);
    let map_val = args.remove(0);

    let key_atomic = match key.first() {
        Some(XdmItem::Atomic(a)) => a.clone(),
        _ => return Err(XPath31Error::type_error("map:get key must be atomic")),
    };

    match map_val.first() {
        Some(XdmItem::Map(m)) => match m.get(&key_atomic) {
            Some(v) => Ok(v.clone()),
            None => Ok(XdmValue::empty()),
        },
        _ => Err(XPath31Error::type_error("map:get requires a map argument")),
    }
}

pub fn map_put<N: Clone>(mut args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 3 {
        return Err(XPath31Error::function("map:put", "Expected 3 arguments"));
    }
    let value = args.remove(2);
    let key = args.remove(1);
    let map_val = args.remove(0);

    let key_atomic = match key.first() {
        Some(XdmItem::Atomic(a)) => a.clone(),
        _ => return Err(XPath31Error::type_error("map:put key must be atomic")),
    };

    match map_val.first() {
        Some(XdmItem::Map(m)) => {
            let new_map = m.put(key_atomic, value);
            Ok(XdmValue::from_map(new_map))
        }
        _ => Err(XPath31Error::type_error("map:put requires a map argument")),
    }
}

pub fn map_remove<N: Clone>(mut args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 2 {
        return Err(XPath31Error::function("map:remove", "Expected 2 arguments"));
    }
    let keys = args.remove(1);
    let map_val = args.remove(0);

    match map_val.first() {
        Some(XdmItem::Map(m)) => {
            let mut result = m.clone();
            for item in keys.items() {
                if let XdmItem::Atomic(a) = item {
                    result = result.remove(a);
                }
            }
            Ok(XdmValue::from_map(result))
        }
        _ => Err(XPath31Error::type_error(
            "map:remove requires a map argument",
        )),
    }
}

pub fn map_entry<N: Clone>(mut args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 2 {
        return Err(XPath31Error::function("map:entry", "Expected 2 arguments"));
    }
    let value = args.remove(1);
    let key = args.remove(0);

    let key_atomic = match key.first() {
        Some(XdmItem::Atomic(a)) => a.clone(),
        _ => return Err(XPath31Error::type_error("map:entry key must be atomic")),
    };

    let map = XdmMap::from_entries(vec![(key_atomic, value)]);
    Ok(XdmValue::from_map(map))
}

pub fn map_merge<N: Clone>(args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if args.is_empty() || args.len() > 2 {
        return Err(XPath31Error::function(
            "map:merge",
            "Expected 1 or 2 arguments",
        ));
    }

    let duplicates_mode = if args.len() == 2 {
        get_duplicates_option(&args[1])?
    } else {
        DuplicatesMode::UseFirst
    };

    let maps_seq = &args[0];
    let mut result_entries: indexmap::IndexMap<AtomicValue, XdmValue<N>> =
        indexmap::IndexMap::new();

    for item in maps_seq.items() {
        if let XdmItem::Map(m) = item {
            for (key, value) in m.entries() {
                if let Some(existing) = result_entries.get(key) {
                    match duplicates_mode {
                        DuplicatesMode::UseFirst => {}
                        DuplicatesMode::UseLast => {
                            result_entries.insert(key.clone(), value.clone());
                        }
                        DuplicatesMode::Combine => {
                            let mut combined_items = existing.items().to_vec();
                            combined_items.extend(value.items().to_vec());
                            result_entries
                                .insert(key.clone(), XdmValue::from_items(combined_items));
                        }
                        DuplicatesMode::Reject => {
                            return Err(XPath31Error::function(
                                "map:merge",
                                format!("Duplicate key: {}", key.to_string_value()),
                            ));
                        }
                        DuplicatesMode::UseAny => {}
                    }
                } else {
                    result_entries.insert(key.clone(), value.clone());
                }
            }
        }
    }

    let entries: Vec<(AtomicValue, XdmValue<N>)> = result_entries.into_iter().collect();
    Ok(XdmValue::from_map(XdmMap::from_entries(entries)))
}

#[derive(Clone, Copy)]
enum DuplicatesMode {
    UseFirst,
    UseLast,
    Combine,
    Reject,
    UseAny,
}

fn get_duplicates_option<N: Clone>(options: &XdmValue<N>) -> Result<DuplicatesMode, XPath31Error> {
    if let Some(XdmItem::Map(opts_map)) = options.first() {
        let key = AtomicValue::String("duplicates".to_string());
        if let Some(dup_value) = opts_map.get(&key) {
            let mode_str = dup_value.to_string_value();
            return match mode_str.as_str() {
                "use-first" => Ok(DuplicatesMode::UseFirst),
                "use-last" => Ok(DuplicatesMode::UseLast),
                "combine" => Ok(DuplicatesMode::Combine),
                "reject" => Ok(DuplicatesMode::Reject),
                "use-any" => Ok(DuplicatesMode::UseAny),
                _ => Err(XPath31Error::function(
                    "map:merge",
                    format!("Invalid duplicates option: {}", mode_str),
                )),
            };
        }
    }
    Ok(DuplicatesMode::UseFirst)
}

pub fn array_size<N: Clone>(mut args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 1 {
        return Err(XPath31Error::function("array:size", "Expected 1 argument"));
    }
    let arg = args.remove(0);
    match arg.first() {
        Some(XdmItem::Array(a)) => Ok(XdmValue::from_integer(a.size() as i64)),
        _ => Err(XPath31Error::type_error(
            "array:size requires an array argument",
        )),
    }
}

pub fn array_get<N: Clone>(mut args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 2 {
        return Err(XPath31Error::function("array:get", "Expected 2 arguments"));
    }
    let index = args.remove(1);
    let arr_val = args.remove(0);

    let idx = index.to_double() as usize;
    if idx == 0 {
        return Err(XPath31Error::ArrayIndexOutOfBounds { index: 0, size: 0 });
    }

    match arr_val.first() {
        Some(XdmItem::Array(a)) => match a.get(idx) {
            Some(v) => Ok(v.clone()),
            None => Err(XPath31Error::ArrayIndexOutOfBounds {
                index: idx as i64,
                size: a.size(),
            }),
        },
        _ => Err(XPath31Error::type_error(
            "array:get requires an array argument",
        )),
    }
}

pub fn array_put<N: Clone>(mut args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 3 {
        return Err(XPath31Error::function("array:put", "Expected 3 arguments"));
    }
    let value = args.remove(2);
    let index = args.remove(1);
    let arr_val = args.remove(0);

    let idx = index.to_double() as usize;

    match arr_val.first() {
        Some(XdmItem::Array(a)) => match a.put(idx, value) {
            Some(new_arr) => Ok(XdmValue::from_array(new_arr)),
            None => Err(XPath31Error::ArrayIndexOutOfBounds {
                index: idx as i64,
                size: a.size(),
            }),
        },
        _ => Err(XPath31Error::type_error(
            "array:put requires an array argument",
        )),
    }
}

pub fn array_append<N: Clone>(mut args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 2 {
        return Err(XPath31Error::function(
            "array:append",
            "Expected 2 arguments",
        ));
    }
    let value = args.remove(1);
    let arr_val = args.remove(0);

    match arr_val.first() {
        Some(XdmItem::Array(a)) => Ok(XdmValue::from_array(a.append(value))),
        _ => Err(XPath31Error::type_error(
            "array:append requires an array argument",
        )),
    }
}

pub fn array_head<N: Clone>(mut args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 1 {
        return Err(XPath31Error::function("array:head", "Expected 1 argument"));
    }
    let arr_val = args.remove(0);

    match arr_val.first() {
        Some(XdmItem::Array(a)) => match a.head() {
            Some(v) => Ok(v.clone()),
            None => Err(XPath31Error::function("array:head", "Array is empty")),
        },
        _ => Err(XPath31Error::type_error(
            "array:head requires an array argument",
        )),
    }
}

pub fn array_tail<N: Clone>(mut args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 1 {
        return Err(XPath31Error::function("array:tail", "Expected 1 argument"));
    }
    let arr_val = args.remove(0);

    match arr_val.first() {
        Some(XdmItem::Array(a)) => match a.tail() {
            Some(new_arr) => Ok(XdmValue::from_array(new_arr)),
            None => Err(XPath31Error::function("array:tail", "Array is empty")),
        },
        _ => Err(XPath31Error::type_error(
            "array:tail requires an array argument",
        )),
    }
}

pub fn array_reverse<N: Clone>(mut args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 1 {
        return Err(XPath31Error::function(
            "array:reverse",
            "Expected 1 argument",
        ));
    }
    let arr_val = args.remove(0);

    match arr_val.first() {
        Some(XdmItem::Array(a)) => Ok(XdmValue::from_array(a.reverse())),
        _ => Err(XPath31Error::type_error(
            "array:reverse requires an array argument",
        )),
    }
}

pub fn array_join<N: Clone>(args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 1 {
        return Err(XPath31Error::function("array:join", "Expected 1 argument"));
    }

    let mut arrays = Vec::new();
    for item in args[0].items() {
        if let XdmItem::Array(a) = item {
            arrays.push(a.clone());
        }
    }

    Ok(XdmValue::from_array(XdmArray::join(&arrays)))
}

pub fn array_subarray<N: Clone>(mut args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() < 2 || args.len() > 3 {
        return Err(XPath31Error::function(
            "array:subarray",
            "Expected 2 or 3 arguments",
        ));
    }

    let length = if args.len() == 3 {
        Some(args.remove(2).to_double() as usize)
    } else {
        None
    };
    let start = args.remove(1).to_double() as usize;
    let arr_val = args.remove(0);

    match arr_val.first() {
        Some(XdmItem::Array(a)) => {
            let len = length.unwrap_or(a.size() - start + 1);
            match a.subarray(start, len) {
                Some(new_arr) => Ok(XdmValue::from_array(new_arr)),
                None => Err(XPath31Error::function(
                    "array:subarray",
                    "Invalid start position",
                )),
            }
        }
        _ => Err(XPath31Error::type_error(
            "array:subarray requires an array argument",
        )),
    }
}

pub fn array_remove<N: Clone>(mut args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 2 {
        return Err(XPath31Error::function(
            "array:remove",
            "Expected 2 arguments",
        ));
    }
    let positions = args.remove(1);
    let arr_val = args.remove(0);

    match arr_val.first() {
        Some(XdmItem::Array(a)) => {
            let mut result = a.clone();
            let mut indices: Vec<usize> = positions
                .items()
                .iter()
                .filter_map(|i| {
                    if let XdmItem::Atomic(AtomicValue::Integer(n)) = i {
                        Some(*n as usize)
                    } else {
                        None
                    }
                })
                .collect();
            indices.sort_by(|a, b| b.cmp(a));

            for idx in indices {
                if let Some(new_arr) = result.remove(idx) {
                    result = new_arr;
                }
            }
            Ok(XdmValue::from_array(result))
        }
        _ => Err(XPath31Error::type_error(
            "array:remove requires an array argument",
        )),
    }
}

pub fn array_insert_before<N: Clone>(
    mut args: Vec<XdmValue<N>>,
) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 3 {
        return Err(XPath31Error::function(
            "array:insert-before",
            "Expected 3 arguments",
        ));
    }
    let value = args.remove(2);
    let position = args.remove(1).to_double() as usize;
    let arr_val = args.remove(0);

    match arr_val.first() {
        Some(XdmItem::Array(a)) => match a.insert_before(position, value) {
            Some(new_arr) => Ok(XdmValue::from_array(new_arr)),
            None => Err(XPath31Error::function(
                "array:insert-before",
                "Invalid position",
            )),
        },
        _ => Err(XPath31Error::type_error(
            "array:insert-before requires an array argument",
        )),
    }
}

pub fn array_flatten<N: Clone>(args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    fn flatten_items<N: Clone>(items: &[XdmItem<N>]) -> Vec<XdmItem<N>> {
        let mut result = Vec::new();
        for item in items {
            match item {
                XdmItem::Array(arr) => {
                    for member in arr.members() {
                        result.extend(flatten_items(member.items()));
                    }
                }
                other => result.push(other.clone()),
            }
        }
        result
    }

    let mut all_items = Vec::new();
    for arg in args {
        all_items.extend(flatten_items(arg.items()));
    }
    Ok(XdmValue::from_items(all_items))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_map_operations() {
        let map: XdmMap<()> = XdmMap::from_entries(vec![
            (
                AtomicValue::String("a".to_string()),
                XdmValue::from_integer(1),
            ),
            (
                AtomicValue::String("b".to_string()),
                XdmValue::from_integer(2),
            ),
        ]);
        let map_val = XdmValue::from_map(map);

        let size = map_size(vec![map_val.clone()]).unwrap();
        assert_eq!(size.to_double(), 2.0);

        let keys = map_keys(vec![map_val.clone()]).unwrap();
        assert_eq!(keys.len(), 2);

        let contains = map_contains(vec![map_val.clone(), XdmValue::from_string("a")]).unwrap();
        assert!(contains.effective_boolean_value());

        let get = map_get(vec![map_val.clone(), XdmValue::from_string("a")]).unwrap();
        assert_eq!(get.to_double(), 1.0);
    }

    #[test]
    fn test_array_operations() {
        let arr: XdmArray<()> = XdmArray::from_members(vec![
            XdmValue::from_integer(10),
            XdmValue::from_integer(20),
            XdmValue::from_integer(30),
        ]);
        let arr_val = XdmValue::from_array(arr);

        let size = array_size(vec![arr_val.clone()]).unwrap();
        assert_eq!(size.to_double(), 3.0);

        let get = array_get(vec![arr_val.clone(), XdmValue::from_integer(2)]).unwrap();
        assert_eq!(get.to_double(), 20.0);

        let head = array_head(vec![arr_val.clone()]).unwrap();
        assert_eq!(head.to_double(), 10.0);
    }
}
