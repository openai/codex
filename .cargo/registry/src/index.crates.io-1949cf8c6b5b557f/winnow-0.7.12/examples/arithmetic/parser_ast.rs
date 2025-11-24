use std::fmt;
use std::fmt::{Debug, Display, Formatter};

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

#[derive(Debug, Clone)]
pub(crate) enum Expr {
    Value(i64),
    Add(Box<Expr>, Box<Expr>),
    Sub(Box<Expr>, Box<Expr>),
    Mul(Box<Expr>, Box<Expr>),
    Div(Box<Expr>, Box<Expr>),
    Paren(Box<Expr>),
}

impl Expr {
    pub(crate) fn eval(&self) -> i64 {
        match self {
            Self::Value(v) => *v,
            Self::Add(lhs, rhs) => lhs.eval() + rhs.eval(),
            Self::Sub(lhs, rhs) => lhs.eval() - rhs.eval(),
            Self::Mul(lhs, rhs) => lhs.eval() * rhs.eval(),
            Self::Div(lhs, rhs) => lhs.eval() / rhs.eval(),
            Self::Paren(expr) => expr.eval(),
        }
    }
}

impl Display for Expr {
    fn fmt(&self, format: &mut Formatter<'_>) -> fmt::Result {
        use Expr::{Add, Div, Mul, Paren, Sub, Value};
        match *self {
            Value(val) => write!(format, "{val}"),
            Add(ref left, ref right) => write!(format, "{left} + {right}"),
            Sub(ref left, ref right) => write!(format, "{left} - {right}"),
            Mul(ref left, ref right) => write!(format, "{left} * {right}"),
            Div(ref left, ref right) => write!(format, "{left} / {right}"),
            Paren(ref expr) => write!(format, "({expr})"),
        }
    }
}

pub(crate) fn expr(i: &mut &str) -> Result<Expr> {
    let init = term.parse_next(i)?;

    repeat(0.., (one_of(['+', '-']), term))
        .fold(
            move || init.clone(),
            |acc, (op, val): (char, Expr)| {
                if op == '+' {
                    Expr::Add(Box::new(acc), Box::new(val))
                } else {
                    Expr::Sub(Box::new(acc), Box::new(val))
                }
            },
        )
        .parse_next(i)
}

pub(crate) fn term(i: &mut &str) -> Result<Expr> {
    let init = factor.parse_next(i)?;

    repeat(0.., (one_of(['*', '/']), factor))
        .fold(
            move || init.clone(),
            |acc, (op, val): (char, Expr)| {
                if op == '*' {
                    Expr::Mul(Box::new(acc), Box::new(val))
                } else {
                    Expr::Div(Box::new(acc), Box::new(val))
                }
            },
        )
        .parse_next(i)
}

pub(crate) fn factor(i: &mut &str) -> Result<Expr> {
    delimited(
        multispaces,
        alt((digits.try_map(FromStr::from_str).map(Expr::Value), parens)),
        multispaces,
    )
    .parse_next(i)
}

fn parens(i: &mut &str) -> Result<Expr> {
    delimited("(", expr, ")")
        .map(|e| Expr::Paren(Box::new(e)))
        .parse_next(i)
}
