#[cfg(feature = "parsing")]
use crate::buffer::Cursor;
#[cfg(feature = "parsing")]
use crate::lookahead;
#[cfg(feature = "parsing")]
use crate::parse::{Parse, ParseStream, Result};
#[cfg(feature = "parsing")]
use crate::token::Token;

pub use proc_macro2::Ident;

#[cfg(feature = "parsing")]
#[doc(hidden)]
#[allow(non_snake_case)]
pub fn Ident(marker: lookahead::TokenMarker) -> Ident {
    match marker {}
}

#[cfg(feature = "parsing")]
fn accept_as_ident(ident: &Ident) -> bool {
    match ident.to_string().as_str() {
        "_" |
        // Based on https://doc.rust-lang.org/grammar.html#keywords
        // and https://github.com/rust-lang/rfcs/blob/master/text/2421-unreservations-2018.md
        // and https://github.com/rust-lang/rfcs/blob/master/text/2420-unreserve-proc.md
        "abstract" | "as" | "become" | "box" | "break" | "const" | "continue" |
        "crate" | "do" | "else" | "enum" | "extern" | "false" | "final" | "fn" |
        "for" | "if" | "impl" | "in" | "let" | "loop" | "macro" | "match" |
        "mod" | "move" | "mut" | "override" | "priv" | "pub" | "ref" |
        "return" | "Self" | "self" | "static" | "struct" | "super" | "trait" |
        "true" | "type" | "typeof" | "unsafe" | "unsized" | "use" | "virtual" |
        "where" | "while" | "yield" => false,
        _ => true,
    }
}

#[cfg(feature = "parsing")]
#[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
impl Parse for Ident {
    fn parse(input: ParseStream) -> Result<Self> {
        input.step(|cursor| {
            if let Some((ident, rest)) = cursor.ident() {
                if accept_as_ident(&ident) {
                    return Ok((ident, rest));
                }
            }
            Err(cursor.error("expected identifier"))
        })
    }
}

#[cfg(feature = "parsing")]
impl Token for Ident {
    fn peek(cursor: Cursor) -> bool {
        if let Some((ident, _rest)) = cursor.ident() {
            accept_as_ident(&ident)
        } else {
            false
        }
    }

    fn display() -> &'static str {
        "identifier"
    }
}

macro_rules! ident_from_token {
    ($token:ident) => {
        impl From<Token![$token]> for Ident {
            fn from(token: Token![$token]) -> Ident {
                Ident::new(stringify!($token), token.span)
            }
        }
    };
}

ident_from_token!(self);
ident_from_token!(Self);
ident_from_token!(super);
ident_from_token!(crate);
ident_from_token!(extern);

impl From<Token![_]> for Ident {
    fn from(token: Token![_]) -> Ident {
        Ident::new("_", token.span)
    }
}

pub fn xid_ok(symbol: &str) -> bool {
    let mut chars = symbol.chars();
    let first = chars.next().unwrap();
    if !(first == '_' || unicode_ident::is_xid_start(first)) {
        return false;
    }
    for ch in chars {
        if !unicode_ident::is_xid_continue(ch) {
            return false;
        }
    }
    true
}
