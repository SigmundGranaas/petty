// FILE: src/parser/xpath/functions.rs
//! Defines the registry and built-in implementations for XPath 1.0 functions.

use super::engine::{EvaluationContext, XPathValue};
use crate::parser::datasource::DataSourceNode;
use crate::parser::ParseError;
use std::collections::HashMap;

// A simple registry that just holds the names of built-in functions.
pub struct FunctionRegistry {
    functions: HashMap<&'static str, ()>,
}

impl FunctionRegistry {
    pub fn new() -> Self {
        Self { functions: HashMap::new() }
    }
    pub fn register(&mut self, name: &'static str) {
        self.functions.insert(name, ());
    }
    pub fn get(&self, name: &str) -> Option<()> {
        self.functions.get(name.to_lowercase().as_str()).copied()
    }
}

/// Dispatches a function call to the correct implementation.
pub fn evaluate_function<'a, 'd, N: DataSourceNode<'a>>(
    name: &str,
    args: Vec<XPathValue<N>>,
    e_ctx: &EvaluationContext<'a, 'd, N>,
) -> Result<XPathValue<N>, ParseError> {
    match name.to_lowercase().as_str() {
        // Core & Node-Set
        "string" => func_string(args, e_ctx),
        "count" => func_count(args),
        "position" => func_position(args, e_ctx),
        "last" => func_last(args, e_ctx),
        "local-name" => func_local_name(args, e_ctx),
        "name" => func_name(args, e_ctx),

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
        "lang" => Err(ParseError::TemplateRender("lang() function is not implemented.".to_string())),

        // Number
        "sum" => func_sum(args),
        "floor" => func_floor(args),
        "ceiling" => func_ceiling(args),
        "round" => func_round(args),

        // "node" is not a real function, but registering it prevents "unknown function" errors
        // when the parser mistakes the node() test for a function call.
        "node" => Err(ParseError::XPathParse("node()".to_string(), "node() is a node-test, not a function.".to_string())),
        _ => Err(ParseError::TemplateRender(format!("Unknown XPath function: {}", name))),
    }
}

// --- Core & Node-Set Functions ---

fn func_string<'a, 'd, N: DataSourceNode<'a>>(
    mut args: Vec<XPathValue<N>>,
    e_ctx: &EvaluationContext<'a, 'd, N>,
) -> Result<XPathValue<N>, ParseError> {
    if args.len() > 1 {
        return Err(ParseError::XPathParse("string()".to_string(), "Expected 0 or 1 arguments".to_string()));
    }
    let s = if args.is_empty() {
        e_ctx.context_node.string_value()
    } else {
        args.remove(0).to_string()
    };
    Ok(XPathValue::String(s))
}

fn func_count<'a, N: DataSourceNode<'a>>(mut args: Vec<XPathValue<N>>) -> Result<XPathValue<N>, ParseError> {
    if args.len() != 1 {
        return Err(ParseError::XPathParse("count()".to_string(), "Expected 1 argument".to_string()));
    }
    let count = match args.remove(0) {
        XPathValue::NodeSet(nodes) => nodes.len() as f64,
        v => return Err(ParseError::XPathParse("count()".to_string(), format!("Argument must be a node-set, got {:?}", v))),
    };
    Ok(XPathValue::Number(count))
}

fn func_position<'a, 'd, N: DataSourceNode<'a>>(args: Vec<XPathValue<N>>, e_ctx: &EvaluationContext<'a, 'd, N>) -> Result<XPathValue<N>, ParseError> {
    if !args.is_empty() {
        return Err(ParseError::XPathParse("position()".to_string(), "Expected 0 arguments".to_string()));
    }
    Ok(XPathValue::Number(e_ctx.context_position as f64))
}

fn func_last<'a, 'd, N: DataSourceNode<'a>>(args: Vec<XPathValue<N>>, e_ctx: &EvaluationContext<'a, 'd, N>) -> Result<XPathValue<N>, ParseError> {
    if !args.is_empty() {
        return Err(ParseError::XPathParse("last()".to_string(), "Expected 0 arguments".to_string()));
    }
    Ok(XPathValue::Number(e_ctx.context_size as f64))
}

