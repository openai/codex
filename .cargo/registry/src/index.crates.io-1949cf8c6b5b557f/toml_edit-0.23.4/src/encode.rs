use std::borrow::Cow;
use std::fmt::{Display, Formatter, Result, Write};

use toml_datetime::Datetime;
use toml_writer::ToTomlValue as _;
use toml_writer::TomlWrite as _;

use crate::inline_table::DEFAULT_INLINE_KEY_DECOR;
use crate::key::Key;
use crate::repr::{Formatted, Repr, ValueRepr};
use crate::table::{
    DEFAULT_KEY_DECOR, DEFAULT_KEY_PATH_DECOR, DEFAULT_ROOT_DECOR, DEFAULT_TABLE_DECOR,
};
use crate::value::{
    DEFAULT_LEADING_VALUE_DECOR, DEFAULT_TRAILING_VALUE_DECOR, DEFAULT_VALUE_DECOR,
};
use crate::DocumentMut;
use crate::{Array, InlineTable, Item, Table, Value};

pub(crate) fn encode_key(this: &Key, buf: &mut dyn Write, input: Option<&str>) -> Result {
    if let Some(input) = input {
        let repr = this
            .as_repr()
            .map(Cow::Borrowed)
            .unwrap_or_else(|| Cow::Owned(this.default_repr()));
        repr.encode(buf, input)?;
    } else {
        let repr = this.display_repr();
        write!(buf, "{repr}")?;
    };

    Ok(())
}

fn encode_key_path(
    this: &[Key],
    mut buf: &mut dyn Write,
    input: Option<&str>,
    default_decor: (&str, &str),
) -> Result {
    let leaf_decor = this.last().expect("always at least one key").leaf_decor();
    for (i, key) in this.iter().enumerate() {
        let dotted_decor = key.dotted_decor();

        let first = i == 0;
        let last = i + 1 == this.len();

        if first {
            leaf_decor.prefix_encode(buf, input, default_decor.0)?;
        } else {
            buf.key_sep()?;
            dotted_decor.prefix_encode(buf, input, DEFAULT_KEY_PATH_DECOR.0)?;
        }

        encode_key(key, buf, input)?;

        if last {
            leaf_decor.suffix_encode(buf, input, default_decor.1)?;
        } else {
            dotted_decor.suffix_encode(buf, input, DEFAULT_KEY_PATH_DECOR.1)?;
        }
    }
    Ok(())
}

pub(crate) fn encode_key_path_ref(
    this: &[&Key],
    mut buf: &mut dyn Write,
    input: Option<&str>,
    default_decor: (&str, &str),
) -> Result {
    let leaf_decor = this.last().expect("always at least one key").leaf_decor();
    for (i, key) in this.iter().enumerate() {
        let dotted_decor = key.dotted_decor();

        let first = i == 0;
        let last = i + 1 == this.len();

        if first {
            leaf_decor.prefix_encode(buf, input, default_decor.0)?;
        } else {
            buf.key_sep()?;
            dotted_decor.prefix_encode(buf, input, DEFAULT_KEY_PATH_DECOR.0)?;
        }

        encode_key(key, buf, input)?;

        if last {
            leaf_decor.suffix_encode(buf, input, default_decor.1)?;
        } else {
            dotted_decor.suffix_encode(buf, input, DEFAULT_KEY_PATH_DECOR.1)?;
        }
    }
    Ok(())
}

pub(crate) fn encode_formatted<T: ValueRepr>(
    this: &Formatted<T>,
    buf: &mut dyn Write,
    input: Option<&str>,
    default_decor: (&str, &str),
) -> Result {
    let decor = this.decor();
    decor.prefix_encode(buf, input, default_decor.0)?;

    if let Some(input) = input {
        let repr = this
            .as_repr()
            .map(Cow::Borrowed)
            .unwrap_or_else(|| Cow::Owned(this.default_repr()));
        repr.encode(buf, input)?;
    } else {
        let repr = this.display_repr();
        write!(buf, "{repr}")?;
    };

    decor.suffix_encode(buf, input, default_decor.1)?;
    Ok(())
}

