use serde_spanned::Spanned;

use crate::alloc_prelude::*;
use crate::de::parser::key::on_key;
use crate::de::parser::prelude::*;
use crate::de::parser::value::value;
use crate::de::DeString;
use crate::de::DeValue;
use crate::de::{DeArray, DeTable};
use crate::map::Entry;

/// ```bnf
/// ;; TOML
///
/// toml = expression *( newline expression )
///
/// expression = ( ( ws comment ) /
///                ( ws keyval ws [ comment ] ) /
///                ( ws table ws [ comment ] ) /
///                  ws )
/// ```
pub(crate) fn document<'i>(
    input: &mut Input<'_>,
    source: toml_parser::Source<'i>,
    errors: &mut dyn ErrorSink,
) -> Spanned<DeTable<'i>> {
    #[cfg(feature = "debug")]
    let _scope = TraceScope::new("document::document");
    let mut state = State::default();
    while let Some(event) = input.next_token() {
        match event.kind() {
            EventKind::InlineTableOpen
            | EventKind::InlineTableClose
            | EventKind::ArrayOpen
            | EventKind::ArrayClose
            | EventKind::Scalar
            | EventKind::ValueSep
            | EventKind::Error
            | EventKind::KeySep
            | EventKind::KeyValSep
            | EventKind::StdTableClose
            | EventKind::ArrayTableClose => {
                #[cfg(feature = "debug")]
                trace(
                    &format!("unexpected {event:?}"),
                    anstyle::AnsiColor::Red.on_default(),
                );
                continue;
            }
            EventKind::StdTableOpen | EventKind::ArrayTableOpen => {
                state.finish_table(errors);

                let header = on_table(event, input, source, errors);

                state.start_table(header, errors);
            }
            EventKind::SimpleKey => {
                let (path, key) = on_key(event, input, source, errors);
                let Some(key) = key else {
                    break;
                };
                let Some(next_event) = input.next_token() else {
                    break;
                };
                let keyval_event = if next_event.kind() == EventKind::Whitespace {
                    let Some(next_event) = input.next_token() else {
                        break;
                    };
                    next_event
                } else {
                    next_event
                };
                if keyval_event.kind() != EventKind::KeyValSep {
                    break;
                }

                if input
                    .first()
                    .map(|e| e.kind() == EventKind::Whitespace)
                    .unwrap_or(false)
                {
                    let _ = input.next_token();
                }
                let value = value(input, source, errors);

                state.capture_key_value(path, key, value, errors);
            }
            EventKind::Whitespace | EventKind::Comment | EventKind::Newline => {
                state.capture_trailing(event);
            }
        }
    }

    state.finish_table(errors);

    let span = Default::default();
    Spanned::new(span, state.root)
}

/// ```bnf
/// ;; Standard Table
///
/// std-table = std-table-open key *( table-key-sep key) std-table-close
///
/// ;; Array Table
///
/// array-table = array-table-open key *( table-key-sep key) array-table-close
/// ```
fn on_table<'i>(
    open_event: &toml_parser::parser::Event,
    input: &mut Input<'_>,
    source: toml_parser::Source<'i>,
    errors: &mut dyn ErrorSink,
) -> TableHeader<'i> {
    #[cfg(feature = "debug")]
    let _scope = TraceScope::new("document::on_table");
    let is_array = open_event.kind() == EventKind::ArrayTableOpen;
    let mut current_path = None;
    let mut current_key = None;
    let mut current_span = open_event.span();
    let mut current_prefix = None;
    let mut current_suffix = None;

    while let Some(event) = input.next_token() {
        match event.kind() {
            EventKind::InlineTableOpen
            | EventKind::InlineTableClose
            | EventKind::ArrayOpen
            | EventKind::ArrayClose
            | EventKind::Scalar
            | EventKind::ValueSep
            | EventKind::Error
            | EventKind::KeySep
            | EventKind::KeyValSep
            | EventKind::StdTableOpen
            | EventKind::ArrayTableOpen
            | EventKind::Comment
            | EventKind::Newline => {
                #[cfg(feature = "debug")]
                trace(
                    &format!("unexpected {event:?}"),
                    anstyle::AnsiColor::Red.on_default(),
                );
                continue;
            }
            EventKind::ArrayTableClose | EventKind::StdTableClose => {
                current_span = current_span.append(event.span());
                break;
            }
            EventKind::SimpleKey => {
                current_prefix.get_or_insert_with(|| event.span().before());
                let (path, key) = on_key(event, input, source, errors);
                current_path = Some(path);
                current_key = key;
                current_suffix.get_or_insert_with(|| event.span().after());
            }
            EventKind::Whitespace => {
                if current_key.is_some() {
                    current_suffix = Some(event.span());
                } else {
                    current_prefix = Some(event.span());
                }
            }
        }
    }

    TableHeader {
        path: current_path.unwrap_or_default(),
        key: current_key,
        span: current_span,
        is_array,
    }
}

