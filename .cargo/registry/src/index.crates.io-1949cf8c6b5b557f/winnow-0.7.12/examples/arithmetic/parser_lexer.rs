use std::fmt;
use std::fmt::{Debug, Display, Formatter};

use winnow::prelude::*;
use winnow::Result;
use winnow::{
    ascii::{digit1 as digits, multispace0 as multispaces},
    combinator::alt,
    combinator::dispatch,
    combinator::eof,
    combinator::fail,
    combinator::opt,
    combinator::peek,
    combinator::repeat,
    combinator::{delimited, preceded, terminated},
    error::ContextError,
    stream::TokenSlice,
    token::any,
    token::literal,
    token::one_of,
    token::take_till,
};

/// Lex and parse
#[allow(dead_code)]
pub(crate) fn expr2(i: &mut &str) -> Result<Expr> {
    let tokens = tokens.parse_next(i)?;
    let mut tokens = Tokens::new(&tokens);
    expr.parse_next(&mut tokens)
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct Token<'s> {
    kind: TokenKind,
    raw: &'s str,
}

impl fmt::Debug for Token<'_> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Customized for brevity for a better `winnow/debug` experience
        match self.kind {
            TokenKind::Value => Debug::fmt(self.raw, fmt),
            TokenKind::Oper(oper) => Debug::fmt(&oper, fmt),
            TokenKind::OpenParen => fmt.write_str("OpenParen"),
            TokenKind::CloseParen => fmt.write_str("CloseParen"),
            TokenKind::Unknown => fmt.write_str("Unknown"),
            TokenKind::Eof => fmt.write_str("Eof"),
        }
    }
}

impl PartialEq<TokenKind> for Token<'_> {
    fn eq(&self, other: &TokenKind) -> bool {
        self.kind == *other
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum TokenKind {
    Value,
    Oper(Oper),
    OpenParen,
    CloseParen,
    Unknown,
    Eof,
}

impl<'i> Parser<Tokens<'i>, &'i Token<'i>, ContextError> for TokenKind {
    fn parse_next(&mut self, input: &mut Tokens<'i>) -> Result<&'i Token<'i>> {
        literal(*self).parse_next(input).map(|t| &t[0])
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum Oper {
    Add,
    Sub,
    Mul,
    Div,
}

impl winnow::stream::ContainsToken<&'_ Token<'_>> for TokenKind {
    #[inline(always)]
    fn contains_token(&self, token: &'_ Token<'_>) -> bool {
        *self == token.kind
    }
}

impl winnow::stream::ContainsToken<&'_ Token<'_>> for &'_ [TokenKind] {
    #[inline]
    fn contains_token(&self, token: &'_ Token<'_>) -> bool {
        self.iter().any(|t| *t == token.kind)
    }
}

impl<const LEN: usize> winnow::stream::ContainsToken<&'_ Token<'_>> for &'_ [TokenKind; LEN] {
    #[inline]
    fn contains_token(&self, token: &'_ Token<'_>) -> bool {
        self.iter().any(|t| *t == token.kind)
    }
}

impl<const LEN: usize> winnow::stream::ContainsToken<&'_ Token<'_>> for [TokenKind; LEN] {
    #[inline]
    fn contains_token(&self, token: &'_ Token<'_>) -> bool {
        self.iter().any(|t| *t == token.kind)
    }
}

/// Lex tokens
///
/// See [`expr`] to parse the tokens
pub(crate) fn tokens<'s>(i: &mut &'s str) -> Result<Vec<Token<'s>>> {
    let mut tokens: Vec<_> =
        preceded(multispaces, repeat(1.., terminated(token, multispaces))).parse_next(i)?;
    if let Some(eof) = opt(eof.map(|raw| Token {
        kind: TokenKind::Eof,
        raw,
    }))
    .parse_next(i)?
    {
        tokens.push(eof);
    }
    Ok(tokens)
}

fn token<'s>(i: &mut &'s str) -> Result<Token<'s>> {
    dispatch! {peek(any);
        '0'..='9' => digits.value(TokenKind::Value),
        '(' => '('.value(TokenKind::OpenParen),
        ')' => ')'.value(TokenKind::CloseParen),
        '+' => '+'.value(TokenKind::Oper(Oper::Add)),
        '-' => '-'.value(TokenKind::Oper(Oper::Sub)),
        '*' => '*'.value(TokenKind::Oper(Oper::Mul)),
        '/' => '/'.value(TokenKind::Oper(Oper::Div)),
        ' '| '\t'| '\r'| '\n' => fail,
        _ => take_till(.., ('0'..='9', '(', ')', '+', '-', '*', '/')).value(TokenKind::Unknown),
    }
    .with_taken()
    .map(|(kind, raw)| Token { kind, raw })
    .parse_next(i)
}

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

pub(crate) type Tokens<'i> = TokenSlice<'i, Token<'i>>;

/// Parse the tokens lexed in [`tokens`]
pub(crate) fn expr(i: &mut Tokens<'_>) -> Result<Expr> {
    let init = term.parse_next(i)?;

    let expr = repeat(
        0..,
        (
            one_of([TokenKind::Oper(Oper::Add), TokenKind::Oper(Oper::Sub)]),
            term,
        ),
    )
    .fold(
        move || init.clone(),
        |acc, (op, val): (&Token<'_>, Expr)| {
            if op.kind == TokenKind::Oper(Oper::Add) {
                Expr::Add(Box::new(acc), Box::new(val))
            } else {
                Expr::Sub(Box::new(acc), Box::new(val))
            }
        },
    )
    .parse_next(i)?;

    opt(TokenKind::Eof).parse_next(i)?;

    Ok(expr)
}

pub(crate) fn term(i: &mut Tokens<'_>) -> Result<Expr> {
    let init = factor.parse_next(i)?;

    repeat(
        0..,
        (
            one_of([TokenKind::Oper(Oper::Mul), TokenKind::Oper(Oper::Div)]),
            factor,
        ),
    )
    .fold(
        move || init.clone(),
        |acc, (op, val): (&Token<'_>, Expr)| {
            if op.kind == TokenKind::Oper(Oper::Mul) {
                Expr::Mul(Box::new(acc), Box::new(val))
            } else {
                Expr::Div(Box::new(acc), Box::new(val))
            }
        },
    )
    .parse_next(i)
}

pub(crate) fn factor(i: &mut Tokens<'_>) -> Result<Expr> {
    alt((
        TokenKind::Value.try_map(|t: &Token<'_>| t.raw.parse::<i64>().map(Expr::Value)),
        parens,
    ))
    .parse_next(i)
}

fn parens(i: &mut Tokens<'_>) -> Result<Expr> {
    delimited(TokenKind::OpenParen, expr, TokenKind::CloseParen)
        .map(|e| Expr::Paren(Box::new(e)))
        .parse_next(i)
}