pub(crate) fn encode_array(
    this: &Array,
    mut buf: &mut dyn Write,
    input: Option<&str>,
    default_decor: (&str, &str),
) -> Result {
    let decor = this.decor();
    decor.prefix_encode(buf, input, default_decor.0)?;
    buf.open_array()?;

    for (i, elem) in this.iter().enumerate() {
        let inner_decor;
        if i == 0 {
            inner_decor = DEFAULT_LEADING_VALUE_DECOR;
        } else {
            inner_decor = DEFAULT_VALUE_DECOR;
            buf.val_sep()?;
        }
        encode_value(elem, buf, input, inner_decor)?;
    }
    if this.trailing_comma() && !this.is_empty() {
        buf.val_sep()?;
    }

    this.trailing().encode_with_default(buf, input, "")?;
    buf.close_array()?;
    decor.suffix_encode(buf, input, default_decor.1)?;

    Ok(())
}

pub(crate) fn encode_table(
    this: &InlineTable,
    mut buf: &mut dyn Write,
    input: Option<&str>,
    default_decor: (&str, &str),
) -> Result {
    let decor = this.decor();
    decor.prefix_encode(buf, input, default_decor.0)?;
    buf.open_inline_table()?;
    this.preamble().encode_with_default(buf, input, "")?;

    let children = this.get_values();
    let len = children.len();
    for (i, (key_path, value)) in children.into_iter().enumerate() {
        if i != 0 {
            buf.val_sep()?;
        }
        let inner_decor = if i == len - 1 {
            DEFAULT_TRAILING_VALUE_DECOR
        } else {
            DEFAULT_VALUE_DECOR
        };
        encode_key_path_ref(&key_path, buf, input, DEFAULT_INLINE_KEY_DECOR)?;
        buf.keyval_sep()?;
        encode_value(value, buf, input, inner_decor)?;
    }

    buf.close_inline_table()?;
    decor.suffix_encode(buf, input, default_decor.1)?;

    Ok(())
}

pub(crate) fn encode_value(
    this: &Value,
    buf: &mut dyn Write,
    input: Option<&str>,
    default_decor: (&str, &str),
) -> Result {
    match this {
        Value::String(repr) => encode_formatted(repr, buf, input, default_decor),
        Value::Integer(repr) => encode_formatted(repr, buf, input, default_decor),
        Value::Float(repr) => encode_formatted(repr, buf, input, default_decor),
        Value::Boolean(repr) => encode_formatted(repr, buf, input, default_decor),
        Value::Datetime(repr) => encode_formatted(repr, buf, input, default_decor),
        Value::Array(array) => encode_array(array, buf, input, default_decor),
        Value::InlineTable(table) => encode_table(table, buf, input, default_decor),
    }
}

impl Display for DocumentMut {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        let decor = self.decor();
        decor.prefix_encode(f, None, DEFAULT_ROOT_DECOR.0)?;

        let mut path = Vec::new();
        let mut last_position = 0;
        let mut tables = Vec::new();
        visit_nested_tables(self.as_table(), &mut path, false, &mut |t, p, is_array| {
            if let Some(pos) = t.position() {
                last_position = pos;
            }
            tables.push((last_position, t, p.clone(), is_array));
            Ok(())
        })
        .unwrap();

        tables.sort_by_key(|&(id, _, _, _)| id);
        let mut first_table = true;
        for (_, table, path, is_array) in tables {
            visit_table(f, None, table, &path, is_array, &mut first_table)?;
        }
        decor.suffix_encode(f, None, DEFAULT_ROOT_DECOR.1)?;
        self.trailing().encode_with_default(f, None, "")
    }
}