struct TableHeader<'i> {
    path: Vec<Spanned<DeString<'i>>>,
    key: Option<Spanned<DeString<'i>>>,
    span: toml_parser::Span,
    is_array: bool,
}

#[derive(Default)]
struct State<'i> {
    root: DeTable<'i>,
    current_table: DeTable<'i>,
    current_header: Option<TableHeader<'i>>,
    current_position: usize,
}

impl<'i> State<'i> {
    fn capture_trailing(&mut self, _event: &toml_parser::parser::Event) {}

    fn capture_key_value(
        &mut self,
        path: Vec<Spanned<DeString<'i>>>,
        key: Spanned<DeString<'i>>,
        value: Spanned<DeValue<'i>>,
        errors: &mut dyn ErrorSink,
    ) {
        #[cfg(feature = "debug")]
        let _scope = TraceScope::new("document::capture_key_value");
        #[cfg(feature = "debug")]
        trace(
            &format!(
                "path={:?}",
                path.iter().map(|k| k.get_ref()).collect::<Vec<_>>()
            ),
            anstyle::AnsiColor::Blue.on_default(),
        );
        #[cfg(feature = "debug")]
        trace(
            &format!("key={key}",),
            anstyle::AnsiColor::Blue.on_default(),
        );
        #[cfg(feature = "debug")]
        trace(
            &format!("value={value:?}",),
            anstyle::AnsiColor::Blue.on_default(),
        );

        let dotted = true;
        let Some(parent_table) = descend_path(&mut self.current_table, &path, dotted, errors)
        else {
            return;
        };
        // "Likewise, using dotted keys to redefine tables already defined in [table] form is not allowed"
        let mixed_table_types = parent_table.is_dotted() == path.is_empty();
        if mixed_table_types {
            let key_span = get_key_span(&key);
            errors.report_error(ParseError::new("duplicate key").with_unexpected(key_span));
            return;
        }
        let key_span = get_key_span(&key);
        match parent_table.entry(key) {
            Entry::Vacant(o) => {
                o.insert(value);
            }
            Entry::Occupied(existing) => {
                // "Since tables cannot be defined more than once, redefining such tables using a [table] header is not allowed"
                let old_span = get_key_span(existing.key());
                errors.report_error(
                    ParseError::new("duplicate key")
                        .with_unexpected(key_span)
                        .with_context(old_span),
                );
            }
        }
    }

    fn finish_table(&mut self, errors: &mut dyn ErrorSink) {
        #[cfg(feature = "debug")]
        let _scope = TraceScope::new("document::finish_table");
        let prev_table = core::mem::take(&mut self.current_table);
        if let Some(header) = self.current_header.take() {
            let Some(key) = &header.key else {
                return;
            };
            let header_span = header.span.start()..header.span.end();
            let prev_table = Spanned::new(header_span.clone(), DeValue::Table(prev_table));

            let parent_key = &header.path;
            let dotted = false;
            let Some(parent_table) = descend_path(&mut self.root, parent_key, dotted, errors)
            else {
                return;
            };
            #[cfg(feature = "debug")]
            trace(
                &format!("key={key}",),
                anstyle::AnsiColor::Blue.on_default(),
            );
            if header.is_array {
                let entry = parent_table.entry(key.clone()).or_insert_with(|| {
                    let mut array = DeArray::new();
                    array.set_array_of_tables(true);
                    Spanned::new(header_span, DeValue::Array(array))
                });
                let Some(array) = entry
                    .as_mut()
                    .as_array_mut()
                    .filter(|a| a.is_array_of_tables())
                else {
                    let key_span = get_key_span(key);
                    let old_span = entry.span();
                    let old_span = toml_parser::Span::new_unchecked(old_span.start, old_span.end);
                    errors.report_error(
                        ParseError::new("duplicate key")
                            .with_unexpected(key_span)
                            .with_context(old_span),
                    );
                    return;
                };
                array.push(prev_table);
            } else {
                let existing = parent_table.insert(key.clone(), prev_table);
                debug_assert!(existing.is_none());
            }
        } else {
            self.root = prev_table;
        }
    }

