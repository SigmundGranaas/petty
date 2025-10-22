// FILE: src/parser/xslt/pattern.rs
//! A dedicated engine for parsing and evaluating XSLT `match` patterns.
use crate::parser::datasource::{DataSourceNode, NodeType};
use crate::parser::xpath::ast::{NodeTest, NodeTypeTest};
use crate::parser::xpath::parser as xpath_parser;
use crate::parser::ParseError;
use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::combinator::map;
use nom::multi::separated_list1;
use nom::sequence::preceded;
use nom::IResult;
use std::fmt;

/// Represents a single location step in a match pattern (e.g., `foo`, `*`, `text()`).
#[derive(Debug, Clone, PartialEq, Eq)]
struct MatchStep {
    axis: MatchAxis,
    node_test: NodeTest,
}

/// The axes relevant for match patterns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MatchAxis {
    Child,
    Attribute,
}

/// A compiled representation of an XSLT match pattern.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pattern {
    /// A pattern can be a union of multiple paths, e.g., "para|note".
    paths: Vec<LocationPathPattern>,
    original_text: String,
}

impl fmt::Display for Pattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.original_text)
    }
}


/// A single location path within a pattern, e.g., "/doc/section/para".
#[derive(Debug, Clone, PartialEq, Eq)]
struct LocationPathPattern {
    is_absolute: bool,
    steps: Vec<MatchStep>,
}

impl Pattern {
    /// Evaluates if a given node matches this compiled pattern.
    pub fn matches<'a, N: DataSourceNode<'a>>(&self, node: N, root: N) -> bool {
        self.paths.iter().any(|path| path.matches(node, root))
    }
}

impl LocationPathPattern {
    fn matches<'a, N: DataSourceNode<'a>>(&self, node: N, root: N) -> bool {
        if self.is_absolute && self.steps.is_empty() { // Special case for "/"
            return node == root;
        }

        let mut current_node = Some(node);
        let mut steps_to_match = self.steps.iter().rev();

        while let Some(step) = steps_to_match.next() {
            if let Some(cn) = current_node {
                if !step.matches(cn) {
                    return false;
                }
                current_node = cn.parent();
            } else {
                return false; // Ran out of nodes before running out of steps
            }
        }

        if self.is_absolute {
            current_node == Some(root)
        } else {
            true
        }
    }
}

impl MatchStep {
    fn matches<'a, N: DataSourceNode<'a>>(&self, node: N) -> bool {
        let node_type = node.node_type();
        let name = node.name();

        match self.axis {
            MatchAxis::Attribute => {
                if node_type != NodeType::Attribute {
                    return false;
                }
            }
            MatchAxis::Child => {
                if node_type != NodeType::Element && node_type != NodeType::Text {
                    return false;
                }
            }
        }

        match &self.node_test {
            NodeTest::Wildcard => true,
            NodeTest::Name(test_name) => name.map_or(false, |q| q.local_part == test_name),
            NodeTest::NodeType(ntt) => match ntt {
                NodeTypeTest::Text => node_type == NodeType::Text,
                NodeTypeTest::Node => true,
            },
        }
    }
}

// --- Parser ---

pub fn parse(text: &str) -> Result<Pattern, ParseError> {
    match pattern_parser(text.trim()) {
        Ok(("", paths)) => Ok(Pattern { paths, original_text: text.to_string() }),
        Ok((rem, _)) => Err(ParseError::XPathParse(text.to_string(), format!("Unconsumed input in pattern: {}", rem))),
        Err(e) => Err(ParseError::XPathParse(text.to_string(), e.to_string())),
    }
}

fn step_parser(input: &str) -> IResult<&str, MatchStep> {
    let (remaining_input, node_test) = alt((
        map(preceded(tag("@"), xpath_parser::node_test), |nt| (nt, MatchAxis::Attribute)),
        map(xpath_parser::node_test, |nt| (nt, MatchAxis::Child)),
    ))(input)?;

    Ok((remaining_input, MatchStep { axis: node_test.1, node_test: node_test.0 }))
}

