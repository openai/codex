use proc_macro2::{Ident, Span};
use syn::{spanned::Spanned, LitByteStr, LitStr};

use crate::error::{Errors, Result};
use crate::leaf::Callback;
use crate::mir::Mir;
use crate::parser::nested::NestedValue;
use crate::parser::{IgnoreFlags, Parser, Subpatterns};

use super::ignore_flags::ascii_case::MakeAsciiCaseInsensitive;

pub struct Definition {
    pub literal: Literal,
    pub priority: Option<usize>,
    pub callback: Option<Callback>,
    pub ignore_flags: IgnoreFlags,
}

pub enum Literal {
    Utf8(LitStr),
    Bytes(LitByteStr),
}

impl Definition {
    pub fn new(literal: Literal) -> Self {
        Definition {
            literal,
            priority: None,
            callback: None,
            ignore_flags: IgnoreFlags::Empty,
        }
    }

    pub fn named_attr(&mut self, name: Ident, value: NestedValue, parser: &mut Parser) {
        match (name.to_string().as_str(), value) {
            ("priority", NestedValue::Assign(tokens)) => {
                let prio = match tokens.to_string().parse() {
                    Ok(prio) => prio,
                    Err(_) => {
                        parser.err("Expected an unsigned integer", tokens.span());
                        return;
                    }
                };

                if self.priority.replace(prio).is_some() {
                    parser.err("Resetting previously set priority", tokens.span());
                }
            }
            ("priority", _) => {
                parser.err("Expected: priority = <integer>", name.span());
            }
            ("callback", NestedValue::Assign(tokens)) => {
                let span = tokens.span();
                let callback = match parser.parse_callback(tokens) {
                    Some(callback) => callback,
                    None => {
                        parser.err("Not a valid callback", span);
                        return;
                    }
                };

                if let Some(previous) = self.callback.replace(callback) {
                    parser
                        .err(
                            "Callback has been already set",
                            span.join(name.span()).unwrap(),
                        )
                        .err("Previous callback set here", previous.span());
                }
            }
            ("callback", _) => {
                parser.err("Expected: callback = ...", name.span());
            }
            ("ignore", NestedValue::Group(tokens)) => {
                self.ignore_flags.parse_group(name, tokens, parser);
            }
            ("ignore", _) => {
                parser.err("Expected: ignore(<flag>, ...)", name.span());
            }
            (unknown, _) => {
                parser.err(
                    format!(
                        "\
                        Unknown nested attribute: {}\n\
                        \n\
                        Expected one of: priority, callback\
                        ",
                        unknown
                    ),
                    name.span(),
                );
            }
        }
    }
}

impl Literal {
    pub fn to_bytes(&self) -> Vec<u8> {
        match self {
            Literal::Utf8(string) => string.value().into_bytes(),
            Literal::Bytes(bytes) => bytes.value(),
        }
    }

    pub fn escape_regex(&self) -> Literal {
        match self {
            Literal::Utf8(string) => Literal::Utf8(LitStr::new(
                regex_syntax::escape(&string.value()).as_str(),
                self.span(),
            )),
            Literal::Bytes(bytes) => Literal::Bytes(LitByteStr::new(
                regex_syntax::escape(&bytes_to_regex_string(bytes.value())).as_bytes(),
                self.span(),
            )),
        }
    }

    pub fn to_mir(
        &self,
        subpatterns: &Subpatterns,
        ignore_flags: IgnoreFlags,
        errors: &mut Errors,
    ) -> Result<Mir> {
        let value = subpatterns.fix(self, errors);

        if ignore_flags.contains(IgnoreFlags::IgnoreAsciiCase) {
            match self {
                Literal::Utf8(_) => {
                    Mir::utf8(&value).map(MakeAsciiCaseInsensitive::make_ascii_case_insensitive)
                }
                Literal::Bytes(_) => Mir::binary_ignore_case(&value),
            }
        } else if ignore_flags.contains(IgnoreFlags::IgnoreCase) {
            match self {
                Literal::Utf8(_) => Mir::utf8_ignore_case(&value),
                Literal::Bytes(_) => Mir::binary_ignore_case(&value),
            }
        } else {
            match self {
                Literal::Utf8(_) => Mir::utf8(&value),
                Literal::Bytes(_) => Mir::binary(&value),
            }
        }
    }

    pub fn span(&self) -> Span {
        match self {
            Literal::Utf8(string) => string.span(),
            Literal::Bytes(bytes) => bytes.span(),
        }
    }
}

impl syn::parse::Parse for Literal {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let la = input.lookahead1();
        if la.peek(LitStr) {
            Ok(Literal::Utf8(input.parse()?))
        } else if la.peek(LitByteStr) {
            Ok(Literal::Bytes(input.parse()?))
        } else {
            Err(la.error())
        }
    }
}

pub fn bytes_to_regex_string(bytes: Vec<u8>) -> String {
    if bytes.is_ascii() {
        unsafe {
            // Unicode values are prohibited, so we can't use
            // safe version of String::from_utf8
            //
            // We can, however, construct a safe ASCII string
            return String::from_utf8_unchecked(bytes);
        }
    }

    let mut string = String::with_capacity(bytes.len() * 2);

    for byte in bytes {
        if byte < 0x80 {
            string.push(byte as char);
        } else {
            static DIGITS: [u8; 16] = *b"0123456789abcdef";

            string.push_str(r"\x");
            string.push(DIGITS[(byte / 16) as usize] as char);
            string.push(DIGITS[(byte % 16) as usize] as char);
        }
    }

    string
}
