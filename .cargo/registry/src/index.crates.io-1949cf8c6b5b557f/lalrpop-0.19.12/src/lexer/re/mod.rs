//! A parser and representation of regular expressions.

use regex_syntax::hir::Hir;
use regex_syntax::{self, Error, Parser};

#[cfg(test)]
mod test;

pub type Regex = Hir;
pub type RegexError = Error;

/// Convert a string literal into a parsed regular expression.
pub fn parse_literal(s: &str) -> Regex {
    match parse_regex(&regex_syntax::escape(s)) {
        Ok(v) => v,
        Err(_) => panic!("failed to parse literal regular expression"),
    }
}

/// Parse a regular expression like `a+` etc.
pub fn parse_regex(s: &str) -> Result<Regex, RegexError> {
    let expr = Parser::new().parse(s)?;
    Ok(expr)
}
