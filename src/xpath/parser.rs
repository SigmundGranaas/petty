// FILE: /home/sigmund/RustroverProjects/petty/src/xpath/parser.rs
//! A `nom`-based parser for the expression language.
use super::ast::{Expression, Selection};
use crate::parser::ParseError;
use nom::{
    branch::alt,
    bytes::complete::{is_not, tag, take_while},
    character::complete::{alpha1, char, multispace0},
    combinator::{map, recognize},
    multi::{separated_list0, separated_list1}, // Import separated_list1
    number::complete::double,
    sequence::{delimited, pair, preceded},
    IResult,
};
use serde_json::{json, Value};

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

// --- Combinators ---

fn expression(input: &str) -> IResult<&str, Expression> {
    ws(alt((
        map(literal, Expression::Literal),
        function_call, // Must be before selection to parse `func()` not `func`
        map(selection, Expression::Selection),
    )))(input)
}

// --- Literal Parsers ---

fn boolean(input: &str) -> IResult<&str, Value> {
    alt((map(tag("true"), |_| json!(true)), map(tag("false"), |_| json!(false))))(input)
}

fn null(input: &str) -> IResult<&str, Value> {
    map(tag("null"), |_| json!(null))(input)
}

fn string_literal(input: &str) -> IResult<&str, Value> {
    map(delimited(char('\''), is_not("'"), char('\'')), |s: &str| json!(s))(input)
}

fn number(input: &str) -> IResult<&str, Value> {
    map(double, Value::from)(input)
}

fn literal(input: &str) -> IResult<&str, Value> {
    alt((null, boolean, number, string_literal))(input)
}

// --- Path/Selection Parser ---

// The pattern is `[a-zA-Z_][a-zA-Z0-9_]*`.
fn identifier(input: &str) -> IResult<&str, &str> {
    recognize(pair(
        alt((alpha1, tag("_"))),
        take_while(|c: char| c.is_alphanumeric() || c == '_'),
    ))(input)
}

fn selection(input: &str) -> IResult<&str, Selection> {
    alt((
        map(tag("."), |_| Selection::CurrentContext),
        map(preceded(char('$'), identifier), |name| Selection::Variable(name.to_string())),
        // ======================= START: THIS IS THE FIX =======================
        // A path must have at least one part. Using separated_list1 makes the
        // parser less ambiguous and more robust. It correctly handles both
        // 'name' and 'customer.name' while failing on invalid path inputs,
        // which separated_list0 did not.
        map(separated_list1(char('.'), identifier), |parts| {
            let path = format!("/{}", parts.join("/"));
            Selection::JsonPointer(path)
        }),
        // ======================== END: THIS IS THE FIX ========================
    ))(input)
}

// --- Function Call Parser ---

fn function_call(input: &str) -> IResult<&str, Expression> {
    let (input, name) = identifier(input)?;
    let (input, _) = multispace0(input)?;
    let (input, args) =
        delimited(char('('), separated_list0(ws(char(',')), expression), char(')'))(input)?;

    Ok((input, Expression::FunctionCall { name: name.to_string(), args }))
}

/// A combinator that takes a parser `inner` and produces a parser that consumes surrounding whitespace.
fn ws<'a, F, O>(inner: F) -> impl FnMut(&'a str) -> IResult<&'a str, O>
where
    F: FnMut(&'a str) -> IResult<&'a str, O>,
{
    delimited(multispace0, inner, multispace0)
}