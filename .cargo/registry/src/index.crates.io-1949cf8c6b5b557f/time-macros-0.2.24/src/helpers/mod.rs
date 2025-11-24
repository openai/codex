#[cfg(any(feature = "formatting", feature = "parsing"))]
mod string;

use std::iter::Peekable;
use std::str::FromStr;

use num_conv::prelude::*;
use proc_macro::{token_stream, Span, TokenTree};
use time_core::util::{days_in_year, is_leap_year};

use crate::Error;

#[cfg(any(feature = "formatting", feature = "parsing"))]
pub(crate) fn get_string_literal(
    mut tokens: impl Iterator<Item = TokenTree>,
) -> Result<(Span, Vec<u8>), Error> {
    match (tokens.next(), tokens.next()) {
        (Some(TokenTree::Literal(literal)), None) => string::parse(&literal),
        (Some(tree), None) => Err(Error::ExpectedString {
            span_start: Some(tree.span()),
            span_end: Some(tree.span()),
        }),
        (_, Some(tree)) => Err(Error::UnexpectedToken { tree }),
        (None, None) => Err(Error::ExpectedString {
            span_start: None,
            span_end: None,
        }),
    }
}

pub(crate) fn consume_number<T: FromStr>(
    component_name: &'static str,
    chars: &mut Peekable<token_stream::IntoIter>,
) -> Result<(Span, T), Error> {
    let (span, digits) = match chars.next() {
        Some(TokenTree::Literal(literal)) => (literal.span(), literal.to_string()),
        Some(tree) => return Err(Error::UnexpectedToken { tree }),
        None => return Err(Error::UnexpectedEndOfInput),
    };

    if let Ok(value) = digits.replace('_', "").parse() {
        Ok((span, value))
    } else {
        Err(Error::InvalidComponent {
            name: component_name,
            value: digits,
            span_start: Some(span),
            span_end: Some(span),
        })
    }
}

pub(crate) fn consume_any_ident(
    idents: &[&str],
    chars: &mut Peekable<token_stream::IntoIter>,
) -> Result<Span, Error> {
    match chars.peek() {
        Some(TokenTree::Ident(char)) if idents.contains(&char.to_string().as_str()) => {
            let ret = Ok(char.span());
            drop(chars.next());
            ret
        }
        Some(tree) => Err(Error::UnexpectedToken { tree: tree.clone() }),
        None => Err(Error::UnexpectedEndOfInput),
    }
}

pub(crate) fn consume_punct(
    c: char,
    chars: &mut Peekable<token_stream::IntoIter>,
) -> Result<Span, Error> {
    match chars.peek() {
        Some(TokenTree::Punct(punct)) if *punct == c => {
            let ret = Ok(punct.span());
            drop(chars.next());
            ret
        }
        Some(tree) => Err(Error::UnexpectedToken { tree: tree.clone() }),
        None => Err(Error::UnexpectedEndOfInput),
    }
}

fn jan_weekday(year: i32, ordinal: i32) -> u8 {
    macro_rules! div_floor {
        ($a:expr, $b:expr) => {{
            let (_quotient, _remainder) = ($a / $b, $a % $b);
            if (_remainder > 0 && $b < 0) || (_remainder < 0 && $b > 0) {
                _quotient - 1
            } else {
                _quotient
            }
        }};
    }

    let adj_year = year - 1;
    (ordinal + adj_year + div_floor!(adj_year, 4) - div_floor!(adj_year, 100)
        + div_floor!(adj_year, 400)
        + 6)
    .rem_euclid(7)
    .cast_unsigned()
    .truncate()
}

pub(crate) fn days_in_year_month(year: i32, month: u8) -> u8 {
    [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31][month.extend::<usize>() - 1]
        + u8::from(month == 2 && is_leap_year(year))
}

pub(crate) fn ywd_to_yo(year: i32, week: u8, iso_weekday_number: u8) -> (i32, u16) {
    let (ordinal, overflow) = (u16::from(week) * 7 + u16::from(iso_weekday_number))
        .overflowing_sub(u16::from(jan_weekday(year, 4)) + 4);

    if overflow || ordinal == 0 {
        return (year - 1, (ordinal.wrapping_add(days_in_year(year - 1))));
    }

    let days_in_cur_year = days_in_year(year);
    if ordinal > days_in_cur_year {
        (year + 1, ordinal - days_in_cur_year)
    } else {
        (year, ordinal)
    }
}

pub(crate) fn ymd_to_yo(year: i32, month: u8, day: u8) -> (i32, u16) {
    let ordinal = [0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334]
        [month.extend::<usize>() - 1]
        + u16::from(month > 2 && is_leap_year(year));

    (year, ordinal + u16::from(day))
}
