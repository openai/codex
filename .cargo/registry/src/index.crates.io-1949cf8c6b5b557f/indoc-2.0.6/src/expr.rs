use crate::error::{Error, Result};
use proc_macro::token_stream::IntoIter as TokenIter;
use proc_macro::{Spacing, Span, TokenStream, TokenTree};
use std::iter::{self, Peekable};

pub fn parse(input: &mut Peekable<TokenIter>, require_comma: bool) -> Result<TokenStream> {
    #[derive(PartialEq)]
    enum Lookbehind {
        JointColon,
        DoubleColon,
        JointHyphen,
        Other,
    }

    let mut expr = TokenStream::new();
    let mut lookbehind = Lookbehind::Other;
    let mut angle_bracket_depth = 0;

    loop {
        if angle_bracket_depth == 0 {
            match input.peek() {
                Some(TokenTree::Punct(punct)) if punct.as_char() == ',' => {
                    return Ok(expr);
                }
                _ => {}
            }
        }
        match input.next() {
            Some(TokenTree::Punct(punct)) => {
                let ch = punct.as_char();
                let spacing = punct.spacing();
                expr.extend(iter::once(TokenTree::Punct(punct)));
                lookbehind = match ch {
                    ':' if lookbehind == Lookbehind::JointColon => Lookbehind::DoubleColon,
                    ':' if spacing == Spacing::Joint => Lookbehind::JointColon,
                    '<' if lookbehind == Lookbehind::DoubleColon => {
                        angle_bracket_depth += 1;
                        Lookbehind::Other
                    }
                    '>' if angle_bracket_depth > 0 && lookbehind != Lookbehind::JointHyphen => {
                        angle_bracket_depth -= 1;
                        Lookbehind::Other
                    }
                    '-' if spacing == Spacing::Joint => Lookbehind::JointHyphen,
                    _ => Lookbehind::Other,
                };
            }
            Some(token) => expr.extend(iter::once(token)),
            None => {
                return if require_comma {
                    Err(Error::new(
                        Span::call_site(),
                        "unexpected end of macro input",
                    ))
                } else {
                    Ok(expr)
                };
            }
        }
    }
}
