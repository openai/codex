use std::fmt;
use std::fmt::{Debug, Display, Formatter};

use std::str::FromStr;

use nom::{
  branch::alt,
  bytes::complete::tag,
  character::complete::{digit1 as digit, multispace0 as multispace},
  combinator::{map, map_res},
  multi::many0,
  sequence::{delimited, preceded},
  IResult,
};

pub enum Expr {
  Value(i64),
  Add(Box<Expr>, Box<Expr>),
  Sub(Box<Expr>, Box<Expr>),
  Mul(Box<Expr>, Box<Expr>),
  Div(Box<Expr>, Box<Expr>),
  Paren(Box<Expr>),
}

#[derive(Debug)]
pub enum Oper {
  Add,
  Sub,
  Mul,
  Div,
}

impl Display for Expr {
  fn fmt(&self, format: &mut Formatter<'_>) -> fmt::Result {
    use self::Expr::*;
    match *self {
      Value(val) => write!(format, "{}", val),
      Add(ref left, ref right) => write!(format, "{} + {}", left, right),
      Sub(ref left, ref right) => write!(format, "{} - {}", left, right),
      Mul(ref left, ref right) => write!(format, "{} * {}", left, right),
      Div(ref left, ref right) => write!(format, "{} / {}", left, right),
      Paren(ref expr) => write!(format, "({})", expr),
    }
  }
}

impl Debug for Expr {
  fn fmt(&self, format: &mut Formatter<'_>) -> fmt::Result {
    use self::Expr::*;
    match *self {
      Value(val) => write!(format, "{}", val),
      Add(ref left, ref right) => write!(format, "({:?} + {:?})", left, right),
      Sub(ref left, ref right) => write!(format, "({:?} - {:?})", left, right),
      Mul(ref left, ref right) => write!(format, "({:?} * {:?})", left, right),
      Div(ref left, ref right) => write!(format, "({:?} / {:?})", left, right),
      Paren(ref expr) => write!(format, "[{:?}]", expr),
    }
  }
}

fn parens(i: &str) -> IResult<&str, Expr> {
  delimited(
    multispace,
    delimited(tag("("), map(expr, |e| Expr::Paren(Box::new(e))), tag(")")),
    multispace,
  )(i)
}

fn factor(i: &str) -> IResult<&str, Expr> {
  alt((
    map(
      map_res(delimited(multispace, digit, multispace), FromStr::from_str),
      Expr::Value,
    ),
    parens,
  ))(i)
}

fn fold_exprs(initial: Expr, remainder: Vec<(Oper, Expr)>) -> Expr {
  remainder.into_iter().fold(initial, |acc, pair| {
    let (oper, expr) = pair;
    match oper {
      Oper::Add => Expr::Add(Box::new(acc), Box::new(expr)),
      Oper::Sub => Expr::Sub(Box::new(acc), Box::new(expr)),
      Oper::Mul => Expr::Mul(Box::new(acc), Box::new(expr)),
      Oper::Div => Expr::Div(Box::new(acc), Box::new(expr)),
    }
  })
}

fn term(i: &str) -> IResult<&str, Expr> {
  let (i, initial) = factor(i)?;
  let (i, remainder) = many0(alt((
    |i| {
      let (i, mul) = preceded(tag("*"), factor)(i)?;
      Ok((i, (Oper::Mul, mul)))
    },
    |i| {
      let (i, div) = preceded(tag("/"), factor)(i)?;
      Ok((i, (Oper::Div, div)))
    },
  )))(i)?;

  Ok((i, fold_exprs(initial, remainder)))
}

fn expr(i: &str) -> IResult<&str, Expr> {
  let (i, initial) = term(i)?;
  let (i, remainder) = many0(alt((
    |i| {
      let (i, add) = preceded(tag("+"), term)(i)?;
      Ok((i, (Oper::Add, add)))
    },
    |i| {
      let (i, sub) = preceded(tag("-"), term)(i)?;
      Ok((i, (Oper::Sub, sub)))
    },
  )))(i)?;

  Ok((i, fold_exprs(initial, remainder)))
}

#[test]
fn factor_test() {
  assert_eq!(
    factor("  3  ").map(|(i, x)| (i, format!("{:?}", x))),
    Ok(("", String::from("3")))
  );
}

#[test]
fn term_test() {
  assert_eq!(
    term(" 3 *  5   ").map(|(i, x)| (i, format!("{:?}", x))),
    Ok(("", String::from("(3 * 5)")))
  );
}

#[test]
fn expr_test() {
  assert_eq!(
    expr(" 1 + 2 *  3 ").map(|(i, x)| (i, format!("{:?}", x))),
    Ok(("", String::from("(1 + (2 * 3))")))
  );
  assert_eq!(
    expr(" 1 + 2 *  3 / 4 - 5 ").map(|(i, x)| (i, format!("{:?}", x))),
    Ok(("", String::from("((1 + ((2 * 3) / 4)) - 5)")))
  );
  assert_eq!(
    expr(" 72 / 2 / 3 ").map(|(i, x)| (i, format!("{:?}", x))),
    Ok(("", String::from("((72 / 2) / 3)")))
  );
}

#[test]
fn parens_test() {
  assert_eq!(
    expr(" ( 1 + 2 ) *  3 ").map(|(i, x)| (i, format!("{:?}", x))),
    Ok(("", String::from("([(1 + 2)] * 3)")))
  );
}