fn path_parser(input: &str) -> IResult<&str, LocationPathPattern> {
    // Check for an absolute path first.
    if let Ok((remaining, _)) = tag::<&str, &str, nom::error::Error<&str>>("/")(input) {
        // It's an absolute path. It's only valid if no steps follow, i.e., it's just "/".
        // We test this by seeing if a step can be parsed from the remaining input.
        if let Ok(_) = step_parser(remaining) {
            // A step was parsed (e.g., "/foo"), which is an invalid pattern. Return a parse error.
            Err(nom::Err::Error(nom::error::Error::new(
                input,
                nom::error::ErrorKind::Verify,
            )))
        } else {
            // No step could be parsed. This is the valid "/" pattern.
            Ok((
                remaining,
                LocationPathPattern {
                    is_absolute: true,
                    steps: vec![],
                },
            ))
        }
    } else {
        // It's a relative path. It must have at least one step.
        let (input, steps) = separated_list1(tag("/"), step_parser)(input)?;
        Ok((
            input,
            LocationPathPattern {
                is_absolute: false,
                steps,
            },
        ))
    }
}

fn pattern_parser(input: &str) -> IResult<&str, Vec<LocationPathPattern>> {
    separated_list1(tag("|"), path_parser)(input)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::datasource::tests::{create_test_tree, MockNode, MockTree};

    fn get_node<'a>(tree: &'a MockTree<'a>, id: usize) -> MockNode<'a> {
        MockNode { id, tree }
    }

    #[test]
    fn test_pattern_parsing() {
        assert!(parse("foo").is_ok());
        assert!(parse("foo/bar").is_ok());
        assert!(parse("/foo/bar").is_err(), "Absolute paths with steps should be invalid patterns");
        assert!(parse("foo|bar").is_ok());
        assert!(parse("text()").is_ok());
        assert!(parse("@id").is_ok());
        assert!(parse("*").is_ok());
        assert!(parse("foo/*/@id").is_ok());
        assert!(parse("/").is_ok());
    }

    #[test]
    fn test_simple_name_match() {
        let tree = create_test_tree();
        let pattern = parse("para").unwrap();
        assert!(pattern.matches(get_node(&tree, 1), get_node(&tree, 0))); // <para>
        assert!(!pattern.matches(get_node(&tree, 0), get_node(&tree, 0))); // <root>
    }

    #[test]
    fn test_path_match() {
        let tree = create_test_tree();
        let pattern = parse("para/text()").unwrap();
        assert!(pattern.matches(get_node(&tree, 3), get_node(&tree, 0))); // "Hello" text node
        assert!(!pattern.matches(get_node(&tree, 1), get_node(&tree, 0))); // <para> itself
    }

    #[test]
    fn test_absolute_path_match() {
        let tree = create_test_tree();
        let pattern = parse("para").unwrap();
        let root = get_node(&tree, 0);
        let para = get_node(&tree, 1);
        // FIX: The pattern is relative, so it should match the `para` node regardless of its parent.
        assert!(pattern.matches(para, root));

        let root_pattern = parse("/").unwrap();
        assert!(root_pattern.matches(root, root));
        assert!(!root_pattern.matches(para, root));
    }

    #[test]
    fn test_union_match() {
        let tree = create_test_tree();
        let pattern = parse("nonexistent|para").unwrap();
        assert!(pattern.matches(get_node(&tree, 1), get_node(&tree, 0)));
    }

    #[test]
    fn test_attribute_match() {
        let tree = create_test_tree();
        let pattern = parse("@id").unwrap();
        assert!(pattern.matches(get_node(&tree, 2), get_node(&tree, 0))); // id attribute
        assert!(!pattern.matches(get_node(&tree, 1), get_node(&tree, 0))); // <para> element
    }
}