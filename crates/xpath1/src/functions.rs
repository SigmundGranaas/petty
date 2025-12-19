//! Defines the registry and built-in implementations for XPath 1.0 functions.

use super::engine::{EvaluationContext, XPathValue};
use crate::datasource::{DataSourceNode, NodeType};
use crate::error::XPathError;
use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, HashSet};
use std::hash::Hasher;

// A simple registry that just holds the names of built-in functions.
pub struct FunctionRegistry {
    functions: HashMap<&'static str, ()>,
}

impl FunctionRegistry {
    pub fn new() -> Self {
        Self {
            functions: HashMap::new(),
        }
    }
    pub fn register(&mut self, name: &'static str) {
        self.functions.insert(name, ());
    }
    pub fn get(&self, name: &str) -> Option<()> {
        self.functions.get(name).copied()
    }
}

/// Dispatches a function call to the correct implementation.
pub fn evaluate_function<'a, 'd, N: DataSourceNode<'a>>(
    name: &str,
    args: Vec<XPathValue<N>>,
    e_ctx: &EvaluationContext<'a, 'd, N>,
) -> Result<XPathValue<N>, XPathError> {
    match name {
        // Core & Node-Set
        "string" => func_string(args, e_ctx),
        "count" => func_count(args),
        "id" => func_id(args, e_ctx),
        "position" => func_position(args, e_ctx),
        "last" => func_last(args, e_ctx),
        "local-name" => func_local_name(args, e_ctx),
        "name" => func_name(args, e_ctx),
        "key" => func_key(args, e_ctx),
        "generate-id" => func_generate_id(args, e_ctx),

        // String
        "concat" => func_concat(args),
        "starts-with" => func_starts_with(args),
        "contains" => func_contains(args),
        "substring-before" => func_substring_before(args),
        "substring-after" => func_substring_after(args),
        "substring" => func_substring(args),
        "string-length" => func_string_length(args, e_ctx),
        "normalize-space" => func_normalize_space(args, e_ctx),
        "translate" => func_translate(args),

        // Boolean
        "not" => func_not(args),
        "true" => func_true(args),
        "false" => func_false(args),
        "lang" => func_lang(args, e_ctx),

        // Number
        "sum" => func_sum(args),
        "floor" => func_floor(args),
        "ceiling" => func_ceiling(args),
        "round" => func_round(args),

        // Petty extension functions
        "petty:index" => func_petty_index(args, e_ctx),

        // "node" is not a real function, but registering it prevents "unknown function" errors
        // when the parser mistakes the node() test for a function call.
        "node" | "comment" | "processing-instruction" => Err(XPathError::FunctionError {
            function: name.to_string(),
            message: "This is a node-test, not a function.".to_string(),
        }),
        _ => Err(XPathError::FunctionError {
            function: name.to_string(),
            message: "Unknown XPath function".to_string(),
        }),
    }
}

// --- Petty Extension Functions ---

fn func_petty_index<'a, 'd, N: DataSourceNode<'a>>(
    _args: Vec<XPathValue<N>>,
    _e_ctx: &EvaluationContext<'a, 'd, N>,
) -> Result<XPathValue<N>, XPathError> {
    // This is a stub function. It is detected at compile-time to flag the template
    // as having an index dependency. The actual indexing happens in the layout engine,
    // and this function's result is not used during the initial template execution pass.
    Ok(XPathValue::NodeSet(vec![]))
}

// --- Core & Node-Set Functions ---

fn func_id<'a, 'd, N: DataSourceNode<'a>>(
    mut args: Vec<XPathValue<N>>,
    e_ctx: &EvaluationContext<'a, 'd, N>,
) -> Result<XPathValue<N>, XPathError> {
    if args.len() != 1 {
        return Err(XPathError::FunctionError {
            function: "id()".to_string(),
            message: "Expected 1 argument".to_string(),
        });
    }

    let id_string = args.remove(0).to_string();
    let ids_to_find: HashSet<_> = id_string.split_whitespace().collect();
    if ids_to_find.is_empty() {
        return Ok(XPathValue::NodeSet(vec![]));
    }

    let mut results = Vec::new();
    let mut seen_nodes = HashSet::new();
    let mut stack = e_ctx.root_node.children().collect::<Vec<_>>();

    while let Some(node) = stack.pop() {
        if node.node_type() == NodeType::Element {
            for attr in node.attributes() {
                if let Some(q_name) = attr.name() {
                    let is_id_attr = (q_name.prefix == Some("xml") || q_name.prefix.is_none())
                        && q_name.local_part == "id";

                    if is_id_attr
                        && ids_to_find.contains(attr.string_value().as_str())
                        && seen_nodes.insert(node)
                    {
                        results.push(node);
                    }
                }
            }
        }
        stack.extend(node.children());
    }

    results.sort();
    Ok(XPathValue::NodeSet(results))
}