fn func_local_name<'a, 'd, N: DataSourceNode<'a>>(
    mut args: Vec<XPathValue<N>>,
    e_ctx: &EvaluationContext<'a, 'd, N>,
) -> Result<XPathValue<N>, ParseError> {
    if args.len() > 1 {
        return Err(ParseError::XPathParse("local-name()".to_string(), "Expected 0 or 1 arguments".to_string()));
    }
    let node = if args.is_empty() {
        Some(e_ctx.context_node)
    } else {
        match args.remove(0) {
            XPathValue::NodeSet(nodes) => nodes.first().copied(),
            v => return Err(ParseError::XPathParse("local-name()".to_string(), format!("Argument must be a node-set, got {:?}", v))),
        }
    };
    let name = node.and_then(|n| n.name().map(|q| q.local_part.to_string())).unwrap_or_default();
    Ok(XPathValue::String(name))
}

fn func_name<'a, 'd, N: DataSourceNode<'a>>(
    mut args: Vec<XPathValue<N>>,
    e_ctx: &EvaluationContext<'a, 'd, N>,
) -> Result<XPathValue<N>, ParseError> {
    if args.len() > 1 {
        return Err(ParseError::XPathParse("name()".to_string(), "Expected 0 or 1 arguments".to_string()));
    }
    let node = if args.is_empty() {
        Some(e_ctx.context_node)
    } else {
        match args.remove(0) {
            XPathValue::NodeSet(nodes) => nodes.first().copied(),
            v => return Err(ParseError::XPathParse("name()".to_string(), format!("Argument must be a node-set, got {:?}", v))),
        }
    };
    let name = node.and_then(|n| n.name().map(|q| {
        if let Some(prefix) = q.prefix {
            format!("{}:{}", prefix, q.local_part)
        } else {
            q.local_part.to_string()
        }
    })).unwrap_or_default();
    Ok(XPathValue::String(name))
}

// --- String Functions ---

fn func_concat<'a, N: DataSourceNode<'a>>(args: Vec<XPathValue<N>>) -> Result<XPathValue<N>, ParseError> {
    if args.len() < 2 {
        return Err(ParseError::XPathParse("concat()".to_string(), "Expected at least 2 arguments".to_string()));
    }
    let result = args.iter().map(|v| v.to_string()).collect::<String>();
    Ok(XPathValue::String(result))
}

fn func_starts_with<'a, N: DataSourceNode<'a>>(mut args: Vec<XPathValue<N>>) -> Result<XPathValue<N>, ParseError> {
    if args.len() != 2 {
        return Err(ParseError::XPathParse("starts-with()".to_string(), "Expected 2 arguments".to_string()));
    }
    let s2 = args.remove(1).to_string();
    let s1 = args.remove(0).to_string();
    Ok(XPathValue::Boolean(s1.starts_with(&s2)))
}

fn func_contains<'a, N: DataSourceNode<'a>>(mut args: Vec<XPathValue<N>>) -> Result<XPathValue<N>, ParseError> {
    if args.len() != 2 {
        return Err(ParseError::XPathParse("contains()".to_string(), "Expected 2 arguments".to_string()));
    }
    let s2 = args.remove(1).to_string();
    let s1 = args.remove(0).to_string();
    Ok(XPathValue::Boolean(s1.contains(&s2)))
}

fn func_substring_before<'a, N: DataSourceNode<'a>>(mut args: Vec<XPathValue<N>>) -> Result<XPathValue<N>, ParseError> {
    if args.len() != 2 {
        return Err(ParseError::XPathParse("substring-before()".to_string(), "Expected 2 arguments".to_string()));
    }
    let s2 = args.remove(1).to_string();
    let s1 = args.remove(0).to_string();
    if let Some(index) = s1.find(&s2) {
        Ok(XPathValue::String(s1[..index].to_string()))
    } else {
        Ok(XPathValue::String("".to_string()))
    }
}

fn func_substring_after<'a, N: DataSourceNode<'a>>(mut args: Vec<XPathValue<N>>) -> Result<XPathValue<N>, ParseError> {
    if args.len() != 2 {
        return Err(ParseError::XPathParse("substring-after()".to_string(), "Expected 2 arguments".to_string()));
    }
    let s2 = args.remove(1).to_string();
    let s1 = args.remove(0).to_string();
    if let Some(index) = s1.find(&s2) {
        Ok(XPathValue::String(s1[index + s2.len()..].to_string()))
    } else {
        Ok(XPathValue::String("".to_string()))
    }
}

