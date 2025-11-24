use crate::key::Key;
use crate::parser::key::on_key;
use crate::parser::prelude::*;
use crate::parser::value::value;
use crate::repr::Decor;
use crate::Item;
use crate::RawString;
use crate::Value;
use crate::{ArrayOfTables, Document, Table};

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
pub(crate) fn document<'s>(
    input: &mut Input<'_>,
    source: toml_parser::Source<'s>,
    errors: &mut dyn ErrorSink,
) -> Document<&'s str> {
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

                let prefix = state.take_trailing();
                let header = on_table(event, input, source, errors);
                let suffix = ws_comment_newline(input)
                    .map(|s| RawString::with_span(s.start()..s.end()))
                    .unwrap_or_default();
                let decor = Decor::new(prefix, suffix);

                state.start_table(header, decor, errors);
            }
            EventKind::SimpleKey => {
                let key_prefix = state.take_trailing();
                let (path, key) = on_key(event, input, source, errors);
                let Some(mut key) = key else {
                    break;
                };
                let Some(next_event) = input.next_token() else {
                    break;
                };
                let keyval_event;
                let key_suffix;
                if next_event.kind() == EventKind::Whitespace {
                    key_suffix = Some(next_event);
                    let Some(next_event) = input.next_token() else {
                        break;
                    };
                    keyval_event = next_event;
                } else {
                    key_suffix = None;
                    keyval_event = next_event;
                }
                if keyval_event.kind() != EventKind::KeyValSep {
                    break;
                }
                let key_suffix = key_suffix
                    .map(|e| RawString::with_span(e.span().start()..e.span().end()))
                    .unwrap_or_default();
                key.leaf_decor.set_prefix(key_prefix);
                key.leaf_decor.set_suffix(key_suffix);

                let value_prefix = if input
                    .first()
                    .map(|e| e.kind() == EventKind::Whitespace)
                    .unwrap_or(false)
                {
                    input
                        .next_token()
                        .map(|e| RawString::with_span(e.span().start()..e.span().end()))
                } else {
                    None
                }
                .unwrap_or_default();
                let mut value = value(input, source, errors);
                let value_suffix = ws_comment_newline(input)
                    .map(|s| RawString::with_span(s.start()..s.end()))
                    .unwrap_or_default();
                let decor = value.decor_mut();
                decor.set_prefix(value_prefix);
                decor.set_suffix(value_suffix);

                state.capture_key_value(path, key, value, errors);
            }
            EventKind::Whitespace | EventKind::Comment | EventKind::Newline => {
                state.capture_trailing(event);
            }
        }
    }

    state.finish_table(errors);

    let trailing = state.take_trailing();
    Document {
        root: Item::Table(state.root),
        trailing,
        raw: source.input(),
    }
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
fn on_table(
    open_event: &toml_parser::parser::Event,
    input: &mut Input<'_>,
    source: toml_parser::Source<'_>,
    errors: &mut dyn ErrorSink,
) -> TableHeader {
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

    let prefix = current_prefix
        .take()
        .expect("setting a key should set a prefix");
    let suffix = current_suffix
        .take()
        .expect("setting a key should set a suffix");
    if let Some(last_key) = current_key.as_mut() {
        let prefix = RawString::with_span(prefix.start()..prefix.end());
        let suffix = RawString::with_span(suffix.start()..suffix.end());
        let leaf_decor = Decor::new(prefix, suffix);
        *last_key.leaf_decor_mut() = leaf_decor;
    }

    TableHeader {
        path: current_path.unwrap_or_default(),
        key: current_key,
        span: current_span,
        is_array,
    }
}

struct TableHeader {
    path: Vec<Key>,
    key: Option<Key>,
    span: toml_parser::Span,
    is_array: bool,
}

fn ws_comment_newline(input: &mut Input<'_>) -> Option<toml_parser::Span> {
    let mut current_span = None;
    while let Some(event) = input.next_token() {
        match event.kind() {
            EventKind::InlineTableOpen
            | EventKind::InlineTableClose
            | EventKind::ArrayOpen
            | EventKind::ArrayClose
            | EventKind::Scalar
            | EventKind::ValueSep
            | EventKind::Error
            | EventKind::SimpleKey
            | EventKind::KeySep
            | EventKind::KeyValSep
            | EventKind::StdTableOpen
            | EventKind::ArrayTableOpen
            | EventKind::StdTableClose
            | EventKind::ArrayTableClose => {
                #[cfg(feature = "debug")]
                trace(
                    &format!("unexpected {event:?}"),
                    anstyle::AnsiColor::Red.on_default(),
                );
            }
            EventKind::Whitespace | EventKind::Comment => {
                let span = current_span.get_or_insert_with(|| event.span());
                *span = span.append(event.span());
            }
            EventKind::Newline => {
                break;
            }
        }
    }

    current_span
}

#[derive(Default)]
struct State {
    root: Table,
    current_table: Table,
    current_trailing: Option<toml_parser::Span>,
    current_header: Option<TableHeader>,
    current_position: isize,
}

impl State {
    fn capture_trailing(&mut self, event: &toml_parser::parser::Event) {
        let decor = self.current_trailing.get_or_insert(event.span());
        *decor = decor.append(event.span());
    }

