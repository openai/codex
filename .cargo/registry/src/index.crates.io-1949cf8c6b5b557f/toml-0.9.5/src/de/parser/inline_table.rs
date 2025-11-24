use serde_spanned::Spanned;

use crate::alloc_prelude::*;
use crate::de::parser::array::on_array;
use crate::de::parser::key::on_key;
use crate::de::parser::prelude::*;
use crate::de::parser::value::on_scalar;
use crate::de::DeString;
use crate::de::DeTable;
use crate::de::DeValue;
use crate::map::Entry;

/// ```bnf
/// ;; Inline Table
///
/// inline-table = inline-table-open inline-table-keyvals inline-table-close
/// ```
pub(crate) fn on_inline_table<'i>(
    open_event: &toml_parser::parser::Event,
    input: &mut Input<'_>,
    source: toml_parser::Source<'i>,
    errors: &mut dyn ErrorSink,
) -> Spanned<DeValue<'i>> {
    #[cfg(feature = "debug")]
    let _scope = TraceScope::new("inline_table::on_inline_table");
    let mut result = DeTable::new();
    result.set_inline(true);
    let mut close_span = open_event.span();

    let mut state = State::default();
    while let Some(event) = input.next_token() {
        close_span = event.span();
        match event.kind() {
            EventKind::StdTableOpen
            | EventKind::ArrayTableOpen
            | EventKind::StdTableClose
            | EventKind::ArrayClose
            | EventKind::ArrayTableClose
            | EventKind::KeySep => {
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
            EventKind::SimpleKey => {
                let (path, key) = on_key(event, input, source, errors);
                state.capture_key(event, path, key);
            }
            EventKind::KeyValSep => {
                state.finish_key(event);
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
                state.finish_value(event, &mut result, errors);
            }
            EventKind::Whitespace | EventKind::Comment | EventKind::Newline => {
                state.whitespace(event);
            }
            EventKind::InlineTableClose => {
                state.finish_value(event, &mut result, errors);
                state.close(open_event, event, &mut result);
                break;
            }
        }
    }

    let span = open_event.span().start()..close_span.end();

    Spanned::new(span, DeValue::Table(result))
}

#[derive(Default)]
struct State<'i> {
    current_key: Option<(Vec<Spanned<DeString<'i>>>, Spanned<DeString<'i>>)>,
    seen_keyval_sep: bool,
    current_value: Option<Spanned<DeValue<'i>>>,
}

impl<'i> State<'i> {
    fn whitespace(&mut self, _event: &toml_parser::parser::Event) {}

    fn capture_key(
        &mut self,
        _event: &toml_parser::parser::Event,
        path: Vec<Spanned<DeString<'i>>>,
        key: Option<Spanned<DeString<'i>>>,
    ) {
        if let Some(key) = key {
            self.current_key = Some((path, key));
        }
    }

    fn finish_key(&mut self, _event: &toml_parser::parser::Event) {
        self.seen_keyval_sep = true;
    }

    fn capture_value(&mut self, _event: &toml_parser::parser::Event, value: Spanned<DeValue<'i>>) {
        self.current_value = Some(value);
    }

    fn finish_value(
        &mut self,
        _event: &toml_parser::parser::Event,
        result: &mut DeTable<'i>,
        errors: &mut dyn ErrorSink,
    ) {
        #[cfg(feature = "debug")]
        let _scope = TraceScope::new("inline_table::finish_value");
        self.seen_keyval_sep = false;
        if let (Some((path, key)), Some(value)) =
            (self.current_key.take(), self.current_value.take())
        {
            let Some(table) = descend_path(result, &path, true, errors) else {
                return;
            };

            // "Likewise, using dotted keys to redefine tables already defined in [table] form is not allowed"
            let mixed_table_types = table.is_dotted() == path.is_empty();
            if mixed_table_types {
                let key_span = get_key_span(&key);
                errors.report_error(ParseError::new("duplicate key").with_unexpected(key_span));
            } else {
                let key_span = get_key_span(&key);
                match table.entry(key) {
                    Entry::Vacant(o) => {
                        o.insert(value);
                    }
                    Entry::Occupied(o) => {
                        let old_span = get_key_span(o.key());
                        errors.report_error(
                            ParseError::new("duplicate key")
                                .with_unexpected(key_span)
                                .with_context(old_span),
                        );
                    }
                }
            }
        }
    }

    fn close(
        &mut self,
        _open_event: &toml_parser::parser::Event,
        _close_event: &toml_parser::parser::Event,
        _result: &mut DeTable<'i>,
    ) {
        #[cfg(feature = "debug")]
        let _scope = TraceScope::new("inline_table::close");
    }
}

fn descend_path<'a, 'i>(
    mut table: &'a mut DeTable<'i>,
    path: &'a [Spanned<DeString<'i>>],
    dotted: bool,
    errors: &mut dyn ErrorSink,
) -> Option<&'a mut DeTable<'i>> {
    #[cfg(feature = "debug")]
    let _scope = TraceScope::new("inline_table::descend_path");
    #[cfg(feature = "debug")]
    trace(
        &format!(
            "key={:?}",
            path.iter().map(|k| k.get_ref()).collect::<Vec<_>>()
        ),
        anstyle::AnsiColor::Blue.on_default(),
    );
    for key in path.iter() {
        table = match table.entry(key.clone()) {
            Entry::Vacant(entry) => {
                let mut new_table = DeTable::new();
                new_table.set_implicit(true);
                new_table.set_dotted(dotted);
                new_table.set_inline(true);
                let value = DeValue::Table(new_table);
                let value = Spanned::new(key.span(), value);
                let value = entry.insert(value);
                value.as_mut().as_table_mut().unwrap()
            }
            Entry::Occupied(entry) => {
                let spanned = entry.into_mut();
                match spanned.as_mut() {
                    DeValue::Table(ref mut sweet_child_of_mine) => {
                        // Since tables cannot be defined more than once, redefining such tables using a
                        // [table] header is not allowed. Likewise, using dotted keys to redefine tables
                        // already defined in [table] form is not allowed.
                        if dotted && !sweet_child_of_mine.is_implicit() {
                            let key_span = get_key_span(key);
                            errors.report_error(
                                ParseError::new("duplicate key").with_unexpected(key_span),
                            );
                            return None;
                        }
                        sweet_child_of_mine
                    }
                    item => {
                        let key_span = get_key_span(key);
                        errors.report_error(
                            ParseError::new(format!(
                                "cannot extend value of type {} with a dotted key",
                                item.type_str()
                            ))
                            .with_unexpected(key_span),
                        );
                        return None;
                    }
                }
            }
        };
    }
    Some(table)
}

fn get_key_span(key: &Spanned<DeString<'_>>) -> toml_parser::Span {
    let key_span = key.span();
    toml_parser::Span::new_unchecked(key_span.start, key_span.end)
}
