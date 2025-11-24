use crate::parser::inline_table::on_inline_table;
use crate::parser::value::on_scalar;
use crate::{Array, RawString, Value};

use crate::parser::prelude::*;

/// ```bnf
/// ;; Array
///
/// array = array-open array-values array-close
/// array-values =  ws-comment-newline val ws-comment-newline array-sep array-values
/// array-values =/ ws-comment-newline val ws-comment-newline [ array-sep ]
/// ```
pub(crate) fn on_array(
    open_event: &toml_parser::parser::Event,
    input: &mut Input<'_>,
    source: toml_parser::Source<'_>,
    errors: &mut dyn ErrorSink,
) -> Value {
    #[cfg(feature = "debug")]
    let _scope = TraceScope::new("array::on_array");
    let mut result = Array::new();

    let mut state = State::default();
    state.open(open_event);
    while let Some(event) = input.next_token() {
        match event.kind() {
            EventKind::StdTableOpen
            | EventKind::ArrayTableOpen
            | EventKind::InlineTableClose
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
                break;
            }
            EventKind::Error => {
                #[cfg(feature = "debug")]
                trace(
                    &format!("unexpected {event:?}"),
                    anstyle::AnsiColor::Red.on_default(),
                );
                continue;
            }
            EventKind::InlineTableOpen => {
                let value = on_inline_table(event, input, source, errors);
                state.capture_value(event, value);
            }
            EventKind::ArrayOpen => {
                let value = on_array(event, input, source, errors);
                state.capture_value(event, value);
            }
            EventKind::Scalar => {
                let value = on_scalar(event, source, errors);
                state.capture_value(event, value);
            }
            EventKind::ValueSep => {
                state.finish_value(event, &mut result);
                state.sep_value(event);
            }
            EventKind::Whitespace | EventKind::Comment | EventKind::Newline => {
                state.whitespace(event);
            }
            EventKind::ArrayClose => {
                state.finish_value(event, &mut result);
                state.close(open_event, event, &mut result);
                break;
            }
        }
    }

    Value::Array(result)
}

#[derive(Default)]
struct State {
    current_prefix: Option<toml_parser::Span>,
    current_value: Option<Value>,
    trailing_start: Option<usize>,
    current_suffix: Option<toml_parser::Span>,
}

impl State {
    fn open(&mut self, open_event: &toml_parser::parser::Event) {
        self.trailing_start = Some(open_event.span().end());
    }

    fn whitespace(&mut self, event: &toml_parser::parser::Event) {
        let decor = if self.is_prefix() {
            self.current_prefix.get_or_insert(event.span())
        } else {
            self.current_suffix.get_or_insert(event.span())
        };
        *decor = decor.append(event.span());
    }

    fn is_prefix(&self) -> bool {
        self.current_value.is_none()
    }

    fn capture_value(&mut self, event: &toml_parser::parser::Event, value: Value) {
        self.trailing_start = None;
        self.current_prefix
            .get_or_insert_with(|| event.span().before());
        self.current_value = Some(value);
    }

    fn finish_value(&mut self, event: &toml_parser::parser::Event, result: &mut Array) {
        #[cfg(feature = "debug")]
        let _scope = TraceScope::new("array::finish_value");
        if let Some(mut value) = self.current_value.take() {
            let prefix = self
                .current_prefix
                .take()
                .expect("setting a value should set a prefix");
            let suffix = self
                .current_suffix
                .take()
                .unwrap_or_else(|| event.span().before());
            let decor = value.decor_mut();
            decor.set_prefix(RawString::with_span(prefix.start()..prefix.end()));
            decor.set_suffix(RawString::with_span(suffix.start()..suffix.end()));
            result.push_formatted(value);
        }
    }

    fn sep_value(&mut self, event: &toml_parser::parser::Event) {
        self.trailing_start = Some(event.span().end());
    }

    fn close(
        &mut self,
        open_event: &toml_parser::parser::Event,
        close_event: &toml_parser::parser::Event,
        result: &mut Array,
    ) {
        #[cfg(feature = "debug")]
        let _scope = TraceScope::new("array::close");
        let trailing_comma = self.trailing_start.is_some() && !result.is_empty();
        let span = open_event.span().append(close_event.span());
        let trailing_start = self
            .trailing_start
            .unwrap_or_else(|| close_event.span().start());
        let trailing_end = close_event.span().start();

        result.set_trailing_comma(trailing_comma);
        result.set_trailing(RawString::with_span(trailing_start..trailing_end));
        result.span = Some(span.start()..span.end());
    }
}
