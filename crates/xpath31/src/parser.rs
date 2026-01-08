use crate::ast::*;
use crate::error::XPath31Error;
use nom::{
    IResult, Parser,
    branch::alt,
    bytes::complete::{tag, take_while, take_while1},
    character::complete::{char, digit1, multispace0},
    combinator::{map, opt, peek, recognize, value},
    multi::{many0, separated_list0, separated_list1},
    sequence::{delimited, pair, preceded},
};

pub fn parse_expression(input: &str) -> Result<Expression, XPath31Error> {
    match expr(input.trim()) {
        Ok(("", expr)) => Ok(expr),
        Ok((rem, _)) => Err(XPath31Error::parse(
            input,
            format!("Unparsed input remaining: '{}'", rem),
        )),
        Err(e) => Err(XPath31Error::parse(input, e.to_string())),
    }
}

fn ws<'a, F, O, E>(inner: F) -> impl Parser<&'a str, Output = O, Error = E>
where
    F: Parser<&'a str, Output = O, Error = E>,
    E: nom::error::ParseError<&'a str>,
{
    delimited(multispace0, inner, multispace0)
}

fn expr(input: &str) -> IResult<&str, Expression> {
    expr_single(input)
}

fn expr_single(input: &str) -> IResult<&str, Expression> {
    alt((for_expr, let_expr, quantified_expr, if_expr, or_expr)).parse(input)
}

fn for_expr(input: &str) -> IResult<&str, Expression> {
    let (input, _) = ws(tag("for")).parse(input)?;
    let (input, bindings) = separated_list1(ws(char(',')), simple_for_binding).parse(input)?;
    let (input, _) = ws(tag("return")).parse(input)?;
    let (input, return_expr) = expr_single(input)?;

    Ok((
        input,
        Expression::ForExpr {
            bindings: bindings
                .into_iter()
                .map(|(n, e)| (n, Box::new(e)))
                .collect(),
            return_expr: Box::new(return_expr),
        },
    ))
}

fn simple_for_binding(input: &str) -> IResult<&str, (String, Expression)> {
    let (input, _) = ws(char('$')).parse(input)?;
    let (input, name) = var_name(input)?;
    let (input, _) = ws(tag("in")).parse(input)?;
    let (input, expr) = expr_single(input)?;
    Ok((input, (name, expr)))
}

fn let_expr(input: &str) -> IResult<&str, Expression> {
    let (input, _) = ws(tag("let")).parse(input)?;
    let (input, bindings) = separated_list1(ws(char(',')), simple_let_binding).parse(input)?;
    let (input, _) = ws(tag("return")).parse(input)?;
    let (input, return_expr) = expr_single(input)?;

    Ok((
        input,
        Expression::LetExpr {
            bindings: bindings
                .into_iter()
                .map(|(n, e)| (n, Box::new(e)))
                .collect(),
            return_expr: Box::new(return_expr),
        },
    ))
}

fn simple_let_binding(input: &str) -> IResult<&str, (String, Expression)> {
    let (input, _) = ws(char('$')).parse(input)?;
    let (input, name) = var_name(input)?;
    let (input, _) = ws(tag(":=")).parse(input)?;
    let (input, expr) = expr_single(input)?;
    Ok((input, (name, expr)))
}

fn quantified_expr(input: &str) -> IResult<&str, Expression> {
    let (input, quantifier) = alt((
        value(Quantifier::Some, ws(tag("some"))),
        value(Quantifier::Every, ws(tag("every"))),
    ))
    .parse(input)?;

    let (input, bindings) = separated_list1(ws(char(',')), simple_for_binding).parse(input)?;
    let (input, _) = ws(tag("satisfies")).parse(input)?;
    let (input, satisfies) = expr_single(input)?;

    Ok((
        input,
        Expression::QuantifiedExpr {
            quantifier,
            bindings: bindings
                .into_iter()
                .map(|(n, e)| (n, Box::new(e)))
                .collect(),
            satisfies: Box::new(satisfies),
        },
    ))
}

fn if_expr(input: &str) -> IResult<&str, Expression> {
    let (input, _) = ws(tag("if")).parse(input)?;
    let (input, condition) = delimited(ws(char('(')), expr, ws(char(')'))).parse(input)?;
    let (input, _) = ws(tag("then")).parse(input)?;
    let (input, then_expr) = expr_single(input)?;
    let (input, _) = ws(tag("else")).parse(input)?;
    let (input, else_expr) = expr_single(input)?;

    Ok((
        input,
        Expression::IfExpr {
            condition: Box::new(condition),
            then_expr: Box::new(then_expr),
            else_expr: Box::new(else_expr),
        },
    ))
}

fn or_expr(input: &str) -> IResult<&str, Expression> {
    let (input, first) = and_expr(input)?;
    let (input, rest) = many0(preceded(ws(tag("or")), and_expr)).parse(input)?;

    Ok((input, fold_binary(first, rest, BinaryOperator::Or)))
}

fn and_expr(input: &str) -> IResult<&str, Expression> {
    let (input, first) = comparison_expr(input)?;
    let (input, rest) = many0(preceded(ws(tag("and")), comparison_expr)).parse(input)?;

    Ok((input, fold_binary(first, rest, BinaryOperator::And)))
}

