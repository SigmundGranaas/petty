// FILE: src/parser/xpath/parser.rs
//! A `nom`-based parser for the XPath 1.0 expression language.

use super::ast::*;
use crate::parser::ParseError;
use nom::{
    branch::alt,
    bytes::complete::{tag, take_while, take_while1},
    character::complete::{char, multispace0},
    combinator::{map, opt, recognize},
    multi::{many0, separated_list0},
    number::complete::double,
    sequence::{delimited, pair, preceded},
    IResult,
};

// --- Main Public Parser ---

pub fn parse_expression(input: &str) -> Result<Expression, ParseError> {
    match expression(input.trim()) {
        Ok(("", expr)) => Ok(expr),
        Ok((rem, _)) => Err(ParseError::XPathParse(
            input.to_string(),
            format!("Parser did not consume all input. Remainder: '{}'", rem),
        )),
        Err(e) => Err(ParseError::XPathParse(input.to_string(), e.to_string())),
    }
}

// --- Combinators & Helpers ---

fn ws<'a, F, O>(inner: F) -> impl FnMut(&'a str) -> IResult<&'a str, O>
where
    F: FnMut(&'a str) -> IResult<&'a str, O>,
{
    delimited(multispace0, inner, multispace0)
}

fn build_binary_expr_parser<'a, F, G>(
    mut sub_expr_parser: F,
    mut op_parser: G,
) -> impl FnMut(&'a str) -> IResult<&'a str, Expression>
where
    F: FnMut(&'a str) -> IResult<&'a str, Expression>,
    G: FnMut(&'a str) -> IResult<&'a str, BinaryOperator>,
{
    move |input: &str| {
        let (input, mut left) = sub_expr_parser(input)?;
        let (input, remainder) =
            many0(pair(ws(&mut op_parser), &mut sub_expr_parser))(input)?;

        for (op, right) in remainder {
            left = Expression::BinaryOp {
                left: Box::new(left),
                op,
                right: Box::new(right),
            };
        }
        Ok((input, left))
    }
}


// --- Expression Parsers (in order of precedence) ---

fn expression(input: &str) -> IResult<&str, Expression> {
    or_expr(input)
}

fn or_expr(input: &str) -> IResult<&str, Expression> {
    build_binary_expr_parser(and_expr, map(tag("or"), |_| BinaryOperator::Or))(input)
}

fn and_expr(input: &str) -> IResult<&str, Expression> {
    build_binary_expr_parser(equality_expr, map(tag("and"), |_| BinaryOperator::And))(input)
}

fn equality_expr(input: &str) -> IResult<&str, Expression> {
    build_binary_expr_parser(
        relational_expr,
        alt((
            map(tag("="), |_| BinaryOperator::Equals),
            map(tag("!="), |_| BinaryOperator::NotEquals),
        )),
    )(input)
}

fn relational_expr(input: &str) -> IResult<&str, Expression> {
    build_binary_expr_parser(
        additive_expr,
        alt((
            map(tag("<="), |_| BinaryOperator::LessThanOrEqual),
            map(tag(">="), |_| BinaryOperator::GreaterThanOrEqual),
            map(tag("<"), |_| BinaryOperator::LessThan),
            map(tag(">"), |_| BinaryOperator::GreaterThan),
        )),
    )(input)
}

fn additive_expr(input: &str) -> IResult<&str, Expression> {
    build_binary_expr_parser(
        multiplicative_expr,
        alt((
            map(char('+'), |_| BinaryOperator::Plus),
            map(char('-'), |_| BinaryOperator::Minus),
        )),
    )(input)
}

fn multiplicative_expr(input: &str) -> IResult<&str, Expression> {
    build_binary_expr_parser(
        union_expr,
        alt((
            map(char('*'), |_| BinaryOperator::Multiply),
            map(tag("div"), |_| BinaryOperator::Divide),
            map(tag("mod"), |_| BinaryOperator::Modulo),
        )),
    )(input)
}

// The union operator `|` has higher precedence than the others, but only applies to paths.
fn union_expr(input: &str) -> IResult<&str, Expression> {
    build_binary_expr_parser(
        path_expr,
        map(char('|'), |_| BinaryOperator::Union)
    )(input)
}


/// This is the core parser that handles the ambiguity between location paths
/// and other primary expressions that might be followed by a path.
fn path_expr(input: &str) -> IResult<&str, Expression> {
    // Try primary expressions FIRST, because a function call like `position()` is a primary expression,
    // but the more general `location_path` parser might incorrectly parse `position` as a step name
    // before the `function_call` parser gets a chance to see the `()`.
    let (i, start_expr) = alt((
        primary_expr,
        map(location_path, Expression::LocationPath),
    ))(input)?;

    let (i, remainder_steps) = many0(pair(alt((tag("//"), tag("/"))), step))(i)?;

    if remainder_steps.is_empty() {
        return Ok((i, start_expr));
    }

    let (start_point, is_absolute, mut steps) = match start_expr {
        Expression::LocationPath(lp) => (lp.start_point, lp.is_absolute, lp.steps),
        other => (Some(Box::new(other)), false, vec![]),
    };

    for (sep, next_step) in remainder_steps {
        if sep == "//" {
            steps.push(Step {
                axis: Axis::DescendantOrSelf,
                node_test: NodeTest::NodeType(NodeTypeTest::Node),
                predicates: vec![],
            });
        }
        steps.push(next_step);
    }

    let result = Expression::LocationPath(LocationPath {
        start_point,
        is_absolute,
        steps,
    });

    Ok((i, result))
}


fn primary_expr(input: &str) -> IResult<&str, Expression> {
    ws(alt((
        variable_reference,
        map(double, Expression::Number),
        map(string_literal, Expression::Literal),
        function_call,
        delimited(ws(char('(')), expression, ws(char(')'))),
    )))(input)
}

// --- Literal Parsers ---
fn string_literal(input: &str) -> IResult<&str, String> {
    map(
        alt((
            delimited(char('\''), take_while(|c| c != '\''), char('\'')),
            delimited(char('"'), take_while(|c| c != '"'), char('"')),
        )),
        |s: &str| s.to_string(),
    )(input)
}

// --- Variable Reference Parser ---
fn variable_reference(input: &str) -> IResult<&str, Expression> {
    map(preceded(char('$'), q_name), Expression::Variable)(input)
}

// --- Name and NodeTest Parsers ---
fn nc_name(input: &str) -> IResult<&str, &str> {
    recognize(pair(
        take_while1(|c: char| c.is_alphabetic() || c == '_'),
        take_while(|c: char| c.is_alphanumeric() || c == '_' || c == '-'),
    ))(input)
}

fn q_name(input: &str) -> IResult<&str, String> {
    map(
        recognize(pair(nc_name, opt(pair(tag(":"), nc_name)))),
        |s: &str| s.to_string(),
    )(input)
}

fn node_type_test(input: &str) -> IResult<&str, NodeTest> {
    map(
        pair(alt((tag("text"), tag("node"))), pair(ws(char('(')), ws(char(')')))),
        |(node_type, _)| match node_type {
            "text" => NodeTest::NodeType(NodeTypeTest::Text),
            _ => NodeTest::NodeType(NodeTypeTest::Node),
        },
    )(input)
}

pub fn node_test(input: &str) -> IResult<&str, NodeTest> {
    alt((
        map(tag("*"), |_| NodeTest::Wildcard),
        node_type_test,
        map(q_name, NodeTest::Name),
    ))(input)
}

// --- Path Parsers ---
fn axis(input: &str) -> IResult<&str, Axis> {
    map(
        pair(
            alt((
                tag("child"),
                tag("descendant-or-self"),
                tag("descendant"),
                tag("attribute"),
                tag("parent"),
                tag("ancestor"),
            )),
            tag("::"),
        ),
        |(axis_str, _)| match axis_str {
            "descendant-or-self" => Axis::DescendantOrSelf,
            "descendant" => Axis::Descendant,
            "attribute" => Axis::Attribute,
            "parent" => Axis::Parent,
            "ancestor" => Axis::Ancestor,
            _ => Axis::Child,
        },
    )(input)
}

fn predicate(input: &str) -> IResult<&str, Expression> {
    delimited(ws(char('[')), expression, ws(char(']')))(input)
}

fn step(input: &str) -> IResult<&str, Step> {
    let (i, main_part) = alt((
        map(tag("."), |_| (Axis::Child, NodeTest::Name(".".to_string()))),
        map(preceded(char('@'), node_test), |nt| (Axis::Attribute, nt)),
        map(pair(opt(axis), node_test), |(ax, nt)| (ax.unwrap_or(Axis::Child), nt)),
    ))(input)?;
    let (axis, node_test) = main_part;
    let (i, predicates) = many0(predicate)(i)?;
    Ok((i, Step { axis, node_test, predicates }))
}

fn location_path(input: &str) -> IResult<&str, LocationPath> {
    // This parser handles a path that does NOT start with a variable or function call.
    let (i, (is_absolute, first_step)) = if let Ok((rem, _)) = tag::<&str, &str, nom::error::Error<&str>>("//")(input) {
        let (rem, step) = step(rem)?;
        let initial_steps = vec![
            Step { axis: Axis::DescendantOrSelf, node_test: NodeTest::NodeType(NodeTypeTest::Node), predicates: vec![] },
            step,
        ];
        (rem, (true, initial_steps))
    } else if let Ok((rem, _)) = tag::<&str, &str, nom::error::Error<&str>>("/")(input) {
        if let Ok((rem, first_step)) = step(rem) {
            (rem, (true, vec![first_step]))
        } else {
            // This handles the case of a path that is just "/"
            (rem, (true, vec![]))
        }
    } else {
        let (rem, first_step) = step(input)?;
        (rem, (false, vec![first_step]))
    };

    let (i, mut steps) = (i, first_step);
    // After the first step, subsequent steps MUST be preceded by / or //.
    let (i, remainder) = many0(pair(alt((tag("//"), tag("/"))), step))(i)?;

    for (sep, next_step) in remainder {
        if sep == "//" {
            steps.push(Step {
                axis: Axis::DescendantOrSelf,
                node_test: NodeTest::NodeType(NodeTypeTest::Node),
                predicates: vec![],
            });
        }
        steps.push(next_step);
    }

    Ok((i, LocationPath { start_point: None, is_absolute, steps }))
}

// --- Function Call Parser ---
fn function_call(input: &str) -> IResult<&str, Expression> {
    let (i, name) = q_name(input)?;
    let (i, _) = multispace0(i)?;
    let (i, args) =
        delimited(char('('), separated_list0(ws(char(',')), expression), char(')'))(i)?;

    Ok((i, Expression::FunctionCall { name, args }))
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_path() {
        let result = parse_expression("foo/bar").unwrap();
        assert_eq!(
            result,
            Expression::LocationPath(LocationPath {
                start_point: None,
                is_absolute: false,
                steps: vec![
                    Step { axis: Axis::Child, node_test: NodeTest::Name("foo".into()), predicates: vec![] },
                    Step { axis: Axis::Child, node_test: NodeTest::Name("bar".into()), predicates: vec![] },
                ]
            })
        );
    }

    #[test]
    fn test_parse_path_starting_with_variable() {
        let result = parse_expression("$myVar/foo/bar").unwrap();
        assert_eq!(
            result,
            Expression::LocationPath(LocationPath {
                start_point: Some(Box::new(Expression::Variable("myVar".to_string()))),
                is_absolute: false,
                steps: vec![
                    Step { axis: Axis::Child, node_test: NodeTest::Name("foo".into()), predicates: vec![] },
                    Step { axis: Axis::Child, node_test: NodeTest::Name("bar".into()), predicates: vec![] },
                ]
            })
        );
    }

    #[test]
    fn test_parse_variable_reference() {
        let result = parse_expression("$myVar").unwrap();
        assert_eq!(result, Expression::Variable("myVar".to_string()));

        let result_with_op = parse_expression("$myVar + 5").unwrap();
        assert_eq!(
            result_with_op,
            Expression::BinaryOp {
                left: Box::new(Expression::Variable("myVar".to_string())),
                op: BinaryOperator::Plus,
                right: Box::new(Expression::Number(5.0))
            }
        )
    }

    #[test]
    fn test_parse_predicate() {
        let result = parse_expression("foo[@id = 'a']").unwrap();
        let expected_predicate_path = LocationPath {
            start_point: None,
            is_absolute: false,
            steps: vec![Step {
                axis: Axis::Attribute,
                node_test: NodeTest::Name("id".into()),
                predicates: vec![],
            }]
        };
        assert_eq!(
            result,
            Expression::LocationPath(LocationPath {
                start_point: None,
                is_absolute: false,
                steps: vec![Step {
                    axis: Axis::Child,
                    node_test: NodeTest::Name("foo".into()),
                    predicates: vec![Expression::BinaryOp {
                        left: Box::new(Expression::LocationPath(expected_predicate_path)),
                        op: BinaryOperator::Equals,
                        right: Box::new(Expression::Literal("a".into())),
                    }]
                }]
            })
        );
    }

    #[test]
    fn test_parse_numeric_predicate() {
        let result = parse_expression("foo[1]").unwrap();
        assert_eq!(
            result,
            Expression::LocationPath(LocationPath {
                start_point: None,
                is_absolute: false,
                steps: vec![Step {
                    axis: Axis::Child,
                    node_test: NodeTest::Name("foo".into()),
                    predicates: vec![Expression::Number(1.0)]
                }]
            })
        );
    }

    #[test]
    fn test_parse_function_in_predicate() {
        let result = parse_expression("para[position()=1]").unwrap();
        assert!(result.is_location_path());
        if let Expression::LocationPath(lp) = result {
            assert_eq!(lp.steps.len(), 1);
            assert_eq!(lp.steps[0].predicates.len(), 1);
            assert!(lp.steps[0].predicates[0].is_binary_op());
        } else {
            panic!("Expected LocationPath");
        }
    }

    #[test]
    fn test_parse_text_node_test() {
        let result = parse_expression("foo/text()").unwrap();
        if let Expression::LocationPath(lp) = result {
            assert_eq!(lp.steps.len(), 2);
            assert_eq!(lp.steps[1].node_test, NodeTest::NodeType(NodeTypeTest::Text));
        } else {
            panic!("Expected location path");
        }
    }

    #[test]
    fn test_parse_abbreviated_step() {
        let result = parse_expression(".").unwrap();
        if let Expression::LocationPath(lp) = result {
            assert_eq!(lp.steps.len(), 1);
            assert_eq!(lp.steps[0].node_test, NodeTest::Name(".".to_string()));
        } else {
            panic!("Expected location path for '.'");
        }
    }

    #[test]
    fn test_parse_operator_precedence() {
        let result = parse_expression("1 + 2 * 3").unwrap();
        assert_eq!(
            result,
            Expression::BinaryOp {
                left: Box::new(Expression::Number(1.0)),
                op: BinaryOperator::Plus,
                right: Box::new(Expression::BinaryOp {
                    left: Box::new(Expression::Number(2.0)),
                    op: BinaryOperator::Multiply,
                    right: Box::new(Expression::Number(3.0)),
                }),
            }
        );
    }

    #[test]
    fn test_parse_boolean_logic() {
        let a_path = Expression::LocationPath(LocationPath { start_point: None, is_absolute: false, steps: vec![Step { axis: Axis::Child, node_test: NodeTest::Name("a".into()), predicates: vec![] }] });
        let b_path = Expression::LocationPath(LocationPath { start_point: None, is_absolute: false, steps: vec![Step { axis: Axis::Child, node_test: NodeTest::Name("b".into()), predicates: vec![] }] });
        let c_path = Expression::LocationPath(LocationPath { start_point: None, is_absolute: false, steps: vec![Step { axis: Axis::Child, node_test: NodeTest::Name("c".into()), predicates: vec![] }] });
        let d_path = Expression::LocationPath(LocationPath { start_point: None, is_absolute: false, steps: vec![Step { axis: Axis::Child, node_test: NodeTest::Name("d".into()), predicates: vec![] }] });
        let e_path = Expression::LocationPath(LocationPath { start_point: None, is_absolute: false, steps: vec![Step { axis: Axis::Child, node_test: NodeTest::Name("e".into()), predicates: vec![] }] });
        let f_path = Expression::LocationPath(LocationPath { start_point: None, is_absolute: false, steps: vec![Step { axis: Axis::Child, node_test: NodeTest::Name("f".into()), predicates: vec![] }] });

        let result = parse_expression("a = b or c = d and e = f").unwrap();
        let a_eq_b = Expression::BinaryOp { left: Box::new(a_path), op: BinaryOperator::Equals, right: Box::new(b_path) };
        let c_eq_d = Expression::BinaryOp { left: Box::new(c_path), op: BinaryOperator::Equals, right: Box::new(d_path) };
        let e_eq_f = Expression::BinaryOp { left: Box::new(e_path), op: BinaryOperator::Equals, right: Box::new(f_path) };

        assert_eq!(
            result,
            Expression::BinaryOp {
                left: Box::new(a_eq_b),
                op: BinaryOperator::Or,
                right: Box::new(Expression::BinaryOp {
                    left: Box::new(c_eq_d),
                    op: BinaryOperator::And,
                    right: Box::new(e_eq_f),
                }),
            }
        );
    }

    #[test]
    fn test_parse_descendant_or_self() {
        let result = parse_expression("//foo").unwrap();
        assert_eq!(
            result,
            Expression::LocationPath(LocationPath {
                start_point: None,
                is_absolute: true,
                steps: vec![
                    Step {
                        axis: Axis::DescendantOrSelf,
                        node_test: NodeTest::NodeType(NodeTypeTest::Node),
                        predicates: vec![]
                    },
                    Step { axis: Axis::Child, node_test: NodeTest::Name("foo".into()), predicates: vec![] },
                ]
            })
        );
    }
}