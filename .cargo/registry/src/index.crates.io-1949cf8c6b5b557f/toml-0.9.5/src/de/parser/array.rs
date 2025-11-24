use serde_spanned::Spanned;

use crate::de::parser::inline_table::on_inline_table;
use crate::de::parser::value::on_scalar;
use crate::de::{DeArray, DeValue};

use crate::de::parser::prelude::*;

/// ```bnf
/// ;; Array
///
/// array = array-open array-values array-close
/// array-values =  ws-comment-newline val ws-comment-newline array-sep array-values
/// array-values =/ ws-comment-newline val ws-comment-newline [ array-sep ]
/// ```
pub(crate) fn on_array<'i>(
    open_event: &toml_parser::parser::Event,
    input: &mut Input<'_>,
    source: toml_parser::Source<'i>,
    errors: &mut dyn ErrorSink,
) -> Spanned<DeValue<'i>> {
    #[cfg(feature = "debug")]
    let _scope = TraceScope::new("array::on_array");
    let mut result = DeArray::new();
    let mut close_span = open_event.span();

    let mut state = State::default();
    state.open(open_event);
    while let Some(event) = input.next_token() {
        close_span = event.span();
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

    let span = open_event.span().start()..close_span.end();

    Spanned::new(span, DeValue::Array(result))
}

#[derive(Default)]
struct State<'i> {
    current_value: Option<Spanned<DeValue<'i>>>,
    trailing_start: Option<usize>,
}

impl<'i> State<'i> {
    fn open(&mut self, _open_event: &toml_parser::parser::Event) {}

    fn whitespace(&mut self, _event: &toml_parser::parser::Event) {}

    fn capture_value(&mut self, _event: &toml_parser::parser::Event, value: Spanned<DeValue<'i>>) {
        self.trailing_start = None;
        self.current_value = Some(value);
    }

    fn finish_value(&mut self, _event: &toml_parser::parser::Event, result: &mut DeArray<'i>) {
        #[cfg(feature = "debug")]
        let _scope = TraceScope::new("array::finish_value");
        if let Some(value) = self.current_value.take() {
            result.push(value);
        }
    }

    fn sep_value(&mut self, event: &toml_parser::parser::Event) {
        self.trailing_start = Some(event.span().end());
    }

    fn close(
        &mut self,
        _open_event: &toml_parser::parser::Event,
        _close_event: &toml_parser::parser::Event,
        _result: &mut DeArray<'i>,
    ) {
        #[cfg(feature = "debug")]
        let _scope = TraceScope::new("array::close");
    }
}