fn comparison_expr(input: &str) -> IResult<&str, Expression> {
    let (input, first) = string_concat_expr(input)?;
    let (input, rest) = opt(pair(
        ws(alt((
            value(BinaryOperator::Equals, tag("=")),
            value(BinaryOperator::NotEquals, tag("!=")),
            value(
                BinaryOperator::LessThanOrEqual,
                alt((tag("<="), tag("&lt;="))),
            ),
            value(BinaryOperator::LessThan, alt((tag("<"), tag("&lt;")))),
            value(
                BinaryOperator::GreaterThanOrEqual,
                alt((tag(">="), tag("&gt;="))),
            ),
            value(BinaryOperator::GreaterThan, alt((tag(">"), tag("&gt;")))),
        ))),
        string_concat_expr,
    ))
    .parse(input)?;

    match rest {
        Some((op, right)) => Ok((
            input,
            Expression::BinaryOp {
                left: Box::new(first),
                op,
                right: Box::new(right),
            },
        )),
        None => Ok((input, first)),
    }
}

fn string_concat_expr(input: &str) -> IResult<&str, Expression> {
    let (input, first) = range_expr(input)?;
    let (input, rest) = many0(preceded(ws(tag("||")), range_expr)).parse(input)?;

    if rest.is_empty() {
        Ok((input, first))
    } else {
        let mut result = first;
        for right in rest {
            result = Expression::StringConcat {
                left: Box::new(result),
                right: Box::new(right),
            };
        }
        Ok((input, result))
    }
}

fn range_expr(input: &str) -> IResult<&str, Expression> {
    let (input, first) = additive_expr(input)?;
    let (input, rest) = opt(preceded(ws(tag("to")), additive_expr)).parse(input)?;

    match rest {
        Some(end) => Ok((
            input,
            Expression::RangeExpr {
                start: Box::new(first),
                end: Box::new(end),
            },
        )),
        None => Ok((input, first)),
    }
}

fn additive_expr(input: &str) -> IResult<&str, Expression> {
    let (input, first) = multiplicative_expr(input)?;
    let (input, rest) = many0(pair(
        ws(alt((
            value(BinaryOperator::Plus, char('+')),
            value(BinaryOperator::Minus, char('-')),
        ))),
        multiplicative_expr,
    ))
    .parse(input)?;

    let mut result = first;
    for (op, right) in rest {
        result = Expression::BinaryOp {
            left: Box::new(result),
            op,
            right: Box::new(right),
        };
    }
    Ok((input, result))
}

fn multiplicative_expr(input: &str) -> IResult<&str, Expression> {
    let (input, first) = union_expr(input)?;
    let (input, rest) = many0(pair(
        ws(alt((
            value(BinaryOperator::Multiply, char('*')),
            value(BinaryOperator::Divide, tag("div")),
            value(BinaryOperator::Modulo, tag("mod")),
        ))),
        union_expr,
    ))
    .parse(input)?;

    let mut result = first;
    for (op, right) in rest {
        result = Expression::BinaryOp {
            left: Box::new(result),
            op,
            right: Box::new(right),
        };
    }
    Ok((input, result))
}

fn union_expr(input: &str) -> IResult<&str, Expression> {
    let (input, first) = instanceof_expr(input)?;
    let (input, rest) =
        many0(preceded(ws(alt((tag("union"), tag("|")))), instanceof_expr)).parse(input)?;

    Ok((input, fold_binary(first, rest, BinaryOperator::Union)))
}

fn instanceof_expr(input: &str) -> IResult<&str, Expression> {
    let (input, expr) = treat_expr(input)?;
    let (input, type_decl) = opt(preceded(
        pair(ws(tag("instance")), ws(tag("of"))),
        sequence_type,
    ))
    .parse(input)?;

    match type_decl {
        Some(seq_type) => Ok((
            input,
            Expression::InstanceOf {
                expr: Box::new(expr),
                sequence_type: seq_type,
            },
        )),
        None => Ok((input, expr)),
    }
}

fn treat_expr(input: &str) -> IResult<&str, Expression> {
    let (input, expr) = castable_expr(input)?;
    let (input, type_decl) = opt(preceded(
        pair(ws(tag("treat")), ws(tag("as"))),
        sequence_type,
    ))
    .parse(input)?;

    match type_decl {
        Some(seq_type) => Ok((
            input,
            Expression::TreatAs {
                expr: Box::new(expr),
                sequence_type: seq_type,
            },
        )),
        None => Ok((input, expr)),
    }
}

fn castable_expr(input: &str) -> IResult<&str, Expression> {
    let (input, expr) = cast_expr(input)?;
    let (input, type_decl) = opt(preceded(
        pair(ws(tag("castable")), ws(tag("as"))),
        single_type,
    ))
    .parse(input)?;

    match type_decl {
        Some(s_type) => Ok((
            input,
            Expression::CastableAs {
                expr: Box::new(expr),
                single_type: s_type,
            },
        )),
        None => Ok((input, expr)),
    }
}