    fn start_table(&mut self, header: TableHeader<'i>, errors: &mut dyn ErrorSink) {
        if !header.is_array {
            // 1. Look up the table on start to ensure the duplicate_key error points to the right line
            // 2. Ensure any child tables from an implicit table are preserved
            let root = &mut self.root;
            if let (Some(parent_table), Some(key)) =
                (descend_path(root, &header.path, false, errors), &header.key)
            {
                if let Some((old_key, old_value)) = parent_table.remove_entry(key) {
                    match old_value.into_inner() {
                        DeValue::Table(t) if t.is_implicit() && !t.is_dotted() => {
                            self.current_table = t;
                        }
                        // Since tables cannot be defined more than once, redefining such tables using a [table] header is not allowed. Likewise, using dotted keys to redefine tables already defined in [table] form is not allowed.
                        old_value => {
                            let old_span = get_key_span(&old_key);
                            let key_span = get_key_span(key);
                            errors.report_error(
                                ParseError::new("duplicate key")
                                    .with_unexpected(key_span)
                                    .with_context(old_span),
                            );

                            if let DeValue::Table(t) = old_value {
                                self.current_table = t;
                            }
                        }
                    }
                }
            }
        }

        self.current_position += 1;
        self.current_table.set_implicit(false);
        self.current_table.set_dotted(false);
        self.current_header = Some(header);
    }
}

fn descend_path<'t, 'i>(
    mut table: &'t mut DeTable<'i>,
    path: &[Spanned<DeString<'i>>],
    dotted: bool,
    errors: &mut dyn ErrorSink,
) -> Option<&'t mut DeTable<'i>> {
    #[cfg(feature = "debug")]
    let _scope = TraceScope::new("document::descend_path");
    #[cfg(feature = "debug")]
    trace(
        &format!(
            "path={:?}",
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

                let value = DeValue::Table(new_table);
                let value = Spanned::new(key.span(), value);
                let value = entry.insert(value);
                value.as_mut().as_table_mut().unwrap()
            }
            Entry::Occupied(entry) => {
                let spanned = entry.into_mut();
                let old_span = spanned.span();
                match spanned.as_mut() {
                    DeValue::Array(ref mut array) => {
                        if !array.is_array_of_tables() {
                            let old_span =
                                toml_parser::Span::new_unchecked(old_span.start, old_span.end);
                            let key_span = get_key_span(key);
                            errors.report_error(
                                ParseError::new(
                                    "cannot extend value of type array with a dotted key",
                                )
                                .with_unexpected(key_span)
                                .with_context(old_span),
                            );
                            return None;
                        }

                        debug_assert!(!array.is_empty());

                        let index = array.len() - 1;
                        let last_child = array.get_mut(index).unwrap();

                        match last_child.as_mut() {
                            DeValue::Table(table) => table,
                            existing => {
                                let old_span =
                                    toml_parser::Span::new_unchecked(old_span.start, old_span.end);
                                let key_span = get_key_span(key);
                                errors.report_error(
                                    ParseError::new(format!(
                                        "cannot extend value of type {} with a dotted key",
                                        existing.type_str()
                                    ))
                                    .with_unexpected(key_span)
                                    .with_context(old_span),
                                );
                                return None;
                            }
                        }
                    }
                    DeValue::Table(ref mut sweet_child_of_mine) => {
                        if sweet_child_of_mine.is_inline() {
                            let key_span = get_key_span(key);
                            errors.report_error(
                                ParseError::new(
                                    "cannot extend value of type inline table with a dotted key",
                                )
                                .with_unexpected(key_span),
                            );
                            return None;
                        }
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
                    existing => {
                        let old_span =
                            toml_parser::Span::new_unchecked(old_span.start, old_span.end);
                        let key_span = get_key_span(key);
                        errors.report_error(
                            ParseError::new(format!(
                                "cannot extend value of type {} with a dotted key",
                                existing.type_str()
                            ))
                            .with_unexpected(key_span)
                            .with_context(old_span),
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
