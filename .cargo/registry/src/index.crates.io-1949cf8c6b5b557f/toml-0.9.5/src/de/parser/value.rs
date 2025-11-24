use serde_spanned::Spanned;

use crate::alloc_prelude::*;
use crate::de::parser::array::on_array;
use crate::de::parser::inline_table::on_inline_table;
use crate::de::parser::prelude::*;
use crate::de::DeFloat;
use crate::de::DeInteger;
use crate::de::DeValue;

/// ```bnf
/// val = string / boolean / array / inline-table / date-time / float / integer
/// ```
pub(crate) fn value<'i>(
    input: &mut Input<'_>,
    source: toml_parser::Source<'i>,
    errors: &mut dyn ErrorSink,
) -> Spanned<DeValue<'i>> {
    #[cfg(feature = "debug")]
    let _scope = TraceScope::new("value");
    if let Some(event) = input.next_token() {
        match event.kind() {
            EventKind::StdTableOpen
            | EventKind::ArrayTableOpen
            | EventKind::InlineTableClose
            | EventKind::ArrayClose
            | EventKind::ValueSep
            | EventKind::Comment
            | EventKind::Newline
            | EventKind::Error
            | EventKind::SimpleKey
            | EventKind::KeySep
            | EventKind::KeyValSep
            | EventKind::StdTableClose
            | EventKind::ArrayTableClose => {
                #[cfg(feature = "debug")]
                trace(
                    &format!("unexpected {event:?}"),
                    anstyle::AnsiColor::Red.on_default(),
                );
            }
            EventKind::Whitespace => {
                #[cfg(feature = "debug")]
                trace(
                    &format!("unexpected {event:?}"),
                    anstyle::AnsiColor::Red.on_default(),
                );
            }
            EventKind::InlineTableOpen => {
                return on_inline_table(event, input, source, errors);
            }
            EventKind::ArrayOpen => {
                return on_array(event, input, source, errors);
            }
            EventKind::Scalar => {
                return on_scalar(event, source, errors);
            }
        }
    }

    Spanned::new(0..0, DeValue::Integer(Default::default()))
}

pub(crate) fn on_scalar<'i>(
    event: &toml_parser::parser::Event,
    source: toml_parser::Source<'i>,
    errors: &mut dyn ErrorSink,
) -> Spanned<DeValue<'i>> {
    #[cfg(feature = "debug")]
    let _scope = TraceScope::new("on_scalar");
    let value_span = event.span();
    let value_span = value_span.start()..value_span.end();

    let raw = source.get(event).unwrap();
    let mut decoded = alloc::borrow::Cow::Borrowed("");
    let kind = raw.decode_scalar(&mut decoded, errors);
    match kind {
        toml_parser::decoder::ScalarKind::String => {
            Spanned::new(value_span, DeValue::String(decoded))
        }
        toml_parser::decoder::ScalarKind::Boolean(value) => {
            Spanned::new(value_span, DeValue::Boolean(value))
        }
        toml_parser::decoder::ScalarKind::DateTime => {
            let value = match decoded.parse::<toml_datetime::Datetime>() {
                Ok(value) => value,
                Err(err) => {
                    errors.report_error(
                        ParseError::new(err.to_string()).with_unexpected(event.span()),
                    );
                    toml_datetime::Datetime {
                        date: None,
                        time: None,
                        offset: None,
                    }
                }
            };
            Spanned::new(value_span, DeValue::Datetime(value))
        }
        toml_parser::decoder::ScalarKind::Float => {
            Spanned::new(value_span, DeValue::Float(DeFloat { inner: decoded }))
        }
        toml_parser::decoder::ScalarKind::Integer(radix) => Spanned::new(
            value_span,
            DeValue::Integer(DeInteger {
                inner: decoded,
                radix: radix.value(),
            }),
        ),
    }
}