fn cast_expr(input: &str) -> IResult<&str, Expression> {
    let (input, expr) = unary_expr(input)?;
    let (input, type_decl) =
        opt(preceded(pair(ws(tag("cast")), ws(tag("as"))), single_type)).parse(input)?;

    match type_decl {
        Some(s_type) => Ok((
            input,
            Expression::CastAs {
                expr: Box::new(expr),
                single_type: s_type,
            },
        )),
        None => Ok((input, expr)),
    }
}

fn sequence_type(input: &str) -> IResult<&str, SequenceType> {
    let (input, item_t) = item_type(input)?;
    let (input, occur) = opt(ws(alt((
        value(OccurrenceIndicator::ZeroOrOne, char('?')),
        value(OccurrenceIndicator::ZeroOrMore, char('*')),
        value(OccurrenceIndicator::OneOrMore, char('+')),
    ))))
    .parse(input)?;

    Ok((
        input,
        SequenceType {
            item_type: item_t,
            occurrence: occur.unwrap_or(OccurrenceIndicator::ExactlyOne),
        },
    ))
}

fn item_type(input: &str) -> IResult<&str, ItemType> {
    alt((
        map(tag("item()"), |_| ItemType::Item),
        map(preceded(tag("map("), ws(char(')'))), |_| {
            ItemType::MapTest(None, None)
        }),
        map(preceded(tag("array("), ws(char(')'))), |_| {
            ItemType::ArrayTest(None)
        }),
        map(preceded(tag("function("), ws(char(')'))), |_| {
            ItemType::FunctionTest(None, None)
        }),
        map(tag("node()"), |_| ItemType::KindTest(KindTest::AnyKindTest)),
        map(tag("element()"), |_| {
            ItemType::KindTest(KindTest::Element(None, None))
        }),
        map(tag("attribute()"), |_| {
            ItemType::KindTest(KindTest::Attribute(None, None))
        }),
        map(tag("text()"), |_| ItemType::KindTest(KindTest::TextTest)),
        map(tag("comment()"), |_| {
            ItemType::KindTest(KindTest::CommentTest)
        }),
        map(tag("document-node()"), |_| {
            ItemType::KindTest(KindTest::Document(None))
        }),
        map(qname, ItemType::AtomicOrUnion),
    ))
    .parse(input)
}

fn single_type(input: &str) -> IResult<&str, SingleType> {
    let (input, type_name) = qname(input)?;
    let (input, optional) = opt(ws(char('?'))).parse(input)?;

    Ok((
        input,
        SingleType {
            type_name,
            optional: optional.is_some(),
        },
    ))
}

fn unary_expr(input: &str) -> IResult<&str, Expression> {
    let (input, sign) = opt(ws(alt((char('-'), char('+'))))).parse(input)?;
    let (input, expr) = arrow_expr(input)?;

    match sign {
        Some('-') => Ok((
            input,
            Expression::UnaryOp {
                op: UnaryOperator::Minus,
                expr: Box::new(expr),
            },
        )),
        Some('+') => Ok((
            input,
            Expression::UnaryOp {
                op: UnaryOperator::Plus,
                expr: Box::new(expr),
            },
        )),
        _ => Ok((input, expr)),
    }
}

fn arrow_expr(input: &str) -> IResult<&str, Expression> {
    let (input, base) = simple_map_expr(input)?;
    let (input, steps) = many0(arrow_step).parse(input)?;

    if steps.is_empty() {
        Ok((input, base))
    } else {
        Ok((
            input,
            Expression::ArrowExpr {
                base: Box::new(base),
                steps,
            },
        ))
    }
}

fn arrow_step(input: &str) -> IResult<&str, ArrowStep> {
    let (input, _) = ws(tag("=>")).parse(input)?;
    let (input, name) = qname(input)?;
    let (input, args) = delimited(
        ws(char('(')),
        separated_list0(ws(char(',')), expr_single),
        ws(char(')')),
    )
    .parse(input)?;

    Ok((
        input,
        ArrowStep {
            function_name: name,
            args,
        },
    ))
}

fn simple_map_expr(input: &str) -> IResult<&str, Expression> {
    let (input, first) = postfix_expr(input)?;
    let (input, rest) = many0(preceded(ws(char('!')), postfix_expr)).parse(input)?;

    if rest.is_empty() {
        Ok((input, first))
    } else {
        let mut result = first;
        for right in rest {
            result = Expression::SimpleMapExpr {
                base: Box::new(result),
                mapping: Box::new(right),
            };
        }
        Ok((input, result))
    }
}

fn postfix_expr(input: &str) -> IResult<&str, Expression> {
    let (input, base) = primary_expr(input)?;
    let (input, postfixes) = many0(alt((
        map(predicate, PostfixOp::Predicate),
        map(argument_list, PostfixOp::ArgumentList),
        map(lookup, PostfixOp::Lookup),
    )))
    .parse(input)?;

    let mut result = base;
    for postfix in postfixes {
        result = match postfix {
            PostfixOp::Predicate(pred) => {
                if let Expression::LocationPath(mut lp) = result {
                    if let Some(step) = lp.steps.last_mut() {
                        step.predicates.push(convert_to_xpath1_expr(&pred));
                    }
                    Expression::LocationPath(lp)
                } else if let Expression::FilterExpr {
                    base,
                    mut predicates,
                } = result
                {
                    predicates.push(pred);
                    Expression::FilterExpr { base, predicates }
                } else {
                    Expression::FilterExpr {
                        base: Box::new(result),
                        predicates: vec![pred],
                    }
                }
            }
            PostfixOp::ArgumentList(args) => match &result {
                Expression::Variable(_)
                | Expression::LookupExpr { .. }
                | Expression::FilterExpr { .. } => Expression::DynamicFunctionCall {
                    function_expr: Box::new(result),
                    args,
                },
                _ => result,
            },
            PostfixOp::Lookup(key) => Expression::LookupExpr {
                base: Box::new(result),
                key,
            },
        };
    }
    Ok((input, result))
}