fn func_key<'a, 'd, N: DataSourceNode<'a>>(
    mut args: Vec<XPathValue<N>>,
    e_ctx: &EvaluationContext<'a, 'd, N>,
) -> Result<XPathValue<N>, XPathError> {
    if args.len() != 2 {
        return Err(XPathError::FunctionError {
            function: "key()".to_string(),
            message: "Expected 2 arguments".to_string(),
        });
    }

    let key_value_arg = args.remove(1);
    let key_name = args.remove(0).to_string();

    let key_index = match e_ctx.key_indexes.get(&key_name) {
        Some(index) => index,
        None => return Ok(XPathValue::NodeSet(vec![])), // No such key, return empty set
    };

    let key_values = match key_value_arg {
        XPathValue::NodeSet(nodes) => nodes
            .into_iter()
            .map(|n| n.string_value())
            .collect::<Vec<_>>(),
        other => vec![other.to_string()],
    };

    let mut result_nodes = Vec::new();
    let mut seen = std::collections::HashSet::new(); // Avoid duplicates

    for value in key_values {
        if let Some(nodes) = key_index.get(&value) {
            for &node in nodes {
                if seen.insert(node) {
                    result_nodes.push(node);
                }
            }
        }
    }

    result_nodes.sort();
    Ok(XPathValue::NodeSet(result_nodes))
}

fn func_string<'a, 'd, N: DataSourceNode<'a>>(
    mut args: Vec<XPathValue<N>>,
    e_ctx: &EvaluationContext<'a, 'd, N>,
) -> Result<XPathValue<N>, XPathError> {
    if args.len() > 1 {
        return Err(XPathError::FunctionError {
            function: "string()".to_string(),
            message: "Expected 0 or 1 arguments".to_string(),
        });
    }
    let s = if args.is_empty() {
        e_ctx.context_node.string_value()
    } else {
        args.remove(0).to_string()
    };
    Ok(XPathValue::String(s))
}

fn func_count<'a, N: DataSourceNode<'a>>(
    mut args: Vec<XPathValue<N>>,
) -> Result<XPathValue<N>, XPathError> {
    if args.len() != 1 {
        return Err(XPathError::FunctionError {
            function: "count()".to_string(),
            message: "Expected 1 argument".to_string(),
        });
    }
    let count = match args.remove(0) {
        XPathValue::NodeSet(nodes) => nodes.len() as f64,
        v => {
            return Err(XPathError::TypeError(format!(
                "count() argument must be a node-set, got {:?}",
                v
            )));
        }
    };
    Ok(XPathValue::Number(count))
}

fn func_position<'a, 'd, N: DataSourceNode<'a>>(
    args: Vec<XPathValue<N>>,
    e_ctx: &EvaluationContext<'a, 'd, N>,
) -> Result<XPathValue<N>, XPathError> {
    if !args.is_empty() {
        return Err(XPathError::FunctionError {
            function: "position()".to_string(),
            message: "Expected 0 arguments".to_string(),
        });
    }
    Ok(XPathValue::Number(e_ctx.context_position as f64))
}

fn func_last<'a, 'd, N: DataSourceNode<'a>>(
    args: Vec<XPathValue<N>>,
    e_ctx: &EvaluationContext<'a, 'd, N>,
) -> Result<XPathValue<N>, XPathError> {
    if !args.is_empty() {
        return Err(XPathError::FunctionError {
            function: "last()".to_string(),
            message: "Expected 0 arguments".to_string(),
        });
    }
    Ok(XPathValue::Number(e_ctx.context_size as f64))
}

fn func_local_name<'a, 'd, N: DataSourceNode<'a>>(
    mut args: Vec<XPathValue<N>>,
    e_ctx: &EvaluationContext<'a, 'd, N>,
) -> Result<XPathValue<N>, XPathError> {
    if args.len() > 1 {
        return Err(XPathError::FunctionError {
            function: "local-name()".to_string(),
            message: "Expected 0 or 1 arguments".to_string(),
        });
    }
    let node = if args.is_empty() {
        Some(e_ctx.context_node)
    } else {
        match args.remove(0) {
            XPathValue::NodeSet(nodes) => nodes.first().copied(),
            v => {
                return Err(XPathError::TypeError(format!(
                    "local-name() argument must be a node-set, got {:?}",
                    v
                )));
            }
        }
    };
    let name = node
        .and_then(|n| n.name().map(|q| q.local_part.to_string()))
        .unwrap_or_default();
    Ok(XPathValue::String(name))
}

