#![allow(clippy::type_complexity)]

use crate::RawString;
#[cfg(not(feature = "unbounded"))]
use toml_parser::parser::RecursionGuard;
use toml_parser::parser::ValidateWhitespace;
use winnow::stream::Stream as _;

pub(crate) mod array;
#[cfg(feature = "debug")]
pub(crate) mod debug;
pub(crate) mod document;
pub(crate) mod inline_table;
pub(crate) mod key;
pub(crate) mod value;

pub(crate) fn parse_document<'s>(
    source: toml_parser::Source<'s>,
    errors: &mut dyn prelude::ErrorSink,
) -> crate::Document<&'s str> {
    let tokens = source.lex().into_vec();

    let mut events = Vec::with_capacity(tokens.len());
    let mut receiver = ValidateWhitespace::new(&mut events, source);
    #[cfg(not(feature = "unbounded"))]
    let mut receiver = RecursionGuard::new(&mut receiver, LIMIT);
    #[cfg(not(feature = "unbounded"))]
    let receiver = &mut receiver;
    #[cfg(feature = "unbounded")]
    let receiver = &mut receiver;
    toml_parser::parser::parse_document(&tokens, receiver, errors);

    let mut input = prelude::Input::new(&events);
    let doc = document::document(&mut input, source, errors);
    doc
}

pub(crate) fn parse_key(
    source: toml_parser::Source<'_>,
    errors: &mut dyn prelude::ErrorSink,
) -> crate::Key {
    let tokens = source.lex().into_vec();

    let mut events = Vec::with_capacity(tokens.len());
    let mut receiver = ValidateWhitespace::new(&mut events, source);
    #[cfg(not(feature = "unbounded"))]
    let mut receiver = RecursionGuard::new(&mut receiver, LIMIT);
    #[cfg(not(feature = "unbounded"))]
    let receiver = &mut receiver;
    #[cfg(feature = "unbounded")]
    let receiver = &mut receiver;
    toml_parser::parser::parse_simple_key(&tokens, receiver, errors);

    if let Some(event) = events
        .iter()
        .find(|e| e.kind() == toml_parser::parser::EventKind::SimpleKey)
    {
        let (raw, key) = key::on_simple_key(event, source, errors);
        crate::Key::new(key).with_repr_unchecked(crate::Repr::new_unchecked(raw))
    } else {
        let key = source.input();
        let raw = RawString::with_span(0..source.input().len());
        crate::Key::new(key).with_repr_unchecked(crate::Repr::new_unchecked(raw))
    }
}

pub(crate) fn parse_key_path(
    source: toml_parser::Source<'_>,
    errors: &mut dyn prelude::ErrorSink,
) -> Vec<crate::Key> {
    let tokens = source.lex().into_vec();

    let mut events = Vec::with_capacity(tokens.len());
    let mut receiver = ValidateWhitespace::new(&mut events, source);
    #[cfg(not(feature = "unbounded"))]
    let mut receiver = RecursionGuard::new(&mut receiver, LIMIT);
    #[cfg(not(feature = "unbounded"))]
    let receiver = &mut receiver;
    #[cfg(feature = "unbounded")]
    let receiver = &mut receiver;
    toml_parser::parser::parse_key(&tokens, receiver, errors);

    let mut input = prelude::Input::new(&events);
    let mut prefix = None;
    let mut path = None;
    let mut key = None;
    let mut suffix = None;
    while let Some(event) = input.next_token() {
        match event.kind() {
            toml_parser::parser::EventKind::Whitespace => {
                let raw = RawString::with_span(event.span().start()..event.span().end());
                if prefix.is_none() {
                    prefix = Some(raw);
                } else if suffix.is_none() {
                    suffix = Some(raw);
                }
            }
            _ => {
                let (local_path, local_key) = key::on_key(event, &mut input, source, errors);
                path = Some(local_path);
                key = local_key;
            }
        }
    }
    if let Some(mut key) = key {
        if let Some(prefix) = prefix {
            key.leaf_decor.set_prefix(prefix);
        }
        if let Some(suffix) = suffix {
            key.leaf_decor.set_suffix(suffix);
        }
        let mut path = path.unwrap_or_default();
        path.push(key);
        path
    } else {
        Default::default()
    }
}

pub(crate) fn parse_value(
    source: toml_parser::Source<'_>,
    errors: &mut dyn prelude::ErrorSink,
) -> crate::Value {
    let tokens = source.lex().into_vec();

    let mut events = Vec::with_capacity(tokens.len());
    let mut receiver = ValidateWhitespace::new(&mut events, source);
    #[cfg(not(feature = "unbounded"))]
    let mut receiver = RecursionGuard::new(&mut receiver, LIMIT);
    #[cfg(not(feature = "unbounded"))]
    let receiver = &mut receiver;
    #[cfg(feature = "unbounded")]
    let receiver = &mut receiver;
    toml_parser::parser::parse_value(&tokens, receiver, errors);

    let mut input = prelude::Input::new(&events);
    let value = value::value(&mut input, source, errors);
    value
}

#[cfg(not(feature = "unbounded"))]
const LIMIT: u32 = 80;

pub(crate) mod prelude {
    pub(crate) use toml_parser::parser::EventKind;
    pub(crate) use toml_parser::ErrorSink;
    pub(crate) use toml_parser::ParseError;
    pub(crate) use winnow::stream::Stream as _;

    pub(crate) type Input<'i> = winnow::stream::TokenSlice<'i, toml_parser::parser::Event>;

    #[cfg(feature = "debug")]
    pub(crate) use super::debug::trace;
    #[cfg(feature = "debug")]
    pub(crate) use super::debug::TraceScope;
}