enum PostfixOp {
    Predicate(Expression),
    ArgumentList(Vec<Expression>),
    Lookup(LookupKey),
}

fn predicate(input: &str) -> IResult<&str, Expression> {
    delimited(ws(char('[')), expr, ws(char(']'))).parse(input)
}

fn argument_list(input: &str) -> IResult<&str, Vec<Expression>> {
    delimited(
        ws(char('(')),
        separated_list0(ws(char(',')), argument),
        ws(char(')')),
    )
    .parse(input)
}

fn argument(input: &str) -> IResult<&str, Expression> {
    alt((
        value(Expression::ArgumentPlaceholder, ws(char('?'))),
        expr_single,
    ))
    .parse(input)
}

fn lookup(input: &str) -> IResult<&str, LookupKey> {
    preceded(ws(char('?')), key_specifier).parse(input)
}

fn key_specifier(input: &str) -> IResult<&str, LookupKey> {
    alt((
        value(LookupKey::Wildcard, char('*')),
        map(integer_literal, LookupKey::Integer),
        map(nc_name, |s| LookupKey::NCName(s.to_string())),
        map(delimited(ws(char('(')), expr, ws(char(')'))), |e| {
            LookupKey::Parenthesized(Box::new(e))
        }),
    ))
    .parse(input)
}

fn primary_expr(input: &str) -> IResult<&str, Expression> {
    ws(alt((
        map_constructor,
        array_constructor,
        inline_function,
        function_call,
        variable_reference,
        context_item_expr,
        parenthesized_expr,
        literal,
        function_item_expr,
        location_path_expr,
    )))
    .parse(input)
}

fn location_path_expr(input: &str) -> IResult<&str, Expression> {
    let (input, double_slash) = opt(tag("//")).parse(input)?;

    if double_slash.is_some() {
        let (input, first_step) = path_step(input)?;
        let (input, rest) = many0(path_step_with_separator).parse(input)?;
        let desc_step = Step {
            axis: Axis::DescendantOrSelf,
            node_test: NodeTest::NodeType(NodeTypeTest::Node),
            predicates: vec![],
        };
        let mut steps = vec![desc_step, first_step];
        steps.extend(rest);
        return Ok((
            input,
            Expression::LocationPath(LocationPath {
                start_point: None,
                is_absolute: true,
                steps,
            }),
        ));
    }

    let (input, is_absolute) = opt(char('/')).parse(input)?;
    let (input, first_step) = opt(path_step).parse(input)?;

    match (is_absolute, first_step) {
        (Some(_), Some(step)) => {
            let (input, rest) = many0(path_step_with_separator).parse(input)?;
            let mut steps = vec![step];
            steps.extend(rest);
            Ok((
                input,
                Expression::LocationPath(LocationPath {
                    start_point: None,
                    is_absolute: true,
                    steps,
                }),
            ))
        }
        (Some(_), None) => Ok((
            input,
            Expression::LocationPath(LocationPath {
                start_point: None,
                is_absolute: true,
                steps: vec![],
            }),
        )),
        (None, Some(step)) => {
            let (input, rest) = many0(path_step_with_separator).parse(input)?;
            let mut steps = vec![step];
            steps.extend(rest);
            Ok((
                input,
                Expression::LocationPath(LocationPath {
                    start_point: None,
                    is_absolute: false,
                    steps,
                }),
            ))
        }
        (None, None) => Err(nom::Err::Error(nom::error::Error::new(
            input,
            nom::error::ErrorKind::Alt,
        ))),
    }
}

fn path_step_with_separator(input: &str) -> IResult<&str, Step> {
    let (input, double_slash) = opt(tag("//")).parse(input)?;

    if double_slash.is_some() {
        let (input, step) = path_step(input)?;
        return Ok((
            input,
            Step {
                axis: Axis::DescendantOrSelf,
                node_test: step.node_test,
                predicates: step.predicates,
            },
        ));
    }

    let (input, _) = char('/').parse(input)?;
    path_step(input)
}

fn path_step(input: &str) -> IResult<&str, Step> {
    let (input, axis) = opt(axis_specifier).parse(input)?;
    let (input, node_test) = node_test(input)?;
    let (input, predicates) = many0(predicate_xpath1).parse(input)?;

    let axis = axis.unwrap_or(Axis::Child);
    Ok((
        input,
        Step {
            axis,
            node_test,
            predicates,
        },
    ))
}