fn func_name<'a, 'd, N: DataSourceNode<'a>>(
    mut args: Vec<XPathValue<N>>,
    e_ctx: &EvaluationContext<'a, 'd, N>,
) -> Result<XPathValue<N>, XPathError> {
    if args.len() > 1 {
        return Err(XPathError::FunctionError {
            function: "name()".to_string(),
            message: "Expected 0 or 1 arguments".to_string(),
        });
    }
    let node = if args.is_empty() {
        Some(e_ctx.context_node)
    } else {
        match args.remove(0) {
            XPathValue::NodeSet(nodes) => nodes.first().copied(),
            v => {
                return Err(XPathError::TypeError(format!(
                    "name() argument must be a node-set, got {:?}",
                    v
                )));
            }
        }
    };
    let name = node
        .and_then(|n| {
            n.name().map(|q| {
                if let Some(prefix) = q.prefix {
                    format!("{}:{}", prefix, q.local_part)
                } else {
                    q.local_part.to_string()
                }
            })
        })
        .unwrap_or_default();
    Ok(XPathValue::String(name))
}

fn func_generate_id<'a, 'd, N: DataSourceNode<'a>>(
    mut args: Vec<XPathValue<N>>,
    e_ctx: &EvaluationContext<'a, 'd, N>,
) -> Result<XPathValue<N>, XPathError> {
    if args.len() > 1 {
        return Err(XPathError::FunctionError {
            function: "generate-id()".to_string(),
            message: "Expected 0 or 1 arguments".to_string(),
        });
    }

    let node_to_id = if args.is_empty() {
        Some(e_ctx.context_node)
    } else {
        match args.remove(0) {
            XPathValue::NodeSet(mut nodes) => {
                if nodes.is_empty() {
                    None
                } else {
                    // The spec requires using the first node in document order.
                    nodes.sort();
                    nodes.first().copied()
                }
            }
            // For non-node-set arguments, behavior is undefined; returning empty is safe.
            _ => None,
        }
    };

    if let Some(node) = node_to_id {
        let mut hasher = DefaultHasher::new();
        node.hash(&mut hasher);
        let id = hasher.finish();
        // Prefix with a letter to ensure it's a valid XML NCName.
        Ok(XPathValue::String(format!("id{}", id)))
    } else {
        // If the node-set is empty, return an empty string.
        Ok(XPathValue::String("".to_string()))
    }
}

// --- String Functions ---

fn func_concat<'a, N: DataSourceNode<'a>>(
    args: Vec<XPathValue<N>>,
) -> Result<XPathValue<N>, XPathError> {
    if args.len() < 2 {
        return Err(XPathError::FunctionError {
            function: "concat()".to_string(),
            message: "Expected at least 2 arguments".to_string(),
        });
    }
    let result = args.iter().map(|v| v.to_string()).collect::<String>();
    Ok(XPathValue::String(result))
}

fn func_starts_with<'a, N: DataSourceNode<'a>>(
    mut args: Vec<XPathValue<N>>,
) -> Result<XPathValue<N>, XPathError> {
    if args.len() != 2 {
        return Err(XPathError::FunctionError {
            function: "starts-with()".to_string(),
            message: "Expected 2 arguments".to_string(),
        });
    }
    let s2 = args.remove(1).to_string();
    let s1 = args.remove(0).to_string();
    Ok(XPathValue::Boolean(s1.starts_with(&s2)))
}

fn func_contains<'a, N: DataSourceNode<'a>>(
    mut args: Vec<XPathValue<N>>,
) -> Result<XPathValue<N>, XPathError> {
    if args.len() != 2 {
        return Err(XPathError::FunctionError {
            function: "contains()".to_string(),
            message: "Expected 2 arguments".to_string(),
        });
    }
    let s2 = args.remove(1).to_string();
    let s1 = args.remove(0).to_string();
    Ok(XPathValue::Boolean(s1.contains(&s2)))
}

fn func_substring_before<'a, N: DataSourceNode<'a>>(
    mut args: Vec<XPathValue<N>>,
) -> Result<XPathValue<N>, XPathError> {
    if args.len() != 2 {
        return Err(XPathError::FunctionError {
            function: "substring-before()".to_string(),
            message: "Expected 2 arguments".to_string(),
        });
    }
    let s2 = args.remove(1).to_string();
    let s1 = args.remove(0).to_string();
    if let Some(index) = s1.find(&s2) {
        Ok(XPathValue::String(s1[..index].to_string()))
    } else {
        Ok(XPathValue::String("".to_string()))
    }
}

