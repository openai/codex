use crate::error::{Error, Result};
use crate::iter::Iter;
use proc_macro::{Delimiter, Group, Ident, Literal, Span, TokenTree};

pub fn parse_punct(iter: Iter, ch: char) -> Result<()> {
    match iter.next() {
        Some(TokenTree::Punct(ref punct)) if punct.as_char() == ch => Ok(()),
        unexpected => {
            let span = unexpected
                .as_ref()
                .map_or_else(Span::call_site, TokenTree::span);
            Err(Error::new(span, format!("expected `{}`", ch)))
        }
    }
}

pub fn parse_optional_punct(iter: Iter, ch: char) -> Option<()> {
    match iter.peek() {
        Some(TokenTree::Punct(punct)) if punct.as_char() == ch => iter.next().map(drop),
        _ => None,
    }
}

pub fn parse_optional_keyword(iter: Iter, keyword: &str) -> Option<Span> {
    match iter.peek() {
        Some(TokenTree::Ident(ident)) if ident.to_string() == keyword => {
            Some(iter.next().unwrap().span())
        }
        _ => None,
    }
}

pub fn parse_literal(iter: Iter) -> Result<Literal> {
    match iter.next() {
        Some(TokenTree::Literal(literal)) => Ok(literal),
        unexpected => {
            let span = unexpected
                .as_ref()
                .map_or_else(Span::call_site, TokenTree::span);
            Err(Error::new(span, "expected literal"))
        }
    }
}

pub fn parse_paren(introducer: &Ident, iter: Iter) -> Result<Group> {
    match iter.peek() {
        Some(TokenTree::Group(group)) if group.delimiter() == Delimiter::Parenthesis => {
            match iter.next() {
                Some(TokenTree::Group(group)) => Ok(group),
                _ => unreachable!(),
            }
        }
        Some(unexpected) => Err(Error::new(unexpected.span(), "expected `(`")),
        None => Err(Error::new(
            introducer.span(),
            format!("expected `(` after `{}`", introducer),
        )),
    }
}

pub fn parse_optional_paren(iter: Iter) -> Option<Group> {
    match iter.peek() {
        Some(TokenTree::Group(group)) if group.delimiter() == Delimiter::Parenthesis => {
            match iter.next() {
                Some(TokenTree::Group(group)) => Some(group),
                _ => unreachable!(),
            }
        }
        _ => None,
    }
}

pub fn parse_end(iter: Iter) -> Result<()> {
    match iter.next() {
        None => Ok(()),
        Some(unexpected) => Err(Error::new(unexpected.span(), "unexpected token")),
    }
}