fn axis_specifier(input: &str) -> IResult<&str, Axis> {
    alt((
        value(Axis::Child, pair(tag("child"), tag("::"))),
        value(Axis::Parent, pair(tag("parent"), tag("::"))),
        value(Axis::SelfAxis, pair(tag("self"), tag("::"))),
        value(Axis::Ancestor, pair(tag("ancestor-or-self"), tag("::"))),
        value(Axis::Ancestor, pair(tag("ancestor"), tag("::"))),
        value(
            Axis::DescendantOrSelf,
            pair(tag("descendant-or-self"), tag("::")),
        ),
        value(Axis::Descendant, pair(tag("descendant"), tag("::"))),
        value(Axis::Following, pair(tag("following-sibling"), tag("::"))),
        value(
            Axis::FollowingSibling,
            pair(tag("following-sibling"), tag("::")),
        ),
        value(Axis::Following, pair(tag("following"), tag("::"))),
        value(
            Axis::PrecedingSibling,
            pair(tag("preceding-sibling"), tag("::")),
        ),
        value(Axis::Preceding, pair(tag("preceding"), tag("::"))),
        value(Axis::Attribute, pair(tag("attribute"), tag("::"))),
        value(Axis::Attribute, char('@')),
        value(Axis::Parent, tag("..")),
    ))
    .parse(input)
}

fn node_test(input: &str) -> IResult<&str, NodeTest> {
    alt((
        node_type_test,
        map(char('*'), |_| NodeTest::Wildcard),
        map(qname, |q| {
            NodeTest::Name(q.prefix.map(|p| format!("{}:", p)).unwrap_or_default() + &q.local_part)
        }),
    ))
    .parse(input)
}

fn node_type_test(input: &str) -> IResult<&str, NodeTest> {
    let (input, kind) = alt((
        value(NodeTypeTest::Comment, tag("comment")),
        value(NodeTypeTest::Text, tag("text")),
        value(
            NodeTypeTest::ProcessingInstruction,
            tag("processing-instruction"),
        ),
        value(NodeTypeTest::Node, tag("node")),
    ))
    .parse(input)?;
    let (input, _) = tag("()").parse(input)?;
    Ok((input, NodeTest::NodeType(kind)))
}

fn predicate_xpath1(input: &str) -> IResult<&str, petty_xpath1::ast::Expression> {
    let (input, pred) = predicate(input)?;
    Ok((input, convert_to_xpath1_expr(&pred)))
}

fn literal(input: &str) -> IResult<&str, Expression> {
    alt((
        map(string_literal, |s| Expression::Literal(Literal::String(s))),
        numeric_literal,
    ))
    .parse(input)
}

fn string_literal(input: &str) -> IResult<&str, String> {
    alt((
        delimited(char('\''), take_while(|c| c != '\''), char('\'')),
        delimited(char('"'), take_while(|c| c != '"'), char('"')),
    ))
    .map(|s: &str| s.to_string())
    .parse(input)
}

fn numeric_literal(input: &str) -> IResult<&str, Expression> {
    alt((double_literal, decimal_literal, integer_literal_expr)).parse(input)
}

fn integer_literal(input: &str) -> IResult<&str, i64> {
    map(digit1, |s: &str| s.parse::<i64>().unwrap_or(0)).parse(input)
}

fn integer_literal_expr(input: &str) -> IResult<&str, Expression> {
    map(integer_literal, |i| {
        Expression::Literal(Literal::Integer(i))
    })
    .parse(input)
}

fn decimal_literal(input: &str) -> IResult<&str, Expression> {
    let (input, s) = recognize((opt(digit1), char('.'), digit1)).parse(input)?;

    if input.starts_with('e') || input.starts_with('E') {
        return Err(nom::Err::Error(nom::error::Error::new(
            input,
            nom::error::ErrorKind::Verify,
        )));
    }

    Ok((input, Expression::Literal(Literal::Decimal(s.to_string()))))
}

fn double_literal(input: &str) -> IResult<&str, Expression> {
    let (input, s) = recognize((
        alt((
            recognize((digit1, char('.'), opt(digit1))),
            recognize((char('.'), digit1)),
            digit1,
        )),
        alt((char('e'), char('E'))),
        opt(alt((char('+'), char('-')))),
        digit1,
    ))
    .parse(input)?;

    let d: f64 = s.parse().unwrap_or(f64::NAN);
    Ok((input, Expression::Literal(Literal::Double(d))))
}

fn variable_reference(input: &str) -> IResult<&str, Expression> {
    map(preceded(char('$'), var_name), Expression::Variable).parse(input)
}

fn var_name(input: &str) -> IResult<&str, String> {
    map(qname_str, |s| s.to_string()).parse(input)
}

fn context_item_expr(input: &str) -> IResult<&str, Expression> {
    let (input, _) = char('.').parse(input)?;
    if let Some(c) = input.chars().next()
        && (c.is_alphanumeric() || c == '_')
    {
        return Err(nom::Err::Error(nom::error::Error::new(
            input,
            nom::error::ErrorKind::Verify,
        )));
    }
    Ok((input, Expression::ContextItem))
}

fn parenthesized_expr(input: &str) -> IResult<&str, Expression> {
    let (input, items) = delimited(
        ws(char('(')),
        separated_list0(ws(char(',')), expr_single),
        ws(char(')')),
    )
    .parse(input)?;

    let expr = match &items[..] {
        [] => Expression::Sequence(vec![]),
        [single] => single.clone(),
        _ => Expression::Sequence(items),
    };
    Ok((input, expr))
}

