use crate::key::Key;
use crate::parser::array::on_array;
use crate::parser::key::on_key;
use crate::parser::prelude::*;
use crate::parser::value::on_scalar;
use crate::repr::Decor;
use crate::{InlineTable, Item, RawString, Value};

use indexmap::map::Entry;

/// ```bnf
/// ;; Inline Table
///
/// inline-table = inline-table-open inline-table-keyvals inline-table-close
/// ```
pub(crate) fn on_inline_table(
    open_event: &toml_parser::parser::Event,
    input: &mut Input<'_>,
    source: toml_parser::Source<'_>,
    errors: &mut dyn ErrorSink,
) -> Value {
    #[cfg(feature = "debug")]
    let _scope = TraceScope::new("inline_table::on_inline_table");
    let mut result = InlineTable::new();

    let mut state = State::default();
    while let Some(event) = input.next_token() {
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

    Value::InlineTable(result)
}

#[derive(Default)]
struct State {
    current_prefix: Option<toml_parser::Span>,
    current_key: Option<(Vec<Key>, Key)>,
    seen_keyval_sep: bool,
    current_value: Option<Value>,
    current_suffix: Option<toml_parser::Span>,
}

impl State {
    fn whitespace(&mut self, event: &toml_parser::parser::Event) {
        let decor = if self.is_prefix() {
            self.current_prefix.get_or_insert(event.span())
        } else {
            self.current_suffix.get_or_insert(event.span())
        };
        *decor = decor.append(event.span());
    }

    fn is_prefix(&self) -> bool {
        if self.seen_keyval_sep {
            self.current_value.is_none()
        } else {
            self.current_key.is_none()
        }
    }

    fn capture_key(
        &mut self,
        event: &toml_parser::parser::Event,
        path: Vec<Key>,
        key: Option<Key>,
    ) {
        self.current_prefix
            .get_or_insert_with(|| event.span().before());
        if let Some(key) = key {
            self.current_key = Some((path, key));
        }
    }

    fn finish_key(&mut self, event: &toml_parser::parser::Event) {
        self.seen_keyval_sep = true;
        if let Some(last_key) = self.current_key.as_mut().map(|(_, k)| k) {
            let prefix = self
                .current_prefix
                .take()
                .expect("setting a key should set a prefix");
            let suffix = self
                .current_suffix
                .take()
                .unwrap_or_else(|| event.span().before());
            let prefix = RawString::with_span(prefix.start()..prefix.end());
            let suffix = RawString::with_span(suffix.start()..suffix.end());
            let leaf_decor = Decor::new(prefix, suffix);
            *last_key.leaf_decor_mut() = leaf_decor;
        }
    }

    fn capture_value(&mut self, event: &toml_parser::parser::Event, value: Value) {
        self.current_prefix
            .get_or_insert_with(|| event.span().before());
        self.current_value = Some(value);
    }

    fn finish_value(
        &mut self,
        event: &toml_parser::parser::Event,
        result: &mut InlineTable,
        errors: &mut dyn ErrorSink,
    ) {
        #[cfg(feature = "debug")]
        let _scope = TraceScope::new("inline_table::finish_value");
        self.seen_keyval_sep = false;
        if let (Some((path, key)), Some(mut value)) =
            (self.current_key.take(), self.current_value.take())
        {
            let prefix = self
                .current_prefix
                .take()
                .expect("setting a value should set a prefix");
            let suffix = self
                .current_suffix
                .take()
                .unwrap_or_else(|| event.span().before());
            let Some(table) = descend_path(result, &path, true, errors) else {
                return;
            };

            let decor = value.decor_mut();
            decor.set_prefix(RawString::with_span(prefix.start()..prefix.end()));
            decor.set_suffix(RawString::with_span(suffix.start()..suffix.end()));

            // "Likewise, using dotted keys to redefine tables already defined in [table] form is not allowed"
            let mixed_table_types = table.is_dotted() == path.is_empty();
            if mixed_table_types {
                let key_span = get_key_span(&key).unwrap_or_else(|| event.span());
                errors.report_error(ParseError::new("duplicate key").with_unexpected(key_span));
            } else {
                let key_span = get_key_span(&key).unwrap_or_else(|| event.span());
                match table.items.entry(key) {
                    Entry::Vacant(o) => {
                        o.insert(Item::Value(value));
                    }
                    Entry::Occupied(o) => {
                        let old_span = get_key_span(o.key()).unwrap_or_else(|| event.span());
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
        open_event: &toml_parser::parser::Event,
        close_event: &toml_parser::parser::Event,
        result: &mut InlineTable,
    ) {
        #[cfg(feature = "debug")]
        let _scope = TraceScope::new("inline_table::close");
        let span = open_event.span().append(close_event.span());
        let preamble = self
            .current_prefix
            .take()
            .map(|prefix| RawString::with_span(prefix.start()..prefix.end()));

        result.span = Some(span.start()..span.end());
        if let Some(preamble) = preamble {
            result.set_preamble(preamble);
        }
    }
}

fn descend_path<'a>(
    mut table: &'a mut InlineTable,
    path: &'a [Key],
    dotted: bool,
    errors: &mut dyn ErrorSink,
) -> Option<&'a mut InlineTable> {
    #[cfg(feature = "debug")]
    let _scope = TraceScope::new("inline_table::descend_path");
    #[cfg(feature = "debug")]
    trace(
        &format!("key={:?}", path.iter().map(|k| k.get()).collect::<Vec<_>>()),
        anstyle::AnsiColor::Blue.on_default(),
    );
    for key in path.iter() {
        table = match table.entry_format(key) {
            crate::InlineEntry::Vacant(entry) => {
                let mut new_table = InlineTable::new();
                new_table.span = key.span();
                new_table.set_implicit(true);
                new_table.set_dotted(dotted);
                entry
                    .insert(Value::InlineTable(new_table))
                    .as_inline_table_mut()
                    .unwrap()
            }
            crate::InlineEntry::Occupied(entry) => {
                match entry.into_mut() {
                    Value::InlineTable(ref mut sweet_child_of_mine) => {
                        // Since tables cannot be defined more than once, redefining such tables using a
                        // [table] header is not allowed. Likewise, using dotted keys to redefine tables
                        // already defined in [table] form is not allowed.
                        if dotted && !sweet_child_of_mine.is_implicit() {
                            let key_span = get_key_span(key).expect("all keys have spans");
                            errors.report_error(
                                ParseError::new("duplicate key").with_unexpected(key_span),
                            );
                            return None;
                        }
                        sweet_child_of_mine
                    }
                    item => {
                        let key_span = get_key_span(key).expect("all keys have spans");
                        errors.report_error(
                            ParseError::new(format!(
                                "cannot extend value of type {} with a dotted key",
                                item.type_name()
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

fn get_key_span(key: &Key) -> Option<toml_parser::Span> {
    key.as_repr()
        .and_then(|r| r.span())
        .map(|s| toml_parser::Span::new_unchecked(s.start, s.end))
}
