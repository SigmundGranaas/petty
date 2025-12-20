//! A dedicated engine for parsing and evaluating XSLT `match` patterns.
use crate::error::XsltError;
use nom::IResult;
use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::combinator::map;
use nom::multi::{separated_list0, separated_list1};
use nom::sequence::preceded;
use petty_xpath1::ast::{NodeTest, NodeTypeTest};
use petty_xpath1::datasource::{DataSourceNode, NodeType};
use petty_xpath1::parser as xpath_parser;
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
        if self.is_absolute && self.steps.is_empty() {
            // Special case for "/"
            return node == root;
        }

        let mut current_node = Some(node);
        let steps_to_match = self.steps.iter().rev();

        for step in steps_to_match {
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
                // Child axis in patterns can match elements, text nodes, and the root.
                if node_type != NodeType::Element
                    && node_type != NodeType::Text
                    && node_type != NodeType::Root
                {
                    return false;
                }
            }
        }

        match &self.node_test {
            NodeTest::Wildcard => {
                // `*` on a child axis should only match elements.
                if self.axis == MatchAxis::Child {
                    node_type == NodeType::Element
                } else {
                    true
                }
            }
            NodeTest::Name(test_name) => name.is_some_and(|q| q.local_part == test_name),
            NodeTest::NodeType(ntt) => match ntt {
                NodeTypeTest::Text => node_type == NodeType::Text,
                NodeTypeTest::Comment => node_type == NodeType::Comment,
                NodeTypeTest::ProcessingInstruction => node_type == NodeType::ProcessingInstruction,
                NodeTypeTest::Node => true,
            },
        }
    }
}

// --- Parser ---

pub fn parse(text: &str) -> Result<Pattern, XsltError> {
    match pattern_parser(text.trim()) {
        Ok(("", paths)) => Ok(Pattern {
            paths,
            original_text: text.to_string(),
        }),
        Ok((rem, _)) => Err(XsltError::XPathParse(
            text.to_string(),
            format!("Unconsumed input in pattern: {}", rem),
        )),
        Err(e) => Err(XsltError::XPathParse(text.to_string(), e.to_string())),
    }
}

fn step_parser(input: &str) -> IResult<&str, MatchStep> {
    let (remaining_input, node_test) = alt((
        map(preceded(tag("@"), xpath_parser::node_test), |nt| {
            (nt, MatchAxis::Attribute)
        }),
        map(xpath_parser::node_test, |nt| (nt, MatchAxis::Child)),
    ))(input)?;

    Ok((
        remaining_input,
        MatchStep {
            axis: node_test.1,
            node_test: node_test.0,
        },
    ))
}

fn path_parser(input: &str) -> IResult<&str, LocationPathPattern> {
    let (remaining, is_absolute) =
        if let Ok((rem, _)) = tag::<&str, &str, nom::error::Error<&str>>("/")(input) {
            (rem, true)
        } else {
            (input, false)
        };

    let (remaining, steps) = if is_absolute {
        // An absolute path can be just `/` (no steps) or have subsequent steps like `/*` or `/root/item`
        separated_list0(tag("/"), step_parser)(remaining)?
    } else {
        // A relative path MUST have at least one step.
        separated_list1(tag("/"), step_parser)(remaining)?
    };

    Ok((remaining, LocationPathPattern { is_absolute, steps }))
}

fn pattern_parser(input: &str) -> IResult<&str, Vec<LocationPathPattern>> {
    separated_list1(tag("|"), path_parser)(input)
}

#[cfg(test)]
mod tests {
    use super::*;
    use petty_xpath1::datasource::tests::{MockNode, MockTree, create_test_tree};

    fn get_node<'a>(tree: &'a MockTree<'a>, id: usize) -> MockNode<'a> {
        MockNode { id, tree }
    }

    #[test]
    fn test_pattern_parsing() {
        assert!(parse("foo").is_ok());
        assert!(parse("foo/bar").is_ok());
        assert!(parse("/").is_ok());
        assert!(parse("/*").is_ok());
        assert!(parse("/root/item").is_ok()); // This is now valid.
        assert!(parse("foo|bar").is_ok());
        assert!(parse("text()").is_ok());
        assert!(parse("@id").is_ok());
        assert!(parse("*").is_ok());
        assert!(parse("foo/*/@id").is_ok());
    }

    #[test]
    fn test_simple_name_match() {
        let tree = create_test_tree();
        let pattern = parse("para").unwrap();
        assert!(pattern.matches(get_node(&tree, 1), get_node(&tree, 0))); // <para>
        assert!(!pattern.matches(get_node(&tree, 0), get_node(&tree, 0))); // <root>
    }

    #[test]
    fn test_absolute_wildcard_match() {
        let tree = create_test_tree();
        let pattern = parse("/*").unwrap();
        let root_node = get_node(&tree, 0);
        let doc_element = get_node(&tree, 1); // <para> is the document element in the test tree
        let text_node = get_node(&tree, 4);

        assert!(pattern.matches(doc_element, root_node));
        assert!(!pattern.matches(root_node, root_node));
        assert!(!pattern.matches(text_node, root_node));
    }

    #[test]
    fn test_path_match() {
        let tree = create_test_tree();
        let pattern = parse("para/text()").unwrap();
        assert!(pattern.matches(get_node(&tree, 4), get_node(&tree, 0))); // "Hello" text node
        assert!(!pattern.matches(get_node(&tree, 1), get_node(&tree, 0))); // <para> itself
    }

    #[test]
    fn test_absolute_path_match() {
        let tree = create_test_tree();
        let root_pattern = parse("/").unwrap();
        assert!(root_pattern.matches(get_node(&tree, 0), get_node(&tree, 0)));
        assert!(!root_pattern.matches(get_node(&tree, 1), get_node(&tree, 0)));
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