fn func_substring_after<'a, N: DataSourceNode<'a>>(
    mut args: Vec<XPathValue<N>>,
) -> Result<XPathValue<N>, XPathError> {
    if args.len() != 2 {
        return Err(XPathError::FunctionError {
            function: "substring-after()".to_string(),
            message: "Expected 2 arguments".to_string(),
        });
    }
    let s2 = args.remove(1).to_string();
    let s1 = args.remove(0).to_string();
    if let Some(index) = s1.find(&s2) {
        Ok(XPathValue::String(s1[index + s2.len()..].to_string()))
    } else {
        Ok(XPathValue::String("".to_string()))
    }
}

fn func_substring<'a, N: DataSourceNode<'a>>(
    mut args: Vec<XPathValue<N>>,
) -> Result<XPathValue<N>, XPathError> {
    if !(2..=3).contains(&args.len()) {
        return Err(XPathError::FunctionError {
            function: "substring()".to_string(),
            message: "Expected 2 or 3 arguments".to_string(),
        });
    }
    let length_val = if args.len() == 3 {
        Some(args.remove(2).to_number())
    } else {
        None
    };
    let start_val = args.remove(1).to_number();
    let s = args.remove(0).to_string();

    // XPath rounding rules for start/length
    let start_rounded = (start_val + 0.5).floor();
    let length_rounded = length_val.map(|l| (l + 0.5).floor());

    let s_chars: Vec<char> = s.chars().collect();

    let first = start_rounded;
    let last = if let Some(l) = length_rounded {
        first + l
    } else {
        f64::INFINITY
    };

    let result = s_chars
        .iter()
        .enumerate()
        .filter_map(|(i, &c)| {
            let pos = (i + 1) as f64; // XPath positions are 1-based
            if pos >= first && pos < last {
                Some(c)
            } else {
                None
            }
        })
        .collect::<String>();
    Ok(XPathValue::String(result))
}

fn func_string_length<'a, 'd, N: DataSourceNode<'a>>(
    mut args: Vec<XPathValue<N>>,
    e_ctx: &EvaluationContext<'a, 'd, N>,
) -> Result<XPathValue<N>, XPathError> {
    if args.len() > 1 {
        return Err(XPathError::FunctionError {
            function: "string-length()".to_string(),
            message: "Expected 0 or 1 arguments".to_string(),
        });
    }
    let s = if args.is_empty() {
        e_ctx.context_node.string_value()
    } else {
        args.remove(0).to_string()
    };
    Ok(XPathValue::Number(s.chars().count() as f64))
}

fn func_normalize_space<'a, 'd, N: DataSourceNode<'a>>(
    mut args: Vec<XPathValue<N>>,
    e_ctx: &EvaluationContext<'a, 'd, N>,
) -> Result<XPathValue<N>, XPathError> {
    if args.len() > 1 {
        return Err(XPathError::FunctionError {
            function: "normalize-space()".to_string(),
            message: "Expected 0 or 1 arguments".to_string(),
        });
    }
    let s = if args.is_empty() {
        e_ctx.context_node.string_value()
    } else {
        args.remove(0).to_string()
    };
    let normalized = s.split_whitespace().collect::<Vec<_>>().join(" ");
    Ok(XPathValue::String(normalized))
}

fn func_translate<'a, N: DataSourceNode<'a>>(
    mut args: Vec<XPathValue<N>>,
) -> Result<XPathValue<N>, XPathError> {
    if args.len() != 3 {
        return Err(XPathError::FunctionError {
            function: "translate()".to_string(),
            message: "Expected 3 arguments".to_string(),
        });
    }
    let to_str: Vec<char> = args.remove(2).to_string().chars().collect();
    let from_str: Vec<char> = args.remove(1).to_string().chars().collect();
    let source_str = args.remove(0).to_string();
    let result = source_str
        .chars()
        .filter_map(|c| {
            if let Some(pos) = from_str.iter().position(|&fc| fc == c) {
                to_str.get(pos).copied()
            } else {
                Some(c)
            }
        })
        .collect::<String>();
    Ok(XPathValue::String(result))
}

// --- Boolean Functions ---