fn visit_nested_tables<'t, F>(
    table: &'t Table,
    path: &mut Vec<Key>,
    is_array_of_tables: bool,
    callback: &mut F,
) -> Result
where
    F: FnMut(&'t Table, &Vec<Key>, bool) -> Result,
{
    if !table.is_dotted() {
        callback(table, path, is_array_of_tables)?;
    }

    for (key, value) in table.items.iter() {
        match value {
            Item::Table(ref t) => {
                let key = key.clone();
                path.push(key);
                visit_nested_tables(t, path, false, callback)?;
                path.pop();
            }
            Item::ArrayOfTables(ref a) => {
                for t in a.iter() {
                    let key = key.clone();
                    path.push(key);
                    visit_nested_tables(t, path, true, callback)?;
                    path.pop();
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn visit_table(
    mut buf: &mut dyn Write,
    input: Option<&str>,
    table: &Table,
    path: &[Key],
    is_array_of_tables: bool,
    first_table: &mut bool,
) -> Result {
    let children = table.get_values();
    // We are intentionally hiding implicit tables without any tables nested under them (ie
    // `table.is_empty()` which is in contrast to `table.get_values().is_empty()`).  We are
    // trusting the user that an empty implicit table is not semantically meaningful
    //
    // This allows a user to delete all tables under this implicit table and the implicit table
    // will disappear.
    //
    // However, this means that users need to take care in deciding what tables get marked as
    // implicit.
    let is_visible_std_table = !(table.implicit && children.is_empty());

    if path.is_empty() {
        // don't print header for the root node
        if !children.is_empty() {
            *first_table = false;
        }
    } else if is_array_of_tables {
        let default_decor = if *first_table {
            *first_table = false;
            ("", DEFAULT_TABLE_DECOR.1)
        } else {
            DEFAULT_TABLE_DECOR
        };
        table.decor.prefix_encode(buf, input, default_decor.0)?;
        buf.open_array_of_tables_header()?;
        encode_key_path(path, buf, input, DEFAULT_KEY_PATH_DECOR)?;
        buf.close_array_of_tables_header()?;
        table.decor.suffix_encode(buf, input, default_decor.1)?;
        writeln!(buf)?;
    } else if is_visible_std_table {
        let default_decor = if *first_table {
            *first_table = false;
            ("", DEFAULT_TABLE_DECOR.1)
        } else {
            DEFAULT_TABLE_DECOR
        };
        table.decor.prefix_encode(buf, input, default_decor.0)?;
        buf.open_table_header()?;
        encode_key_path(path, buf, input, DEFAULT_KEY_PATH_DECOR)?;
        buf.close_table_header()?;
        table.decor.suffix_encode(buf, input, default_decor.1)?;
        writeln!(buf)?;
    }
    // print table body
    for (key_path, value) in children {
        encode_key_path_ref(&key_path, buf, input, DEFAULT_KEY_DECOR)?;
        buf.keyval_sep()?;
        encode_value(value, buf, input, DEFAULT_VALUE_DECOR)?;
        writeln!(buf)?;
    }
    Ok(())
}

impl ValueRepr for String {
    fn to_repr(&self) -> Repr {
        let output = toml_writer::TomlStringBuilder::new(self.as_str())
            .as_default()
            .to_toml_value();
        Repr::new_unchecked(output)
    }
}

impl ValueRepr for i64 {
    fn to_repr(&self) -> Repr {
        let repr = self.to_toml_value();
        Repr::new_unchecked(repr)
    }
}

impl ValueRepr for f64 {
    fn to_repr(&self) -> Repr {
        let repr = self.to_toml_value();
        Repr::new_unchecked(repr)
    }
}

impl ValueRepr for bool {
    fn to_repr(&self) -> Repr {
        let repr = self.to_toml_value();
        Repr::new_unchecked(repr)
    }
}

impl ValueRepr for Datetime {
    fn to_repr(&self) -> Repr {
        Repr::new_unchecked(self.to_string())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        #[cfg(feature = "parse")]
        fn parseable_string(string in "\\PC*") {
            let value = Value::from(string.clone());
            let encoded = value.to_string();
            let _: Value = encoded.parse().unwrap_or_else(|err| {
                panic!("error: {err}

string:
```
{string}
```
value:
```
{value}
```
")
            });
        }
    }

    proptest! {
        #[test]
        #[cfg(feature = "parse")]
        fn parseable_key(string in "\\PC*") {
            let key = Key::new(string.clone());
            let encoded = key.to_string();
            let _: Key = encoded.parse().unwrap_or_else(|err| {
                panic!("error: {err}

string:
```
{string}
```
key:
```
{key}
```
")
            });
        }
    }
}
