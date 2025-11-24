use crate::parser::array::on_array;
use crate::parser::inline_table::on_inline_table;
use crate::parser::prelude::*;
use crate::repr::{Formatted, Repr};
use crate::RawString;
use crate::Value;

/// ```bnf
/// val = string / boolean / array / inline-table / date-time / float / integer
/// ```
pub(crate) fn value(
    input: &mut Input<'_>,
    source: toml_parser::Source<'_>,
    errors: &mut dyn ErrorSink,
) -> Value {
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

    Value::from(0)
}

pub(crate) fn on_scalar(
    event: &toml_parser::parser::Event,
    source: toml_parser::Source<'_>,
    errors: &mut dyn ErrorSink,
) -> Value {
    #[cfg(feature = "debug")]
    let _scope = TraceScope::new("on_scalar");
    let value_span = event.span();
    let value_raw = RawString::with_span(value_span.start()..value_span.end());

    let raw = source.get(event).unwrap();
    let mut decoded = std::borrow::Cow::Borrowed("");
    let kind = raw.decode_scalar(&mut decoded, errors);
    match kind {
        toml_parser::decoder::ScalarKind::String => {
            let mut f = Formatted::new(decoded.into());
            f.set_repr_unchecked(Repr::new_unchecked(value_raw));
            Value::String(f)
        }
        toml_parser::decoder::ScalarKind::Boolean(value) => {
            let mut f = Formatted::new(value);
            f.set_repr_unchecked(Repr::new_unchecked(value_raw));
            Value::Boolean(f)
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
            let mut f = Formatted::new(value);
            f.set_repr_unchecked(Repr::new_unchecked(value_raw));
            Value::Datetime(f)
        }
        toml_parser::decoder::ScalarKind::Float => {
            let value = match decoded.parse::<f64>() {
                Ok(value) => {
                    if value.is_infinite()
                        && !(decoded
                            .strip_prefix(['+', '-'])
                            .unwrap_or(&decoded)
                            .chars()
                            .all(|c| c.is_ascii_alphabetic()))
                    {
                        errors.report_error(
                            ParseError::new("floating-point number overflowed")
                                .with_unexpected(event.span()),
                        );
                    }
                    value
                }
                Err(_) => {
                    errors.report_error(
                        ParseError::new(kind.invalid_description()).with_unexpected(event.span()),
                    );
                    f64::NAN
                }
            };
            let mut f = Formatted::new(value);
            f.set_repr_unchecked(Repr::new_unchecked(value_raw));
            Value::Float(f)
        }
        toml_parser::decoder::ScalarKind::Integer(radix) => {
            let value = match i64::from_str_radix(&decoded, radix.value()) {
                Ok(value) => value,
                Err(_) => {
                    // Assuming the decoder fully validated it, leaving only overflow errors
                    errors.report_error(
                        ParseError::new("integer number overflowed").with_unexpected(event.span()),
                    );
                    i64::MAX
                }
            };
            let mut f = Formatted::new(value);
            f.set_repr_unchecked(Repr::new_unchecked(value_raw));
            Value::Integer(f)
        }
    }
}