fn func_not<'a, N: DataSourceNode<'a>>(
    mut args: Vec<XPathValue<N>>,
) -> Result<XPathValue<N>, XPathError> {
    if args.len() != 1 {
        return Err(XPathError::FunctionError {
            function: "not()".to_string(),
            message: "Expected 1 argument".to_string(),
        });
    }
    Ok(XPathValue::Boolean(!args.remove(0).to_bool()))
}

fn func_true<'a, N: DataSourceNode<'a>>(
    args: Vec<XPathValue<N>>,
) -> Result<XPathValue<N>, XPathError> {
    if !args.is_empty() {
        return Err(XPathError::FunctionError {
            function: "true()".to_string(),
            message: "Expected 0 arguments".to_string(),
        });
    }
    Ok(XPathValue::Boolean(true))
}

fn func_false<'a, N: DataSourceNode<'a>>(
    args: Vec<XPathValue<N>>,
) -> Result<XPathValue<N>, XPathError> {
    if !args.is_empty() {
        return Err(XPathError::FunctionError {
            function: "false()".to_string(),
            message: "Expected 0 arguments".to_string(),
        });
    }
    Ok(XPathValue::Boolean(false))
}

fn func_lang<'a, 'd, N: DataSourceNode<'a>>(
    mut args: Vec<XPathValue<N>>,
    e_ctx: &EvaluationContext<'a, 'd, N>,
) -> Result<XPathValue<N>, XPathError> {
    if args.len() != 1 {
        return Err(XPathError::FunctionError {
            function: "lang()".to_string(),
            message: "Expected 1 argument".to_string(),
        });
    }
    let test_lang = args.remove(0).to_string().to_lowercase();
    let mut current = Some(e_ctx.context_node);

    // If context node is not an element, start with its parent.
    if current.is_some_and(|n| n.node_type() != NodeType::Element) {
        current = current.and_then(|n| n.parent());
    }

    while let Some(node) = current {
        for attr in node.attributes() {
            #[allow(clippy::collapsible_if)]
            if let Some(name) = attr.name() {
                if name.prefix == Some("xml") && name.local_part == "lang" {
                    let node_lang = attr.string_value().to_lowercase();
                    // Check for exact match or subcode match (e.g., "en" matches "en-GB")
                    if node_lang == test_lang || node_lang.starts_with(&format!("{}-", test_lang)) {
                        return Ok(XPathValue::Boolean(true));
                    }
                    // If we found an xml:lang, we don't need to check higher up.
                    return Ok(XPathValue::Boolean(false));
                }
            }
        }
        current = node.parent();
    }
    Ok(XPathValue::Boolean(false))
}

// --- Number Functions ---

fn func_sum<'a, N: DataSourceNode<'a>>(
    mut args: Vec<XPathValue<N>>,
) -> Result<XPathValue<N>, XPathError> {
    if args.len() != 1 {
        return Err(XPathError::FunctionError {
            function: "sum()".to_string(),
            message: "Expected 1 argument".to_string(),
        });
    }
    let sum = match args.remove(0) {
        XPathValue::NodeSet(nodes) => nodes
            .iter()
            .map(|node| node.string_value().trim().parse::<f64>().unwrap_or(0.0))
            .sum(),
        v => {
            return Err(XPathError::TypeError(format!(
                "sum() argument must be a node-set, got {:?}",
                v
            )));
        }
    };
    Ok(XPathValue::Number(sum))
}

fn func_floor<'a, N: DataSourceNode<'a>>(
    mut args: Vec<XPathValue<N>>,
) -> Result<XPathValue<N>, XPathError> {
    if args.len() != 1 {
        return Err(XPathError::FunctionError {
            function: "floor()".to_string(),
            message: "Expected 1 argument".to_string(),
        });
    }
    Ok(XPathValue::Number(args.remove(0).to_number().floor()))
}

fn func_ceiling<'a, N: DataSourceNode<'a>>(
    mut args: Vec<XPathValue<N>>,
) -> Result<XPathValue<N>, XPathError> {
    if args.len() != 1 {
        return Err(XPathError::FunctionError {
            function: "ceiling()".to_string(),
            message: "Expected 1 argument".to_string(),
        });
    }
    Ok(XPathValue::Number(args.remove(0).to_number().ceil()))
}

fn func_round<'a, N: DataSourceNode<'a>>(
    mut args: Vec<XPathValue<N>>,
) -> Result<XPathValue<N>, XPathError> {
    if args.len() != 1 {
        return Err(XPathError::FunctionError {
            function: "round()".to_string(),
            message: "Expected 1 argument".to_string(),
        });
    }
    let n = args.remove(0).to_number();
    if n.is_nan() || n.is_infinite() || n == 0.0 {
        return Ok(XPathValue::Number(n));
    }
    // XPath 1.0 round() rounds halves towards positive infinity.
    // floor(n + 0.5) handles this correctly for both positive and negative numbers.
    Ok(XPathValue::Number((n + 0.5).floor()))
}