fn map_constructor(input: &str) -> IResult<&str, Expression> {
    let (input, _) = ws(tag("map")).parse(input)?;
    let (input, entries) = delimited(
        ws(char('{')),
        separated_list0(ws(char(',')), map_entry),
        ws(char('}')),
    )
    .parse(input)?;

    Ok((input, Expression::MapConstructor(entries)))
}

fn map_entry(input: &str) -> IResult<&str, MapEntry> {
    let (input, key) = expr_single(input)?;
    let (input, _) = ws(char(':')).parse(input)?;
    let (input, value) = expr_single(input)?;

    Ok((
        input,
        MapEntry {
            key: Box::new(key),
            value: Box::new(value),
        },
    ))
}

fn array_constructor(input: &str) -> IResult<&str, Expression> {
    alt((square_array_constructor, curly_array_constructor)).parse(input)
}

fn square_array_constructor(input: &str) -> IResult<&str, Expression> {
    let (input, members) = delimited(
        ws(char('[')),
        separated_list0(ws(char(',')), expr_single),
        ws(char(']')),
    )
    .parse(input)?;

    Ok((
        input,
        Expression::ArrayConstructor(ArrayConstructorKind::Square(members)),
    ))
}

fn curly_array_constructor(input: &str) -> IResult<&str, Expression> {
    let (input, _) = ws(tag("array")).parse(input)?;
    let (input, enclosed) = delimited(ws(char('{')), expr, ws(char('}'))).parse(input)?;

    Ok((
        input,
        Expression::ArrayConstructor(ArrayConstructorKind::Curly(Box::new(enclosed))),
    ))
}

fn inline_function(input: &str) -> IResult<&str, Expression> {
    let (input, _) = ws(tag("function")).parse(input)?;
    let (input, params) = delimited(
        ws(char('(')),
        separated_list0(ws(char(',')), param),
        ws(char(')')),
    )
    .parse(input)?;
    let (input, return_type) = opt(preceded(ws(tag("as")), sequence_type)).parse(input)?;
    let (input, body) = delimited(ws(char('{')), expr, ws(char('}'))).parse(input)?;

    Ok((
        input,
        Expression::InlineFunction {
            params,
            return_type,
            body: Box::new(body),
        },
    ))
}

fn param(input: &str) -> IResult<&str, Param> {
    let (input, _) = ws(char('$')).parse(input)?;
    let (input, name) = var_name(input)?;
    let (input, type_decl) = opt(preceded(ws(tag("as")), sequence_type)).parse(input)?;

    Ok((input, Param { name, type_decl }))
}

fn function_call(input: &str) -> IResult<&str, Expression> {
    let (input, name) = qname(input)?;
    let (input, _) = peek(ws(char('('))).parse(input)?;

    let reserved = [
        "if", "for", "let", "some", "every", "function", "map", "array",
    ];
    if reserved.contains(&name.local_part.as_str()) {
        return Err(nom::Err::Error(nom::error::Error::new(
            input,
            nom::error::ErrorKind::Verify,
        )));
    }

    let (input, args) = argument_list(input)?;
    Ok((input, Expression::FunctionCall { name, args }))
}

fn function_item_expr(input: &str) -> IResult<&str, Expression> {
    let (input, name) = qname(input)?;
    let (input, _) = ws(char('#')).parse(input)?;
    let (input, arity) = integer_literal(input)?;

    Ok((
        input,
        Expression::NamedFunctionRef {
            name,
            arity: arity as usize,
        },
    ))
}

fn qname(input: &str) -> IResult<&str, QName> {
    let (input, first) = nc_name(input)?;
    let (input, second) = opt(preceded(char(':'), nc_name)).parse(input)?;

    match second {
        Some(local) => Ok((
            input,
            QName {
                prefix: Some(first.to_string()),
                local_part: local.to_string(),
            },
        )),
        None => Ok((
            input,
            QName {
                prefix: None,
                local_part: first.to_string(),
            },
        )),
    }
}

fn qname_str(input: &str) -> IResult<&str, &str> {
    recognize(pair(nc_name, opt(pair(char(':'), nc_name)))).parse(input)
}

fn nc_name(input: &str) -> IResult<&str, &str> {
    recognize(pair(
        take_while1(|c: char| c.is_alphabetic() || c == '_'),
        take_while(|c: char| c.is_alphanumeric() || c == '_' || c == '-'),
    ))
    .parse(input)
}

fn fold_binary(first: Expression, rest: Vec<Expression>, op: BinaryOperator) -> Expression {
    rest.into_iter()
        .fold(first, |acc, right| Expression::BinaryOp {
            left: Box::new(acc),
            op,
            right: Box::new(right),
        })
}