fn func_substring<'a, N: DataSourceNode<'a>>(mut args: Vec<XPathValue<N>>) -> Result<XPathValue<N>, ParseError> {
    if !(2..=3).contains(&args.len()) {
        return Err(ParseError::XPathParse("substring()".to_string(), "Expected 2 or 3 arguments".to_string()));
    }
    let length_val = if args.len() == 3 { Some(args.remove(2).to_number()) } else { None };
    let start_val = args.remove(1).to_number();
    let s = args.remove(0).to_string();

    // XPath rounding rules for start/length
    let start_rounded = (start_val + 0.5).floor();
    let length_rounded = length_val.map(|l| (l + 0.5).floor());

    let s_chars: Vec<char> = s.chars().collect();

    let first = start_rounded;
    let last = if let Some(l) = length_rounded { first + l } else { f64::INFINITY };

    let result = s_chars.iter().enumerate()
        .filter_map(|(i, &c)| {
            let pos = (i + 1) as f64; // XPath positions are 1-based
            if pos >= first && pos < last { Some(c) } else { None }
        })
        .collect::<String>();
    Ok(XPathValue::String(result))
}

fn func_string_length<'a, 'd, N: DataSourceNode<'a>>(
    mut args: Vec<XPathValue<N>>,
    e_ctx: &EvaluationContext<'a, 'd, N>,
) -> Result<XPathValue<N>, ParseError> {
    if args.len() > 1 {
        return Err(ParseError::XPathParse("string-length()".to_string(), "Expected 0 or 1 arguments".to_string()));
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
) -> Result<XPathValue<N>, ParseError> {
    if args.len() > 1 {
        return Err(ParseError::XPathParse("normalize-space()".to_string(), "Expected 0 or 1 arguments".to_string()));
    }
    let s = if args.is_empty() {
        e_ctx.context_node.string_value()
    } else {
        args.remove(0).to_string()
    };
    let normalized = s.trim().split_whitespace().collect::<Vec<_>>().join(" ");
    Ok(XPathValue::String(normalized))
}

fn func_translate<'a, N: DataSourceNode<'a>>(mut args: Vec<XPathValue<N>>) -> Result<XPathValue<N>, ParseError> {
    if args.len() != 3 {
        return Err(ParseError::XPathParse("translate()".to_string(), "Expected 3 arguments".to_string()));
    }
    let to_str: Vec<char> = args.remove(2).to_string().chars().collect();
    let from_str: Vec<char> = args.remove(1).to_string().chars().collect();
    let source_str = args.remove(0).to_string();
    let result = source_str.chars().filter_map(|c| {
        if let Some(pos) = from_str.iter().position(|&fc| fc == c) {
            to_str.get(pos).copied()
        } else {
            Some(c)
        }
    }).collect::<String>();
    Ok(XPathValue::String(result))
}

// --- Boolean Functions ---

fn func_not<'a, N: DataSourceNode<'a>>(mut args: Vec<XPathValue<N>>) -> Result<XPathValue<N>, ParseError> {
    if args.len() != 1 {
        return Err(ParseError::XPathParse("not()".to_string(), "Expected 1 argument".to_string()));
    }
    Ok(XPathValue::Boolean(!args.remove(0).to_bool()))
}

fn func_true<'a, N: DataSourceNode<'a>>(args: Vec<XPathValue<N>>) -> Result<XPathValue<N>, ParseError> {
    if !args.is_empty() {
        return Err(ParseError::XPathParse("true()".to_string(), "Expected 0 arguments".to_string()));
    }
    Ok(XPathValue::Boolean(true))
}

fn func_false<'a, N: DataSourceNode<'a>>(args: Vec<XPathValue<N>>) -> Result<XPathValue<N>, ParseError> {
    if !args.is_empty() {
        return Err(ParseError::XPathParse("false()".to_string(), "Expected 0 arguments".to_string()));
    }
    Ok(XPathValue::Boolean(false))
}

// --- Number Functions ---

fn func_sum<'a, N: DataSourceNode<'a>>(mut args: Vec<XPathValue<N>>) -> Result<XPathValue<N>, ParseError> {
    if args.len() != 1 {
        return Err(ParseError::XPathParse("sum()".to_string(), "Expected 1 argument".to_string()));
    }
    let sum = match args.remove(0) {
        XPathValue::NodeSet(nodes) => {
            nodes.iter().map(|node| {
                node.string_value().trim().parse::<f64>().unwrap_or(0.0)
            }).sum()
        },
        v => return Err(ParseError::XPathParse("sum()".to_string(), format!("Argument must be a node-set, got {:?}", v))),
    };
    Ok(XPathValue::Number(sum))
}

fn func_floor<'a, N: DataSourceNode<'a>>(mut args: Vec<XPathValue<N>>) -> Result<XPathValue<N>, ParseError> {
    if args.len() != 1 {
        return Err(ParseError::XPathParse("floor()".to_string(), "Expected 1 argument".to_string()));
    }
    Ok(XPathValue::Number(args.remove(0).to_number().floor()))
}

fn func_ceiling<'a, N: DataSourceNode<'a>>(mut args: Vec<XPathValue<N>>) -> Result<XPathValue<N>, ParseError> {
    if args.len() != 1 {
        return Err(ParseError::XPathParse("ceiling()".to_string(), "Expected 1 argument".to_string()));
    }
    Ok(XPathValue::Number(args.remove(0).to_number().ceil()))
}

fn func_round<'a, N: DataSourceNode<'a>>(mut args: Vec<XPathValue<N>>) -> Result<XPathValue<N>, ParseError> {
    if args.len() != 1 {
        return Err(ParseError::XPathParse("round()".to_string(), "Expected 1 argument".to_string()));
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
        registry.register("position");
        registry.register("last");
        registry.register("local-name");
        registry.register("name");
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
        // Other
        registry.register("node");
        registry
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::datasource::tests::{create_test_tree, MockNode, MockTree};
    use crate::parser::xpath::engine::EvaluationContext;
    use std::collections::HashMap;

    // --- Test Setup ---

    // A helper struct to hold all the data needed for a test, managing lifetimes correctly.
    struct TestSetup<'a> {
        tree: &'a MockTree<'a>, // Holds a reference to the tree, not ownership
        funcs: FunctionRegistry,
        vars: HashMap<String, XPathValue<MockNode<'a>>>,
    }

    impl<'a> TestSetup<'a> {
        // The owner of the tree (the test function) passes a reference.
        fn new(tree: &'a MockTree<'a>) -> Self {
            TestSetup {
                tree,
                funcs: FunctionRegistry::default(),
                vars: HashMap::new(),
            }
        }

        // Creates an EvaluationContext with a specific context node, position, and size.
        // The returned context borrows from `self` for funcs/vars, and from the tree for nodes.
        fn context<'s>(&'s self, context_node_id: usize, pos: usize, size: usize) -> EvaluationContext<'a, 's, MockNode<'a>> {
            let root = MockNode { id: 0, tree: self.tree };
            let context_node = MockNode { id: context_node_id, tree: self.tree };
            // self.tree has lifetime 'a, so MockNode<'a> is valid.
            // &self.funcs and &self.vars have lifetime 's.
            // This correctly constructs an EvaluationContext<'a, 's, MockNode<'a>>.
            EvaluationContext::new(context_node, root, &self.funcs, pos, size, &self.vars)
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
        let args_true = vec![XPathValue::String("abcdef".to_string()), XPathValue::String("abc".to_string())];
        assert_eq!(eval_func("starts-with", args_true, &e_ctx).to_bool(), true);
        let args_false = vec![XPathValue::String("abcdef".to_string()), XPathValue::String("def".to_string())];
        assert_eq!(eval_func("starts-with", args_false, &e_ctx).to_bool(), false);
    }

    #[test]
    fn test_func_substring() {
        let tree = create_test_tree();
        let setup = TestSetup::new(&tree);
        let e_ctx = setup.context(0, 1, 1);

        let args1 = vec![XPathValue::String("12345".to_string()), XPathValue::Number(2.0), XPathValue::Number(3.0)];
        assert_eq!(eval_func("substring", args1, &e_ctx).to_string(), "234");

        let args2 = vec![XPathValue::String("12345".to_string()), XPathValue::Number(2.0)];
        assert_eq!(eval_func("substring", args2, &e_ctx).to_string(), "2345");

        let args3 = vec![XPathValue::String("12345".to_string()), XPathValue::Number(1.5), XPathValue::Number(2.6)];
        assert_eq!(eval_func("substring", args3, &e_ctx).to_string(), "234");
    }

    #[test]
    fn test_func_string_length() {
        let tree = create_test_tree();
        let setup = TestSetup::new(&tree);
        let e_ctx_para = setup.context(1, 1, 1); // <para> node

        assert_eq!(eval_func("string-length", vec![], &e_ctx_para).to_number(), 5.0); // "Hello"
        let args = vec![XPathValue::String("four".to_string())];
        assert_eq!(eval_func("string-length", args, &e_ctx_para).to_number(), 4.0);
    }

    #[test]
    fn test_func_normalize_space() {
        let tree = create_test_tree();
        let setup = TestSetup::new(&tree);
        let e_ctx = setup.context(0, 1, 1);
        let args = vec![XPathValue::String("  leading \n and  \t trailing  ".to_string())];
        assert_eq!(eval_func("normalize-space", args, &e_ctx).to_string(), "leading and trailing");
    }

    #[test]
    fn test_func_translate() {
        let tree = create_test_tree();
        let setup = TestSetup::new(&tree);
        let e_ctx = setup.context(0, 1, 1);
        let args = vec![
            XPathValue::String("BAR".to_string()),
            XPathValue::String("ABC".to_string()),
            XPathValue::String("abc".to_string())
        ];
        assert_eq!(eval_func("translate", args, &e_ctx).to_string(), "baR");

        let args2 = vec![
            XPathValue::String("12:30".to_string()),
            XPathValue::String("0123456789".to_string()),
            XPathValue::String("abcdefghij".to_string())
        ];
        assert_eq!(eval_func("translate", args2, &e_ctx).to_string(), "bc:da");
    }

    // --- Boolean Function Tests ---

    #[test]
    fn test_func_not() {
        let tree = create_test_tree();
        let setup = TestSetup::new(&tree);
        let e_ctx = setup.context(0, 1, 1);
        assert_eq!(eval_func("not", vec![XPathValue::Boolean(true)], &e_ctx).to_bool(), false);
        assert_eq!(eval_func("not", vec![XPathValue::Number(0.0)], &e_ctx).to_bool(), true);
        assert_eq!(eval_func("not", vec![XPathValue::String("".to_string())], &e_ctx).to_bool(), true);
    }

    // --- Number Function Tests ---

    #[test]
    fn test_func_sum() {
        let tree = create_test_tree();
        let setup = TestSetup::new(&tree);
        let e_ctx = setup.context(0, 1, 1);
        let node1 = MockNode { id: 1, tree: &setup.tree }; // string-value is "Hello" -> NaN -> 0.0
        let node2 = MockNode { id: 2, tree: &setup.tree }; // string-value is "p1" -> NaN -> 0.0
        let args = vec![XPathValue::NodeSet(vec![node1, node2])];
        assert_eq!(eval_func("sum", args, &e_ctx).to_number(), 0.0);
    }

    #[test]
    fn test_func_round() {
        let tree = create_test_tree();
        let setup = TestSetup::new(&tree);
        let e_ctx = setup.context(0, 1, 1);
        assert_eq!(eval_func("round", vec![XPathValue::Number(2.5)], &e_ctx).to_number(), 3.0);
        assert_eq!(eval_func("round", vec![XPathValue::Number(2.4)], &e_ctx).to_number(), 2.0);
        assert_eq!(eval_func("round", vec![XPathValue::Number(-2.5)], &e_ctx).to_number(), -2.0);
        assert_eq!(eval_func("round", vec![XPathValue::Number(-2.6)], &e_ctx).to_number(), -3.0);
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
        let e_ctx_text = setup.context(3, 1, 1); // text()

        // No args, uses context node
        assert_eq!(eval_func("local-name", vec![], &e_ctx_para).to_string(), "para");
        assert_eq!(eval_func("local-name", vec![], &e_ctx_text).to_string(), "");

        // With args
        let para_node = MockNode { id: 1, tree: &setup.tree };
        let args = vec![XPathValue::NodeSet(vec![para_node])];
        assert_eq!(eval_func("local-name", args, &e_ctx_para).to_string(), "para");
    }
}