impl Default for FunctionRegistry {
    fn default() -> Self {
        let mut registry = Self::new();
        // Core
        registry.register("string");
        registry.register("count");
        registry.register("id");
        registry.register("position");
        registry.register("last");
        registry.register("local-name");
        registry.register("name");
        registry.register("key");
        registry.register("generate-id");
        // String
        registry.register("concat");
        registry.register("starts-with");
        registry.register("contains");
        registry.register("substring-before");
        registry.register("substring-after");
        registry.register("substring");
        registry.register("string-length");
        registry.register("normalize-space");
        registry.register("translate");
        // Boolean
        registry.register("not");
        registry.register("true");
        registry.register("false");
        registry.register("lang");
        // Number
        registry.register("sum");
        registry.register("floor");
        registry.register("ceiling");
        registry.register("round");
        // Node Tests (registered to provide better error messages)
        registry.register("node");
        registry.register("comment");
        registry.register("processing-instruction");
        // Petty extensions
        registry.register("petty:index");
        registry
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::datasource::tests::{MockNode, MockTree, create_test_tree};
    use crate::engine::EvaluationContext;
    use std::collections::HashMap;

    // --- Test Setup ---

    // A helper struct to hold all the data needed for a test, managing lifetimes correctly.
    struct TestSetup<'a> {
        tree: &'a MockTree<'a>, // Holds a reference to the tree, not ownership
        funcs: FunctionRegistry,
        vars: HashMap<String, XPathValue<MockNode<'a>>>,
        keys: HashMap<String, HashMap<String, Vec<MockNode<'a>>>>,
    }

    impl<'a> TestSetup<'a> {
        // The owner of the tree (the test function) passes a reference.
        fn new(tree: &'a MockTree<'a>) -> Self {
            TestSetup {
                tree,
                funcs: FunctionRegistry::default(),
                vars: HashMap::new(),
                keys: HashMap::new(),
            }
        }

        fn with_keys(mut self, keys: HashMap<String, HashMap<String, Vec<MockNode<'a>>>>) -> Self {
            self.keys = keys;
            self
        }

