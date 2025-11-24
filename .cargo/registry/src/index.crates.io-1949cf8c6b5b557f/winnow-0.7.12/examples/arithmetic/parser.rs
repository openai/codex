use std::str::FromStr;

use winnow::prelude::*;
use winnow::Result;
use winnow::{
    ascii::{digit1 as digits, multispace0 as multispaces},
    combinator::alt,
    combinator::delimited,
    combinator::repeat,
    token::one_of,
};

// Parser definition

pub(crate) fn expr(i: &mut &str) -> Result<i64> {
    let init = term.parse_next(i)?;

    repeat(0.., (one_of(['+', '-']), term))
        .fold(
            move || init,
            |acc, (op, val): (char, i64)| {
                if op == '+' {
                    acc + val
                } else {
                    acc - val
                }
            },
        )
        .parse_next(i)
}

// We read an initial factor and for each time we find
// a * or / operator followed by another factor, we do
// the math by folding everything
pub(crate) fn term(i: &mut &str) -> Result<i64> {
    let init = factor.parse_next(i)?;

    repeat(0.., (one_of(['*', '/']), factor))
        .fold(
            move || init,
            |acc, (op, val): (char, i64)| {
                if op == '*' {
                    acc * val
                } else {
                    acc / val
                }
            },
        )
        .parse_next(i)
}

// We transform an integer string into a i64, ignoring surrounding whitespace
// We look for a digit suite, and try to convert it.
// If either str::from_utf8 or FromStr::from_str fail,
// we fallback to the parens parser defined above
pub(crate) fn factor(i: &mut &str) -> Result<i64> {
    delimited(
        multispaces,
        alt((digits.try_map(FromStr::from_str), parens)),
        multispaces,
    )
    .parse_next(i)
}

// We parse any expr surrounded by parens, ignoring all whitespace around those
fn parens(i: &mut &str) -> Result<i64> {
    delimited('(', expr, ')').parse_next(i)
}
