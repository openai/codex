use crate::error::{Error, Result};
use crate::expr::{self, Expr};
use crate::{iter, token};
use proc_macro::{Span, TokenStream};

pub struct Args {
    pub condition: Expr,
    pub then: Then,
}

pub enum Then {
    Const(Span),
    Attribute(TokenStream),
}

pub fn parse(input: TokenStream) -> Result<Args> {
    let ref mut input = iter::new(input);
    let condition = expr::parse(input)?;

    token::parse_punct(input, ',')?;
    if input.peek().is_none() {
        return Err(Error::new(Span::call_site(), "expected one or more attrs"));
    }

    let const_span = token::parse_optional_keyword(input, "const");
    let then = if let Some(const_span) = const_span {
        token::parse_optional_punct(input, ',');
        token::parse_end(input)?;
        Then::Const(const_span)
    } else {
        Then::Attribute(input.collect())
    };

    Ok(Args { condition, then })
}