    fn capture_key_value(
        &mut self,
        path: Vec<Key>,
        key: Key,
        value: Value,
        errors: &mut dyn ErrorSink,
    ) {
        #[cfg(feature = "debug")]
        let _scope = TraceScope::new("document::capture_key_value");
        #[cfg(feature = "debug")]
        trace(
            &format!(
                "path={:?}",
                path.iter().map(|k| k.get()).collect::<Vec<_>>()
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
            let key_span = get_key_span(&key).expect("all keys have spans");
            errors.report_error(ParseError::new("duplicate key").with_unexpected(key_span));
            return;
        }
        let key_span = get_key_span(&key).expect("all keys have spans");
        match parent_table.items.entry(key) {
            indexmap::map::Entry::Vacant(o) => {
                o.insert(Item::Value(value));
            }
            indexmap::map::Entry::Occupied(existing) => {
                // "Since tables cannot be defined more than once, redefining such tables using a [table] header is not allowed"
                let old_span = existing.key().span().expect("all items have spans");
                let old_span = toml_parser::Span::new_unchecked(old_span.start, old_span.end);
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
        let mut prev_table = std::mem::take(&mut self.current_table);
        if let Some(header) = self.current_header.take() {
            let Some(key) = &header.key else {
                return;
            };
            prev_table.span = Some(header.span.start()..header.span.end());

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
                let entry = parent_table
                    .entry_format(key)
                    .or_insert(Item::ArrayOfTables(ArrayOfTables::new()));
                let Some(array) = entry.as_array_of_tables_mut() else {
                    let key_span = get_key_span(key).expect("all keys have spans");
                    let old_span = entry.span().unwrap_or_default();
                    let old_span = toml_parser::Span::new_unchecked(old_span.start, old_span.end);
                    errors.report_error(
                        ParseError::new("duplicate key")
                            .with_unexpected(key_span)
                            .with_context(old_span),
                    );
                    return;
                };
                array.push(prev_table);
                let span = if let (Some(first), Some(last)) = (
                    array.values.first().and_then(|t| t.span()),
                    array.values.last().and_then(|t| t.span()),
                ) {
                    Some((first.start)..(last.end))
                } else {
                    None
                };
                array.span = span;
            } else {
                let existing = parent_table.insert_formatted(key, Item::Table(prev_table));
                debug_assert!(existing.is_none());
            }
        } else {
            prev_table.span = Some(Default::default());
            self.root = prev_table;
        }
    }

    fn start_table(&mut self, header: TableHeader, decor: Decor, errors: &mut dyn ErrorSink) {
        if !header.is_array {
            // 1. Look up the table on start to ensure the duplicate_key error points to the right line
            // 2. Ensure any child tables from an implicit table are preserved
            let root = &mut self.root;
            if let (Some(parent_table), Some(key)) =
                (descend_path(root, &header.path, false, errors), &header.key)
            {
                if let Some((old_key, old_value)) = parent_table.remove_entry(key.get()) {
                    match old_value {
                        Item::Table(t) if t.implicit && !t.is_dotted() => {
                            self.current_table = t;
                        }
                        // Since tables cannot be defined more than once, redefining such tables using a [table] header is not allowed. Likewise, using dotted keys to redefine tables already defined in [table] form is not allowed.
                        old_value => {
                            let old_span = old_key.span().expect("all items have spans");
                            let old_span =
                                toml_parser::Span::new_unchecked(old_span.start, old_span.end);
                            let key_span = get_key_span(key).expect("all keys have spans");
                            errors.report_error(
                                ParseError::new("duplicate key")
                                    .with_unexpected(key_span)
                                    .with_context(old_span),
                            );

                            if let Item::Table(t) = old_value {
                                self.current_table = t;
                            }
                        }
                    }
                }
            }
        }

        self.current_position += 1;
        self.current_table.decor = decor;
        self.current_table.set_implicit(false);
        self.current_table.set_dotted(false);
        self.current_table.set_position(self.current_position);
        self.current_table.span = Some(header.span.start()..header.span.end());
        self.current_header = Some(header);
    }

    fn take_trailing(&mut self) -> RawString {
        self.current_trailing
            .take()
            .map(|s| RawString::with_span(s.start()..s.end()))
            .unwrap_or_default()
    }
}

fn descend_path<'t>(
    mut table: &'t mut Table,
    path: &[Key],
    dotted: bool,
    errors: &mut dyn ErrorSink,
) -> Option<&'t mut Table> {
    #[cfg(feature = "debug")]
    let _scope = TraceScope::new("document::descend_path");
    #[cfg(feature = "debug")]
    trace(
        &format!(
            "path={:?}",
            path.iter().map(|k| k.get()).collect::<Vec<_>>()
        ),
        anstyle::AnsiColor::Blue.on_default(),
    );
    for key in path.iter() {
        table = match table.entry_format(key) {
            crate::Entry::Vacant(entry) => {
                let mut new_table = Table::new();
                new_table.span = key.span();
                new_table.set_implicit(true);
                new_table.set_dotted(dotted);

                entry.insert(Item::Table(new_table)).as_table_mut().unwrap()
            }
            crate::Entry::Occupied(entry) => {
                match entry.into_mut() {
                    Item::ArrayOfTables(ref mut array) => {
                        debug_assert!(!array.is_empty());

                        let index = array.len() - 1;
                        let last_child = array.get_mut(index).unwrap();

                        last_child
                    }
                    Item::Table(ref mut sweet_child_of_mine) => {
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
                    Item::Value(ref existing) => {
                        let old_span = existing.span().expect("all items have spans");
                        let old_span =
                            toml_parser::Span::new_unchecked(old_span.start, old_span.end);
                        let key_span = get_key_span(key).expect("all keys have spans");
                        errors.report_error(
                            ParseError::new(format!(
                                "cannot extend value of type {} with a dotted key",
                                existing.type_name()
                            ))
                            .with_unexpected(key_span)
                            .with_context(old_span),
                        );
                        return None;
                    }
                    Item::None => unreachable!(),
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