fn convert_to_xpath1_expr(expr: &Expression) -> petty_xpath1::ast::Expression {
    match expr {
        Expression::Literal(Literal::String(s)) => {
            petty_xpath1::ast::Expression::Literal(s.clone())
        }
        Expression::Literal(Literal::Integer(i)) => {
            petty_xpath1::ast::Expression::Number(*i as f64)
        }
        Expression::Literal(Literal::Double(d)) => petty_xpath1::ast::Expression::Number(*d),
        Expression::Literal(Literal::Decimal(s)) => {
            petty_xpath1::ast::Expression::Number(s.parse().unwrap_or(0.0))
        }
        Expression::Variable(name) => petty_xpath1::ast::Expression::Variable(name.clone()),
        Expression::BinaryOp { left, op, right } => petty_xpath1::ast::Expression::BinaryOp {
            left: Box::new(convert_to_xpath1_expr(left)),
            op: *op,
            right: Box::new(convert_to_xpath1_expr(right)),
        },
        Expression::FunctionCall { name, args } => petty_xpath1::ast::Expression::FunctionCall {
            name: name.to_string(),
            args: args.iter().map(convert_to_xpath1_expr).collect(),
        },
        Expression::LocationPath(lp) => petty_xpath1::ast::Expression::LocationPath(lp.clone()),
        Expression::UnaryOp { op, expr } => petty_xpath1::ast::Expression::UnaryOp {
            op: *op,
            expr: Box::new(convert_to_xpath1_expr(expr)),
        },
        Expression::ContextItem => petty_xpath1::ast::Expression::LocationPath(LocationPath {
            start_point: None,
            is_absolute: false,
            steps: vec![Step {
                axis: Axis::SelfAxis,
                node_test: NodeTest::NodeType(NodeTypeTest::Node),
                predicates: vec![],
            }],
        }),
        Expression::Sequence(items) if items.len() == 1 => convert_to_xpath1_expr(&items[0]),
        Expression::FilterExpr { base, predicates } => {
            let base_expr = convert_to_xpath1_expr(base);
            if let petty_xpath1::ast::Expression::LocationPath(mut lp) = base_expr {
                if let Some(step) = lp.steps.last_mut() {
                    for pred in predicates {
                        step.predicates.push(convert_to_xpath1_expr(pred));
                    }
                }
                petty_xpath1::ast::Expression::LocationPath(lp)
            } else {
                base_expr
            }
        }
        Expression::StringConcat { left, right } => petty_xpath1::ast::Expression::FunctionCall {
            name: "concat".to_string(),
            args: vec![convert_to_xpath1_expr(left), convert_to_xpath1_expr(right)],
        },
        Expression::RangeExpr { .. }
        | Expression::LetExpr { .. }
        | Expression::IfExpr { .. }
        | Expression::ForExpr { .. }
        | Expression::QuantifiedExpr { .. }
        | Expression::MapConstructor(_)
        | Expression::ArrayConstructor(_)
        | Expression::InlineFunction { .. }
        | Expression::NamedFunctionRef { .. }
        | Expression::ArrowExpr { .. }
        | Expression::SimpleMapExpr { .. }
        | Expression::LookupExpr { .. }
        | Expression::UnaryLookup(_)
        | Expression::InstanceOf { .. }
        | Expression::TreatAs { .. }
        | Expression::CastAs { .. }
        | Expression::CastableAs { .. }
        | Expression::DynamicFunctionCall { .. }
        | Expression::Sequence(_)
        | Expression::ArgumentPlaceholder => petty_xpath1::ast::Expression::Literal(String::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_integer_literal() {
        let expr = parse_expression("42").unwrap();
        assert!(matches!(expr, Expression::Literal(Literal::Integer(42))));
    }

    #[test]
    fn test_parse_string_literal() {
        let expr = parse_expression("'hello'").unwrap();
        assert!(matches!(expr, Expression::Literal(Literal::String(s)) if s == "hello"));

        let expr = parse_expression("\"world\"").unwrap();
        assert!(matches!(expr, Expression::Literal(Literal::String(s)) if s == "world"));
    }

    #[test]
    fn test_parse_variable() {
        let expr = parse_expression("$x").unwrap();
        assert!(matches!(expr, Expression::Variable(n) if n == "x"));
    }

    #[test]
    fn test_parse_let_expression() {
        let expr = parse_expression("let $x := 5 return $x * 2").unwrap();
        assert!(matches!(expr, Expression::LetExpr { .. }));
    }

    #[test]
    fn test_parse_let_chained() {
        let expr = parse_expression("let $x := 3, $y := 4 return $x + $y").unwrap();
        if let Expression::LetExpr { bindings, .. } = expr {
            assert_eq!(bindings.len(), 2);
        } else {
            panic!("Expected LetExpr");
        }
    }

    #[test]
    fn test_parse_if_expression() {
        let expr = parse_expression("if (true()) then 1 else 2").unwrap();
        assert!(matches!(expr, Expression::IfExpr { .. }));
    }

    #[test]
    fn test_parse_for_expression() {
        let expr = parse_expression("for $i in 1 to 5 return $i * 2").unwrap();
        assert!(matches!(expr, Expression::ForExpr { .. }));
    }

    #[test]
    fn test_parse_quantified_some() {
        let expr = parse_expression("some $x in (1, 2, 3) satisfies $x > 2").unwrap();
        if let Expression::QuantifiedExpr { quantifier, .. } = expr {
            assert_eq!(quantifier, Quantifier::Some);
        } else {
            panic!("Expected QuantifiedExpr");
        }
    }

    #[test]
    fn test_parse_quantified_every() {
        let expr = parse_expression("every $x in (1, 2, 3) satisfies $x > 0").unwrap();
        if let Expression::QuantifiedExpr { quantifier, .. } = expr {
            assert_eq!(quantifier, Quantifier::Every);
        } else {
            panic!("Expected QuantifiedExpr");
        }
    }

    #[test]
    fn test_parse_map_constructor() {
        let expr = parse_expression("map { 'a': 1, 'b': 2 }").unwrap();
        if let Expression::MapConstructor(entries) = expr {
            assert_eq!(entries.len(), 2);
        } else {
            panic!("Expected MapConstructor");
        }
    }

    #[test]
    fn test_parse_array_constructor_square() {
        let expr = parse_expression("[1, 2, 3]").unwrap();
        if let Expression::ArrayConstructor(ArrayConstructorKind::Square(members)) = expr {
            assert_eq!(members.len(), 3);
        } else {
            panic!("Expected Square ArrayConstructor");
        }
    }

    #[test]
    fn test_parse_array_constructor_curly() {
        let expr = parse_expression("array { 1 to 5 }").unwrap();
        assert!(matches!(
            expr,
            Expression::ArrayConstructor(ArrayConstructorKind::Curly(_))
        ));
    }

    #[test]
    fn test_parse_arrow_expression() {
        let expr = parse_expression("'hello' => upper-case()").unwrap();
        if let Expression::ArrowExpr { steps, .. } = expr {
            assert_eq!(steps.len(), 1);
            assert_eq!(steps[0].function_name.local_part, "upper-case");
        } else {
            panic!("Expected ArrowExpr");
        }
    }

    #[test]
    fn test_parse_simple_map() {
        let expr = parse_expression("(1, 2, 3) ! (. * 2)").unwrap();
        assert!(matches!(expr, Expression::SimpleMapExpr { .. }));
    }

    #[test]
    fn test_parse_lookup() {
        let expr = parse_expression("$map?key").unwrap();
        assert!(matches!(
            expr,
            Expression::LookupExpr {
                key: LookupKey::NCName(_),
                ..
            }
        ));

        let expr = parse_expression("$array?1").unwrap();
        assert!(matches!(
            expr,
            Expression::LookupExpr {
                key: LookupKey::Integer(1),
                ..
            }
        ));
    }

    #[test]
    fn test_parse_string_concat() {
        let expr = parse_expression("'a' || 'b'").unwrap();
        assert!(matches!(expr, Expression::StringConcat { .. }));
    }

    #[test]
    fn test_parse_range() {
        let expr = parse_expression("1 to 10").unwrap();
        assert!(matches!(expr, Expression::RangeExpr { .. }));
    }

    #[test]
    fn test_parse_function_call() {
        let expr = parse_expression("concat('a', 'b', 'c')").unwrap();
        if let Expression::FunctionCall { name, args } = expr {
            assert_eq!(name.local_part, "concat");
            assert_eq!(args.len(), 3);
        } else {
            panic!("Expected FunctionCall");
        }
    }

    #[test]
    fn test_parse_inline_function() {
        let expr = parse_expression("function($x) { $x * 2 }").unwrap();
        if let Expression::InlineFunction { params, .. } = expr {
            assert_eq!(params.len(), 1);
            assert_eq!(params[0].name, "x");
        } else {
            panic!("Expected InlineFunction");
        }
    }

    #[test]
    fn test_parse_named_function_ref() {
        let expr = parse_expression("fn:concat#3").unwrap();
        if let Expression::NamedFunctionRef { name, arity } = expr {
            assert_eq!(name.prefix, Some("fn".to_string()));
            assert_eq!(name.local_part, "concat");
            assert_eq!(arity, 3);
        } else {
            panic!("Expected NamedFunctionRef");
        }
    }

    #[test]
    fn test_parse_context_item() {
        let expr = parse_expression(".").unwrap();
        assert!(matches!(expr, Expression::ContextItem));
    }

    #[test]
    fn test_parse_empty_sequence() {
        let expr = parse_expression("()").unwrap();
        assert!(matches!(expr, Expression::Sequence(v) if v.is_empty()));
    }

    #[test]
    fn test_parse_arithmetic() {
        let expr = parse_expression("1 + 2 * 3").unwrap();
        if let Expression::BinaryOp {
            op: BinaryOperator::Plus,
            right,
            ..
        } = expr
        {
            assert!(matches!(
                *right,
                Expression::BinaryOp {
                    op: BinaryOperator::Multiply,
                    ..
                }
            ));
        } else {
            panic!("Expected correct precedence");
        }
    }

    #[test]
    fn test_parse_comparison() {
        let expr = parse_expression("$x = 5").unwrap();
        assert!(matches!(
            expr,
            Expression::BinaryOp {
                op: BinaryOperator::Equals,
                ..
            }
        ));
    }

    #[test]
    fn test_parse_and_or() {
        let expr = parse_expression("true() and false() or true()").unwrap();
        assert!(matches!(
            expr,
            Expression::BinaryOp {
                op: BinaryOperator::Or,
                ..
            }
        ));
    }
}