        // Creates an EvaluationContext with a specific context node, position, and size.
        // The returned context borrows from `self` for funcs/vars, and from the tree for nodes.
        fn context<'s>(
            &'s self,
            context_node_id: usize,
            pos: usize,
            size: usize,
        ) -> EvaluationContext<'a, 's, MockNode<'a>> {
            let root = MockNode {
                id: 0,
                tree: self.tree,
            };
            let context_node = MockNode {
                id: context_node_id,
                tree: self.tree,
            };
            // self.tree has lifetime 'a, so MockNode<'a> is valid.
            // &self.funcs and &self.vars have lifetime 's.
            // This correctly constructs an EvaluationContext<'a, 's, MockNode<'a>>.
            EvaluationContext::new(
                context_node,
                root,
                &self.funcs,
                pos,
                size,
                &self.vars,
                &self.keys,
                false,
            )
        }
    }

    fn eval_func<'a, 's>(
        name: &str,
        args: Vec<XPathValue<MockNode<'a>>>,
        e_ctx: &EvaluationContext<'a, 's, MockNode<'a>>,
    ) -> XPathValue<MockNode<'a>> {
        evaluate_function(name, args, e_ctx).unwrap()
    }

    // --- String Function Tests ---

    #[test]
    fn test_func_concat() {
        let tree = create_test_tree();
        let setup = TestSetup::new(&tree);
        let e_ctx = setup.context(0, 1, 1);
        let args = vec![
            XPathValue::String("Hello".to_string()),
            XPathValue::String(" ".to_string()),
            XPathValue::String("World".to_string()),
            XPathValue::Number(42.0),
        ];
        let result = eval_func("concat", args, &e_ctx);
        assert_eq!(result.to_string(), "Hello World42");
    }

    #[test]
    fn test_func_starts_with() {
        let tree = create_test_tree();
        let setup = TestSetup::new(&tree);
        let e_ctx = setup.context(0, 1, 1);
        let args_true = vec![
            XPathValue::String("abcdef".to_string()),
            XPathValue::String("abc".to_string()),
        ];
        assert_eq!(eval_func("starts-with", args_true, &e_ctx).to_bool(), true);
        let args_false = vec![
            XPathValue::String("abcdef".to_string()),
            XPathValue::String("def".to_string()),
        ];
        assert_eq!(
            eval_func("starts-with", args_false, &e_ctx).to_bool(),
            false
        );
    }

    #[test]
    fn test_func_substring() {
        let tree = create_test_tree();
        let setup = TestSetup::new(&tree);
        let e_ctx = setup.context(0, 1, 1);

        let args1 = vec![
            XPathValue::String("12345".to_string()),
            XPathValue::Number(2.0),
            XPathValue::Number(3.0),
        ];
        assert_eq!(eval_func("substring", args1, &e_ctx).to_string(), "234");

        let args2 = vec![
            XPathValue::String("12345".to_string()),
            XPathValue::Number(2.0),
        ];
        assert_eq!(eval_func("substring", args2, &e_ctx).to_string(), "2345");

        let args3 = vec![
            XPathValue::String("12345".to_string()),
            XPathValue::Number(1.5),
            XPathValue::Number(2.6),
        ];
        assert_eq!(eval_func("substring", args3, &e_ctx).to_string(), "234");
    }

    #[test]
    fn test_func_string_length() {
        let tree = create_test_tree();
        let setup = TestSetup::new(&tree);
        let e_ctx_para = setup.context(1, 1, 1); // <para> node

        assert_eq!(
            eval_func("string-length", vec![], &e_ctx_para).to_number(),
            5.0
        ); // "Hello"
        let args = vec![XPathValue::String("four".to_string())];
        assert_eq!(
            eval_func("string-length", args, &e_ctx_para).to_number(),
            4.0
        );
    }

    #[test]
    fn test_func_normalize_space() {
        let tree = create_test_tree();
        let setup = TestSetup::new(&tree);
        let e_ctx = setup.context(0, 1, 1);
        let args = vec![XPathValue::String(
            "  leading \n and   \t trailing  ".to_string(),
        )];
        assert_eq!(
            eval_func("normalize-space", args, &e_ctx).to_string(),
            "leading and trailing"
        );
    }

    #[test]
    fn test_func_translate() {
        let tree = create_test_tree();
        let setup = TestSetup::new(&tree);
        let e_ctx = setup.context(0, 1, 1);
        let args = vec![
            XPathValue::String("BAR".to_string()),
            XPathValue::String("ABC".to_string()),
            XPathValue::String("abc".to_string()),
        ];
        assert_eq!(eval_func("translate", args, &e_ctx).to_string(), "baR");

        let args2 = vec![
            XPathValue::String("12:30".to_string()),
            XPathValue::String("0123456789".to_string()),
            XPathValue::String("abcdefghij".to_string()),
        ];
        assert_eq!(eval_func("translate", args2, &e_ctx).to_string(), "bc:da");
    }

    // --- Boolean Function Tests ---

    #[test]
    fn test_func_not() {
        let tree = create_test_tree();
        let setup = TestSetup::new(&tree);
        let e_ctx = setup.context(0, 1, 1);
        assert_eq!(
            eval_func("not", vec![XPathValue::Boolean(true)], &e_ctx).to_bool(),
            false
        );
        assert_eq!(
            eval_func("not", vec![XPathValue::Number(0.0)], &e_ctx).to_bool(),
            true
        );
        assert_eq!(
            eval_func("not", vec![XPathValue::String("".to_string())], &e_ctx).to_bool(),
            true
        );
    }

    #[test]
    fn test_func_lang() {
        let tree = create_test_tree();
        let setup = TestSetup::new(&tree);
        let e_ctx_text = setup.context(4, 1, 1); // "Hello" text node, child of para with xml:lang="en"
        let e_ctx_div = setup.context(5, 1, 1); // div with no lang

        let args_en = vec![XPathValue::String("en".to_string())];
        assert_eq!(eval_func("lang", args_en, &e_ctx_text).to_bool(), true);

        let args_engb = vec![XPathValue::String("en-GB".to_string())];
        assert_eq!(eval_func("lang", args_engb, &e_ctx_text).to_bool(), false);

        let args_en_div = vec![XPathValue::String("en".to_string())];
        assert_eq!(eval_func("lang", args_en_div, &e_ctx_div).to_bool(), false);
    }

    // --- Number Function Tests ---

    #[test]
    fn test_func_sum() {
        let tree = create_test_tree();
        let setup = TestSetup::new(&tree);
        let e_ctx = setup.context(0, 1, 1);
        let node1 = MockNode {
            id: 1,
            tree: &setup.tree,
        }; // string-value is "Hello" -> NaN -> 0.0
        let node2 = MockNode {
            id: 2,
            tree: &setup.tree,
        }; // string-value is "p1" -> NaN -> 0.0
        let args = vec![XPathValue::NodeSet(vec![node1, node2])];
        assert_eq!(eval_func("sum", args, &e_ctx).to_number(), 0.0);
    }

    #[test]
    fn test_func_round() {
        let tree = create_test_tree();
        let setup = TestSetup::new(&tree);
        let e_ctx = setup.context(0, 1, 1);
        assert_eq!(
            eval_func("round", vec![XPathValue::Number(2.5)], &e_ctx).to_number(),
            3.0
        );
        assert_eq!(
            eval_func("round", vec![XPathValue::Number(2.4)], &e_ctx).to_number(),
            2.0
        );
        assert_eq!(
            eval_func("round", vec![XPathValue::Number(-2.5)], &e_ctx).to_number(),
            -2.0
        );
        assert_eq!(
            eval_func("round", vec![XPathValue::Number(-2.6)], &e_ctx).to_number(),
            -3.0
        );
    }

    // --- Node-Set Function Tests ---

    #[test]
    fn test_func_last_and_position() {
        let tree = create_test_tree();
        let setup = TestSetup::new(&tree);
        // Simulate being the 2nd node in a context of 5 nodes
        let e_ctx = setup.context(1, 2, 5);
        assert_eq!(eval_func("last", vec![], &e_ctx).to_number(), 5.0);
        assert_eq!(eval_func("position", vec![], &e_ctx).to_number(), 2.0);
    }

    #[test]
    fn test_func_local_name() {
        let tree = create_test_tree();
        let setup = TestSetup::new(&tree);
        let e_ctx_para = setup.context(1, 1, 1); // <para>
        let e_ctx_text = setup.context(4, 1, 1); // text()

        // No args, uses context node
        assert_eq!(
            eval_func("local-name", vec![], &e_ctx_para).to_string(),
            "para"
        );
        assert_eq!(eval_func("local-name", vec![], &e_ctx_text).to_string(), "");

        // With args
        let para_node = MockNode {
            id: 1,
            tree: &setup.tree,
        };
        let args = vec![XPathValue::NodeSet(vec![para_node])];
        assert_eq!(
            eval_func("local-name", args, &e_ctx_para).to_string(),
            "para"
        );
    }

    #[test]
    fn test_func_key() {
        let tree = create_test_tree();
        let para_node = MockNode { id: 1, tree: &tree };
        let attr_node = MockNode { id: 2, tree: &tree };

        let mut key_index = HashMap::new();
        key_index.insert("p1".to_string(), vec![para_node]); // key 'id-key' with value 'p1' maps to <para>
        key_index.insert("attr-val".to_string(), vec![attr_node]); // key 'id-key' with value 'attr-val' maps to @id

        let mut keys = HashMap::new();
        keys.insert("id-key".to_string(), key_index);

        let setup = TestSetup::new(&tree).with_keys(keys);
        let e_ctx = setup.context(0, 1, 1); // Context is root

        // Test key('id-key', 'p1')
        let args1 = vec![
            XPathValue::String("id-key".to_string()),
            XPathValue::String("p1".to_string()),
        ];
        let result1 = eval_func("key", args1, &e_ctx);
        if let XPathValue::NodeSet(nodes) = result1 {
            assert_eq!(nodes.len(), 1);
            assert_eq!(nodes[0], para_node);
        } else {
            panic!("Expected NodeSet");
        }

        // Test key('id-key', 'nonexistent')
        let args2 = vec![
            XPathValue::String("id-key".to_string()),
            XPathValue::String("nonexistent".to_string()),
        ];
        let result2 = eval_func("key", args2, &e_ctx);
        if let XPathValue::NodeSet(nodes) = result2 {
            assert!(nodes.is_empty());
        } else {
            panic!("Expected NodeSet");
        }

        // Test key('id-key', /para/@id) -- arg is a node-set
        let args3 = vec![
            XPathValue::String("id-key".to_string()),
            XPathValue::NodeSet(vec![attr_node]), // attr_node's string value is 'p1'
        ];
        let result3 = eval_func("key", args3, &e_ctx);
        if let XPathValue::NodeSet(nodes) = result3 {
            assert_eq!(nodes.len(), 1);
            assert_eq!(nodes[0], para_node);
        } else {
            panic!("Expected NodeSet");
        }
    }
}
