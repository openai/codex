use crate::key::Key;
use crate::parser::prelude::*;
use crate::repr::Decor;
use crate::repr::Repr;
use crate::RawString;

/// ```bnf
/// key = simple-key / dotted-key
/// dotted-key = simple-key 1*( dot-sep simple-key )
/// ```
pub(crate) fn on_key(
    key_event: &toml_parser::parser::Event,
    input: &mut Input<'_>,
    source: toml_parser::Source<'_>,
    errors: &mut dyn ErrorSink,
) -> (Vec<Key>, Option<Key>) {
    #[cfg(feature = "debug")]
    let _scope = TraceScope::new("key::on_key");
    let mut result_path = Vec::new();
    let mut result_key = None;

    let mut state = State::new(key_event);
    if more_key(input) {
        while let Some(event) = input.next_token() {
            match event.kind() {
                EventKind::StdTableOpen
                | EventKind::ArrayTableOpen
                | EventKind::InlineTableOpen
                | EventKind::InlineTableClose
                | EventKind::ArrayOpen
                | EventKind::ArrayClose
                | EventKind::Scalar
                | EventKind::ValueSep
                | EventKind::Comment
                | EventKind::Newline
                | EventKind::KeyValSep
                | EventKind::StdTableClose
                | EventKind::ArrayTableClose
                | EventKind::Error => {
                    #[cfg(feature = "debug")]
                    trace(
                        &format!("unexpected {event:?}"),
                        anstyle::AnsiColor::Red.on_default(),
                    );
                    continue;
                }
                EventKind::SimpleKey => {
                    state.current_key = Some(*event);

                    if !more_key(input) {
                        break;
                    }
                }
                EventKind::Whitespace => {
                    state.whitespace(event);
                }
                EventKind::KeySep => {
                    state.close_key(&mut result_path, &mut result_key, source, errors);
                }
            }
        }
    }

    state.close_key(&mut result_path, &mut result_key, source, errors);

    #[cfg(not(feature = "unbounded"))]
    if super::LIMIT <= result_path.len() as u32 {
        errors.report_error(ParseError::new("recursion limit"));
        return (Vec::new(), None);
    }

    (result_path, result_key)
}

fn more_key(input: &Input<'_>) -> bool {
    let first = input.get(0).map(|e| e.kind());
    let second = input.get(1).map(|e| e.kind());
    if first == Some(EventKind::KeySep) {
        true
    } else if first == Some(EventKind::Whitespace) && second == Some(EventKind::KeySep) {
        true
    } else {
        false
    }
}

struct State {
    current_prefix: Option<toml_parser::Span>,
    current_key: Option<toml_parser::parser::Event>,
    current_suffix: Option<toml_parser::Span>,
}

impl State {
    fn new(key_event: &toml_parser::parser::Event) -> Self {
        Self {
            current_prefix: None,
            current_key: Some(*key_event),
            current_suffix: None,
        }
    }

    fn whitespace(&mut self, event: &toml_parser::parser::Event) {
        if self.current_key.is_some() {
            self.current_suffix = Some(event.span());
        } else {
            self.current_prefix = Some(event.span());
        }
    }

    fn close_key(
        &mut self,
        result_path: &mut Vec<Key>,
        result_key: &mut Option<Key>,
        source: toml_parser::Source<'_>,
        errors: &mut dyn ErrorSink,
    ) {
        let Some(key) = self.current_key.take() else {
            return;
        };
        let prefix_span = self
            .current_prefix
            .take()
            .unwrap_or_else(|| key.span().before());
        let prefix = RawString::with_span(prefix_span.start()..prefix_span.end());

        let suffix_span = self
            .current_suffix
            .take()
            .unwrap_or_else(|| key.span().after());
        let suffix = RawString::with_span(suffix_span.start()..suffix_span.end());

        let key_span = key.span();
        let key_raw = RawString::with_span(key_span.start()..key_span.end());

        let raw = source.get(key).unwrap();
        let mut decoded = std::borrow::Cow::Borrowed("");
        raw.decode_key(&mut decoded, errors);

        let key = Key::new(decoded)
            .with_repr_unchecked(Repr::new_unchecked(key_raw))
            .with_dotted_decor(Decor::new(prefix, suffix));
        if let Some(last_key) = result_key.replace(key) {
            result_path.push(last_key);
        }
    }
}

/// ```bnf
/// simple-key = quoted-key / unquoted-key
/// quoted-key = basic-string / literal-string
/// ```
pub(crate) fn on_simple_key(
    event: &toml_parser::parser::Event,
    source: toml_parser::Source<'_>,
    errors: &mut dyn ErrorSink,
) -> (RawString, String) {
    #[cfg(feature = "debug")]
    let _scope = TraceScope::new("key::on_simple_key");
    let raw = source.get(event).unwrap();

    let mut key = std::borrow::Cow::Borrowed("");
    raw.decode_key(&mut key, errors);

    let span = event.span();
    let raw = RawString::with_span(span.start()..span.end());
    let key = String::from(key);
    (raw, key)
}
