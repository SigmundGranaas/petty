//! A `nom`-based parser for the JPath expression language.
use super::ast::{Expression, PathSegment, Selection};
use crate::error::JPathError;
use nom::{
    IResult, Parser,
    branch::alt,
    bytes::complete::{is_not, tag, take_while},
    character::complete::{alpha1, char, multispace0, u64 as nom_u64},
    combinator::{map, recognize},
    multi::{many0, separated_list0},
    number::complete::double,
    sequence::{delimited, pair, preceded},
};
use serde_json::{Value, json};

// --- Main Public Parser ---

pub fn parse_expression(input: &str) -> Result<Expression, JPathError> {
    match expression(input.trim()) {
        Ok(("", expr)) => Ok(expr),
        Ok((rem, _)) => Err(JPathError::JPathParse(
            input.to_string(),
            format!("Parser did not consume all input. Remainder: '{}'", rem),
        )),
        Err(e) => Err(JPathError::JPathParse(input.to_string(), e.to_string())),
    }
}

// --- Combinators ---

fn expression(input: &str) -> IResult<&str, Expression> {
    ws(alt((
        map(literal, Expression::Literal),
        function_call, // Must be before selection to parse `func()` not `func`
        map(selection, Expression::Selection),
    )))
    .parse(input)
}

// --- Literal Parsers ---

fn boolean(input: &str) -> IResult<&str, Value> {
    alt((
        map(tag("true"), |_| json!(true)),
        map(tag("false"), |_| json!(false)),
    ))
    .parse(input)
}

fn null(input: &str) -> IResult<&str, Value> {
    map(tag("null"), |_| json!(null)).parse(input)
}

fn string_literal(input: &str) -> IResult<&str, Value> {
    map(delimited(char('\''), is_not("'"), char('\'')), |s: &str| {
        json!(s)
    })
    .parse(input)
}

fn number(input: &str) -> IResult<&str, Value> {
    map(double, Value::from).parse(input)
}

fn literal(input: &str) -> IResult<&str, Value> {
    alt((null, boolean, number, string_literal)).parse(input)
}

// --- Path/Selection Parser ---

fn identifier(input: &str) -> IResult<&str, &str> {
    recognize(pair(
        alt((alpha1, tag("_"))),
        take_while(|c: char| c.is_alphanumeric() || c == '_'),
    ))
    .parse(input)
}

fn key_segment(input: &str) -> IResult<&str, PathSegment> {
    map(preceded(char('.'), identifier), |s| {
        PathSegment::Key(s.to_string())
    })
    .parse(input)
}

fn index_segment(input: &str) -> IResult<&str, PathSegment> {
    map(delimited(char('['), nom_u64, char(']')), |i| {
        PathSegment::Index(i as usize)
    })
    .parse(input)
}

fn path_segment(input: &str) -> IResult<&str, PathSegment> {
    alt((key_segment, index_segment)).parse(input)
}

fn full_path(input: &str) -> IResult<&str, Selection> {
    map(
        pair(identifier, many0(path_segment)),
        |(start, mut rest)| {
            let mut segments = vec![PathSegment::Key(start.to_string())];
            segments.append(&mut rest);
            Selection::Path(segments)
        },
    )
    .parse(input)
}

fn selection(input: &str) -> IResult<&str, Selection> {
    alt((
        map(tag("."), |_| Selection::CurrentContext),
        map(preceded(char('$'), identifier), |name| {
            Selection::Variable(name.to_string())
        }),
        full_path,
    ))
    .parse(input)
}

// --- Function Call Parser ---

fn function_call(input: &str) -> IResult<&str, Expression> {
    let (input, name) = identifier(input)?;
    let (input, _) = multispace0(input)?;
    let (input, args) = delimited(
        char('('),
        separated_list0(ws(char(',')), expression),
        char(')'),
    )
    .parse(input)?;

    Ok((
        input,
        Expression::FunctionCall {
            name: name.to_string(),
            args,
        },
    ))
}

/// A combinator that takes a parser `inner` and produces a parser that consumes surrounding whitespace.
fn ws<'a, F, O, E>(inner: F) -> impl Parser<&'a str, Output = O, Error = E>
where
    F: Parser<&'a str, Output = O, Error = E>,
    E: nom::error::ParseError<&'a str>,
{
    delimited(multispace0, inner, multispace0)
}
