use crate::bound::{self, Bound};
use crate::date::{self, Date};
use crate::error::{Error, Result};
use crate::iter::{self, Iter};
use crate::release::{self, Release};
use crate::token;
use crate::version::{Channel, Version};
use proc_macro::{Ident, Span, TokenTree};

pub enum Expr {
    Stable,
    Beta,
    Nightly,
    Date(Date),
    Since(Bound),
    Before(Bound),
    Release(Release),
    Not(Box<Expr>),
    Any(Vec<Expr>),
    All(Vec<Expr>),
}

impl Expr {
    pub fn eval(&self, rustc: Version) -> bool {
        use self::Expr::*;

        match self {
            Stable => rustc.channel == Channel::Stable,
            Beta => rustc.channel == Channel::Beta,
            Nightly => match rustc.channel {
                Channel::Nightly(_) | Channel::Dev => true,
                Channel::Stable | Channel::Beta => false,
            },
            Date(date) => match rustc.channel {
                Channel::Nightly(rustc) => rustc == *date,
                Channel::Stable | Channel::Beta | Channel::Dev => false,
            },
            Since(bound) => rustc >= *bound,
            Before(bound) => rustc < *bound,
            Release(release) => {
                rustc.channel == Channel::Stable
                    && rustc.minor == release.minor
                    && release.patch.map_or(true, |patch| rustc.patch == patch)
            }
            Not(expr) => !expr.eval(rustc),
            Any(exprs) => exprs.iter().any(|e| e.eval(rustc)),
            All(exprs) => exprs.iter().all(|e| e.eval(rustc)),
        }
    }
}

pub fn parse(iter: Iter) -> Result<Expr> {
    match &iter.next() {
        Some(TokenTree::Ident(i)) if i.to_string() == "stable" => parse_stable(iter),
        Some(TokenTree::Ident(i)) if i.to_string() == "beta" => Ok(Expr::Beta),
        Some(TokenTree::Ident(i)) if i.to_string() == "nightly" => parse_nightly(iter),
        Some(TokenTree::Ident(i)) if i.to_string() == "since" => parse_since(i, iter),
        Some(TokenTree::Ident(i)) if i.to_string() == "before" => parse_before(i, iter),
        Some(TokenTree::Ident(i)) if i.to_string() == "not" => parse_not(i, iter),
        Some(TokenTree::Ident(i)) if i.to_string() == "any" => parse_any(i, iter),
        Some(TokenTree::Ident(i)) if i.to_string() == "all" => parse_all(i, iter),
        unexpected => {
            let span = unexpected
                .as_ref()
                .map_or_else(Span::call_site, TokenTree::span);
            Err(Error::new(span, "expected one of `stable`, `beta`, `nightly`, `since`, `before`, `not`, `any`, `all`"))
        }
    }
}

fn parse_nightly(iter: Iter) -> Result<Expr> {
    let paren = match token::parse_optional_paren(iter) {
        Some(group) => group,
        None => return Ok(Expr::Nightly),
    };

    let ref mut inner = iter::new(paren.stream());
    let date = date::parse(paren, inner)?;
    token::parse_optional_punct(inner, ',');
    token::parse_end(inner)?;

    Ok(Expr::Date(date))
}

fn parse_stable(iter: Iter) -> Result<Expr> {
    let paren = match token::parse_optional_paren(iter) {
        Some(group) => group,
        None => return Ok(Expr::Stable),
    };

    let ref mut inner = iter::new(paren.stream());
    let release = release::parse(paren, inner)?;
    token::parse_optional_punct(inner, ',');
    token::parse_end(inner)?;

    Ok(Expr::Release(release))
}

fn parse_since(introducer: &Ident, iter: Iter) -> Result<Expr> {
    let paren = token::parse_paren(introducer, iter)?;

    let ref mut inner = iter::new(paren.stream());
    let bound = bound::parse(paren, inner)?;
    token::parse_optional_punct(inner, ',');
    token::parse_end(inner)?;

    Ok(Expr::Since(bound))
}

fn parse_before(introducer: &Ident, iter: Iter) -> Result<Expr> {
    let paren = token::parse_paren(introducer, iter)?;

    let ref mut inner = iter::new(paren.stream());
    let bound = bound::parse(paren, inner)?;
    token::parse_optional_punct(inner, ',');
    token::parse_end(inner)?;

    Ok(Expr::Before(bound))
}

fn parse_not(introducer: &Ident, iter: Iter) -> Result<Expr> {
    let paren = token::parse_paren(introducer, iter)?;

    let ref mut inner = iter::new(paren.stream());
    let expr = self::parse(inner)?;
    token::parse_optional_punct(inner, ',');
    token::parse_end(inner)?;

    Ok(Expr::Not(Box::new(expr)))
}

fn parse_any(introducer: &Ident, iter: Iter) -> Result<Expr> {
    let paren = token::parse_paren(introducer, iter)?;

    let ref mut inner = iter::new(paren.stream());
    let exprs = parse_comma_separated(inner)?;

    Ok(Expr::Any(exprs.into_iter().collect()))
}

fn parse_all(introducer: &Ident, iter: Iter) -> Result<Expr> {
    let paren = token::parse_paren(introducer, iter)?;

    let ref mut inner = iter::new(paren.stream());
    let exprs = parse_comma_separated(inner)?;

    Ok(Expr::All(exprs.into_iter().collect()))
}

fn parse_comma_separated(iter: Iter) -> Result<Vec<Expr>> {
    let mut exprs = Vec::new();

    while iter.peek().is_some() {
        let expr = self::parse(iter)?;
        exprs.push(expr);
        if iter.peek().is_none() {
            break;
        }
        token::parse_punct(iter, ',')?;
    }

    Ok(exprs)
}
