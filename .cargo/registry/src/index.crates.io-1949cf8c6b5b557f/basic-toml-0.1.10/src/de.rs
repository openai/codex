use crate::tokens::{Error as TokenError, Span, Token, Tokenizer};
use serde::de;
use serde::de::IntoDeserializer;
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::error;
use std::f64;
use std::fmt::{self, Display};
use std::iter;
use std::str;
use std::vec;

type TablePair<'a> = ((Span, Cow<'a, str>), Value<'a>);

/// Deserializes a byte slice into a type.
///
/// This function will attempt to interpret `bytes` as UTF-8 data and then
/// deserialize `T` from the TOML document provided.
pub fn from_slice<'de, T>(bytes: &'de [u8]) -> Result<T, crate::Error>
where
    T: de::Deserialize<'de>,
{
    match str::from_utf8(bytes) {
        Ok(s) => from_str(s),
        Err(e) => Err(crate::Error::from(*Error::custom(None, e.to_string()))),
    }
}

/// Deserializes a string into a type.
///
/// This function will attempt to interpret `s` as a TOML document and
/// deserialize `T` from the document.
pub fn from_str<'de, T>(s: &'de str) -> Result<T, crate::Error>
where
    T: de::Deserialize<'de>,
{
    let mut d = Deserializer::new(s);
    T::deserialize(&mut d).map_err(|e| crate::Error::from(*e))
}

#[derive(Debug)]
pub(crate) struct Error {
    kind: ErrorKind,
    line: Option<usize>,
    col: usize,
    at: Option<usize>,
    message: String,
    key: Vec<String>,
}

/// Errors that can occur when deserializing a type.
#[derive(Debug)]
enum ErrorKind {
    /// EOF was reached when looking for a value.
    UnexpectedEof,

    /// An invalid character not allowed in a string was found.
    InvalidCharInString(char),

    /// An invalid character was found as an escape.
    InvalidEscape(char),

    /// An invalid character was found in a hex escape.
    InvalidHexEscape(char),

    /// An invalid escape value was specified in a hex escape in a string.
    ///
    /// Valid values are in the plane of unicode codepoints.
    InvalidEscapeValue(u32),

    /// A newline in a string was encountered when one was not allowed.
    NewlineInString,

    /// An unexpected character was encountered, typically when looking for a
    /// value.
    Unexpected(char),

    /// An unterminated string was found where EOF was found before the ending
    /// EOF mark.
    UnterminatedString,

    /// A newline was found in a table key.
    NewlineInTableKey,

    /// A number failed to parse.
    NumberInvalid,

    /// Wanted one sort of token, but found another.
    Wanted {
        /// Expected token type.
        expected: &'static str,
        /// Actually found token type.
        found: &'static str,
    },

    /// A duplicate table definition was found.
    DuplicateTable(String),

    /// Duplicate key in table.
    DuplicateKey(String),

    /// A previously defined table was redefined as an array.
    RedefineAsArray,

    /// Multiline strings are not allowed for key.
    MultilineStringKey,

    /// A custom error which could be generated when deserializing a particular
    /// type.
    Custom,

    /// A tuple with a certain number of elements was expected but something
    /// else was found.
    ExpectedTuple(usize),

    /// Expected table keys to be in increasing tuple index order, but something
    /// else was found.
    ExpectedTupleIndex {
        /// Expected index.
        expected: usize,
        /// Key that was specified.
        found: String,
    },

    /// An empty table was expected but entries were found.
    ExpectedEmptyTable,

    /// Dotted key attempted to extend something that is not a table.
    DottedKeyInvalidType,

    /// An unexpected key was encountered.
    ///
    /// Used when deserializing a struct with a limited set of fields.
    UnexpectedKeys {
        /// The unexpected keys.
        keys: Vec<String>,
        /// Keys that may be specified.
        available: &'static [&'static str],
    },

    /// Unquoted string was found when quoted one was expected.
    UnquotedString,
}

struct Deserializer<'a> {
    input: &'a str,
    tokens: Tokenizer<'a>,
}

impl<'de> de::Deserializer<'de> for &mut Deserializer<'de> {
    type Error = Box<Error>;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Box<Error>>
    where
        V: de::Visitor<'de>,
    {
        let mut tables = self.tables()?;
        let table_indices = build_table_indices(&tables);
        let table_pindices = build_table_pindices(&tables);

        let res = visitor.visit_map(MapVisitor {
            values: Vec::new().into_iter().peekable(),
            next_value: None,
            depth: 0,
            cur: 0,
            cur_parent: 0,
            max: tables.len(),
            table_indices: &table_indices,
            table_pindices: &table_pindices,
            tables: &mut tables,
            array: false,
            de: self,
            keys: HashSet::new(),
        });
        res.map_err(|mut err| {
            // Errors originating from this library (toml), have an offset
            // attached to them already. Other errors, like those originating
            // from serde (like "missing field") or from a custom deserializer,
            // do not have offsets on them. Here, we do a best guess at their
            // location, by attributing them to the "current table" (the last
            // item in `tables`).
            err.fix_offset(|| tables.last().map(|table| table.at));
            err.fix_linecol(|at| self.to_linecol(at));
            err
        })
    }

    serde::forward_to_deserialize_any! {
        bool u8 u16 u32 u64 i8 i16 i32 i64 f32 f64 char str string seq
        bytes byte_buf map unit newtype_struct
        ignored_any unit_struct tuple_struct tuple option identifier struct enum
    }
}

// Builds a datastructure that allows for efficient sublinear lookups. The
// returned HashMap contains a mapping from table header (like [a.b.c]) to list
// of tables with that precise name. The tables are being identified by their
// index in the passed slice. We use a list as the implementation uses this data
// structure for arrays as well as tables, so if any top level [[name]] array
// contains multiple entries, there are multiple entries in the list. The lookup
// is performed in the `SeqAccess` implementation of `MapVisitor`. The lists are
// ordered, which we exploit in the search code by using bisection.
fn build_table_indices<'de>(tables: &[Table<'de>]) -> HashMap<Vec<Cow<'de, str>>, Vec<usize>> {
    let mut res = HashMap::new();
    for (i, table) in tables.iter().enumerate() {
        let header = table.header.iter().map(|v| v.1.clone()).collect::<Vec<_>>();
        res.entry(header).or_insert_with(Vec::new).push(i);
    }
    res
}

// Builds a datastructure that allows for efficient sublinear lookups. The
// returned HashMap contains a mapping from table header (like [a.b.c]) to list
// of tables whose name at least starts with the specified name. So searching
// for [a.b] would give both [a.b.c.d] as well as [a.b.e]. The tables are being
// identified by their index in the passed slice.
//
// A list is used for two reasons: First, the implementation also stores arrays
// in the same data structure and any top level array of size 2 or greater
// creates multiple entries in the list with the same shared name. Second, there
// can be multiple tables sharing the same prefix.
//
// The lookup is performed in the `MapAccess` implementation of `MapVisitor`.
// The lists are ordered, which we exploit in the search code by using
// bisection.
fn build_table_pindices<'de>(tables: &[Table<'de>]) -> HashMap<Vec<Cow<'de, str>>, Vec<usize>> {
    let mut res = HashMap::new();
    for (i, table) in tables.iter().enumerate() {
        let header = table.header.iter().map(|v| v.1.clone()).collect::<Vec<_>>();
        for len in 0..=header.len() {
            res.entry(header[..len].to_owned())
                .or_insert_with(Vec::new)
                .push(i);
        }
    }
    res
}

fn headers_equal(hdr_a: &[(Span, Cow<str>)], hdr_b: &[(Span, Cow<str>)]) -> bool {
    if hdr_a.len() != hdr_b.len() {
        return false;
    }
    hdr_a.iter().zip(hdr_b.iter()).all(|(h1, h2)| h1.1 == h2.1)
}

struct Table<'a> {
    at: usize,
    header: Vec<(Span, Cow<'a, str>)>,
    values: Option<Vec<TablePair<'a>>>,
    array: bool,
}

struct MapVisitor<'de, 'b> {
    values: iter::Peekable<vec::IntoIter<TablePair<'de>>>,
    next_value: Option<TablePair<'de>>,
    depth: usize,
    cur: usize,
    cur_parent: usize,
    max: usize,
    table_indices: &'b HashMap<Vec<Cow<'de, str>>, Vec<usize>>,
    table_pindices: &'b HashMap<Vec<Cow<'de, str>>, Vec<usize>>,
    tables: &'b mut [Table<'de>],
    array: bool,
    de: &'b mut Deserializer<'de>,
    keys: HashSet<Cow<'de, str>>,
}

impl<'de, 'b> de::MapAccess<'de> for MapVisitor<'de, 'b> {
    type Error = Box<Error>;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Box<Error>>
    where
        K: de::DeserializeSeed<'de>,
    {
        if self.cur_parent == self.max || self.cur == self.max {
            return Ok(None);
        }

        loop {
            assert!(self.next_value.is_none());
            if let Some(((span, key), value)) = self.values.next() {
                if !self.keys.insert(key.clone()) {
                    return Err(Error::from_kind(
                        Some(span.start),
                        ErrorKind::DuplicateKey(key.into_owned()),
                    ));
                }
                let ret = seed.deserialize(StrDeserializer::new(key.clone()))?;
                self.next_value = Some(((span, key), value));
                return Ok(Some(ret));
            }

            let next_table = {
                let prefix_stripped = self.tables[self.cur_parent].header[..self.depth]
                    .iter()
                    .map(|v| v.1.clone())
                    .collect::<Vec<_>>();
                self.table_pindices
                    .get(&prefix_stripped)
                    .and_then(|entries| {
                        let start = entries.binary_search(&self.cur).unwrap_or_else(|v| v);
                        if start == entries.len() || entries[start] < self.cur {
                            return None;
                        }
                        entries[start..]
                            .iter()
                            .filter_map(|i| if *i < self.max { Some(*i) } else { None })
                            .map(|i| (i, &self.tables[i]))
                            .find(|(_, table)| table.values.is_some())
                            .map(|p| p.0)
                    })
            };

            let pos = match next_table {
                Some(pos) => pos,
                None => return Ok(None),
            };
            self.cur = pos;

            // Test to see if we're duplicating our parent's table, and if so
            // then this is an error in the toml format
            if self.cur_parent != pos {
                if headers_equal(
                    &self.tables[self.cur_parent].header,
                    &self.tables[pos].header,
                ) {
                    let at = self.tables[pos].at;
                    let name = self.tables[pos]
                        .header
                        .iter()
                        .map(|k| k.1.clone())
                        .collect::<Vec<_>>()
                        .join(".");
                    return Err(self.de.error(at, ErrorKind::DuplicateTable(name)));
                }

                // If we're here we know we should share the same prefix, and if
                // the longer table was defined first then we want to narrow
                // down our parent's length if possible to ensure that we catch
                // duplicate tables defined afterwards.
                let parent_len = self.tables[self.cur_parent].header.len();
                let cur_len = self.tables[pos].header.len();
                if cur_len < parent_len {
                    self.cur_parent = pos;
                }
            }

            let table = &mut self.tables[pos];

            // If we're not yet at the appropriate depth for this table then we
            // just next the next portion of its header and then continue
            // decoding.
            if self.depth != table.header.len() {
                let (span, key) = &table.header[self.depth];
                if !self.keys.insert(key.clone()) {
                    return Err(Error::from_kind(
                        Some(span.start),
                        ErrorKind::DuplicateKey(key.clone().into_owned()),
                    ));
                }
                let key = seed.deserialize(StrDeserializer::new(key.clone()))?;
                return Ok(Some(key));
            }

            // Rule out cases like:
            //
            //      [[foo.bar]]
            //      [[foo]]
            if table.array {
                let kind = ErrorKind::RedefineAsArray;
                return Err(self.de.error(table.at, kind));
            }

            self.values = table
                .values
                .take()
                .expect("Unable to read table values")
                .into_iter()
                .peekable();
        }
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value, Box<Error>>
    where
        V: de::DeserializeSeed<'de>,
    {
        if let Some((k, v)) = self.next_value.take() {
            match seed.deserialize(ValueDeserializer::new(v)) {
                Ok(v) => return Ok(v),
                Err(mut e) => {
                    e.add_key_context(&k.1);
                    return Err(e);
                }
            }
        }

        let array =
            self.tables[self.cur].array && self.depth == self.tables[self.cur].header.len() - 1;
        self.cur += 1;
        let res = seed.deserialize(MapVisitor {
            values: Vec::new().into_iter().peekable(),
            next_value: None,
            depth: self.depth + if array { 0 } else { 1 },
            cur_parent: self.cur - 1,
            cur: 0,
            max: self.max,
            array,
            table_indices: self.table_indices,
            table_pindices: self.table_pindices,
            tables: &mut *self.tables,
            de: &mut *self.de,
            keys: HashSet::new(),
        });
        res.map_err(|mut e| {
            e.add_key_context(&self.tables[self.cur - 1].header[self.depth].1);
            e
        })
    }
}

impl<'de, 'b> de::SeqAccess<'de> for MapVisitor<'de, 'b> {
    type Error = Box<Error>;

    fn next_element_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Box<Error>>
    where
        K: de::DeserializeSeed<'de>,
    {
        assert!(self.next_value.is_none());
        assert!(self.values.next().is_none());

        if self.cur_parent == self.max {
            return Ok(None);
        }

        let header_stripped = self.tables[self.cur_parent]
            .header
            .iter()
            .map(|v| v.1.clone())
            .collect::<Vec<_>>();
        let start_idx = self.cur_parent + 1;
        let next = self
            .table_indices
            .get(&header_stripped)
            .and_then(|entries| {
                let start = entries.binary_search(&start_idx).unwrap_or_else(|v| v);
                if start == entries.len() || entries[start] < start_idx {
                    return None;
                }
                entries[start..]
                    .iter()
                    .filter_map(|i| if *i < self.max { Some(*i) } else { None })
                    .map(|i| (i, &self.tables[i]))
                    .find(|(_, table)| table.array)
                    .map(|p| p.0)
            })
            .unwrap_or(self.max);

        let ret = seed.deserialize(MapVisitor {
            values: self.tables[self.cur_parent]
                .values
                .take()
                .expect("Unable to read table values")
                .into_iter()
                .peekable(),
            next_value: None,
            depth: self.depth + 1,
            cur_parent: self.cur_parent,
            max: next,
            cur: 0,
            array: false,
            table_indices: self.table_indices,
            table_pindices: self.table_pindices,
            tables: self.tables,
            de: self.de,
            keys: HashSet::new(),
        })?;
        self.cur_parent = next;
        Ok(Some(ret))
    }
}

impl<'de, 'b> de::Deserializer<'de> for MapVisitor<'de, 'b> {
    type Error = Box<Error>;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Box<Error>>
    where
        V: de::Visitor<'de>,
    {
        if self.array {
            visitor.visit_seq(self)
        } else {
            visitor.visit_map(self)
        }
    }

    // `None` is interpreted as a missing field so be sure to implement `Some`
    // as a present field.
    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Box<Error>>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_some(self)
    }

    fn deserialize_newtype_struct<V>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Box<Error>>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_newtype_struct(self)
    }

    serde::forward_to_deserialize_any! {
        bool u8 u16 u32 u64 i8 i16 i32 i64 f32 f64 char str string seq
        bytes byte_buf map unit identifier
        ignored_any unit_struct tuple_struct tuple struct enum
    }
}

struct StrDeserializer<'a> {
    key: Cow<'a, str>,
}

impl<'a> StrDeserializer<'a> {
    fn new(key: Cow<'a, str>) -> StrDeserializer<'a> {
        StrDeserializer { key }
    }
}

impl<'a> de::IntoDeserializer<'a, Box<Error>> for StrDeserializer<'a> {
    type Deserializer = Self;

    fn into_deserializer(self) -> Self::Deserializer {
        self
    }
}

impl<'de> de::Deserializer<'de> for StrDeserializer<'de> {
    type Error = Box<Error>;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Box<Error>>
    where
        V: de::Visitor<'de>,
    {
        match self.key {
            Cow::Borrowed(s) => visitor.visit_borrowed_str(s),
            Cow::Owned(s) => visitor.visit_string(s),
        }
    }

    serde::forward_to_deserialize_any! {
        bool u8 u16 u32 u64 i8 i16 i32 i64 f32 f64 char str string seq
        bytes byte_buf map option unit newtype_struct
        ignored_any unit_struct tuple_struct tuple enum identifier struct
    }
}

struct ValueDeserializer<'a> {
    value: Value<'a>,
    validate_struct_keys: bool,
}

impl<'a> ValueDeserializer<'a> {
    fn new(value: Value<'a>) -> ValueDeserializer<'a> {
        ValueDeserializer {
            value,
            validate_struct_keys: false,
        }
    }

    fn with_struct_key_validation(mut self) -> Self {
        self.validate_struct_keys = true;
        self
    }
}

impl<'de> de::Deserializer<'de> for ValueDeserializer<'de> {
    type Error = Box<Error>;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Box<Error>>
    where
        V: de::Visitor<'de>,
    {
        let start = self.value.start;
        let res = match self.value.e {
            E::Integer(i) => visitor.visit_i64(i),
            E::Boolean(b) => visitor.visit_bool(b),
            E::Float(f) => visitor.visit_f64(f),
            E::String(Cow::Borrowed(s)) => visitor.visit_borrowed_str(s),
            E::String(Cow::Owned(s)) => visitor.visit_string(s),
            E::Array(values) => {
                let mut s = de::value::SeqDeserializer::new(values.into_iter());
                let ret = visitor.visit_seq(&mut s)?;
                s.end()?;
                Ok(ret)
            }
            E::InlineTable(values) | E::DottedTable(values) => {
                visitor.visit_map(InlineTableDeserializer {
                    values: values.into_iter(),
                    next_value: None,
                    keys: HashSet::new(),
                })
            }
        };
        res.map_err(|mut err| {
            // Attribute the error to whatever value returned the error.
            err.fix_offset(|| Some(start));
            err
        })
    }

    fn deserialize_struct<V>(
        self,
        _name: &'static str,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Box<Error>>
    where
        V: de::Visitor<'de>,
    {
        if self.validate_struct_keys {
            match self.value.e {
                E::InlineTable(ref values) | E::DottedTable(ref values) => {
                    let extra_fields = values
                        .iter()
                        .filter_map(|key_value| {
                            let (ref key, ref _val) = *key_value;
                            if fields.contains(&&*(key.1)) {
                                None
                            } else {
                                Some(key.clone())
                            }
                        })
                        .collect::<Vec<_>>();

                    if !extra_fields.is_empty() {
                        return Err(Error::from_kind(
                            Some(self.value.start),
                            ErrorKind::UnexpectedKeys {
                                keys: extra_fields
                                    .iter()
                                    .map(|k| k.1.to_string())
                                    .collect::<Vec<_>>(),
                                available: fields,
                            },
                        ));
                    }
                }
                _ => {}
            }
        }

        self.deserialize_any(visitor)
    }

    // `None` is interpreted as a missing field so be sure to implement `Some`
    // as a present field.
    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Box<Error>>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_some(self)
    }

    fn deserialize_enum<V>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Box<Error>>
    where
        V: de::Visitor<'de>,
    {
        match self.value.e {
            E::String(val) => visitor.visit_enum(val.into_deserializer()),
            e => Err(Error::from_kind(
                Some(self.value.start),
                ErrorKind::Wanted {
                    expected: "string",
                    found: e.type_name(),
                },
            )),
        }
    }

    fn deserialize_newtype_struct<V>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Box<Error>>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_newtype_struct(self)
    }

    serde::forward_to_deserialize_any! {
        bool u8 u16 u32 u64 i8 i16 i32 i64 f32 f64 char str string seq
        bytes byte_buf map unit identifier
        ignored_any unit_struct tuple_struct tuple
    }
}

impl<'de, 'b> de::IntoDeserializer<'de, Box<Error>> for MapVisitor<'de, 'b> {
    type Deserializer = MapVisitor<'de, 'b>;

    fn into_deserializer(self) -> Self::Deserializer {
        self
    }
}

impl<'de> de::IntoDeserializer<'de, Box<Error>> for &mut Deserializer<'de> {
    type Deserializer = Self;

    fn into_deserializer(self) -> Self::Deserializer {
        self
    }
}

impl<'de> de::IntoDeserializer<'de, Box<Error>> for Value<'de> {
    type Deserializer = ValueDeserializer<'de>;

    fn into_deserializer(self) -> Self::Deserializer {
        ValueDeserializer::new(self)
    }
}

struct InlineTableDeserializer<'de> {
    values: vec::IntoIter<TablePair<'de>>,
    next_value: Option<Value<'de>>,
    keys: HashSet<Cow<'de, str>>,
}

impl<'de> de::MapAccess<'de> for InlineTableDeserializer<'de> {
    type Error = Box<Error>;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Box<Error>>
    where
        K: de::DeserializeSeed<'de>,
    {
        let ((span, key), value) = match self.values.next() {
            Some(pair) => pair,
            None => return Ok(None),
        };
        self.next_value = Some(value);
        if !self.keys.insert(key.clone()) {
            return Err(Error::from_kind(
                Some(span.start),
                ErrorKind::DuplicateKey(key.into_owned()),
            ));
        }
        seed.deserialize(StrDeserializer::new(key)).map(Some)
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value, Box<Error>>
    where
        V: de::DeserializeSeed<'de>,
    {
        let value = self.next_value.take().expect("Unable to read table values");
        seed.deserialize(ValueDeserializer::new(value))
    }
}

impl<'de> de::EnumAccess<'de> for InlineTableDeserializer<'de> {
    type Error = Box<Error>;
    type Variant = TableEnumDeserializer<'de>;

    fn variant_seed<V>(mut self, seed: V) -> Result<(V::Value, Self::Variant), Self::Error>
    where
        V: de::DeserializeSeed<'de>,
    {
        let (key, value) = match self.values.next() {
            Some(pair) => pair,
            None => {
                return Err(Error::from_kind(
                    None, // FIXME: How do we get an offset here?
                    ErrorKind::Wanted {
                        expected: "table with exactly 1 entry",
                        found: "empty table",
                    },
                ));
            }
        };

        seed.deserialize(StrDeserializer::new(key.1))
            .map(|val| (val, TableEnumDeserializer { value }))
    }
}

/// Deserializes table values into enum variants.
struct TableEnumDeserializer<'a> {
    value: Value<'a>,
}

impl<'de> de::VariantAccess<'de> for TableEnumDeserializer<'de> {
    type Error = Box<Error>;

    fn unit_variant(self) -> Result<(), Self::Error> {
        match self.value.e {
            E::InlineTable(values) | E::DottedTable(values) => {
                if values.is_empty() {
                    Ok(())
                } else {
                    Err(Error::from_kind(
                        Some(self.value.start),
                        ErrorKind::ExpectedEmptyTable,
                    ))
                }
            }
            e => Err(Error::from_kind(
                Some(self.value.start),
                ErrorKind::Wanted {
                    expected: "table",
                    found: e.type_name(),
                },
            )),
        }
    }

    fn newtype_variant_seed<T>(self, seed: T) -> Result<T::Value, Self::Error>
    where
        T: de::DeserializeSeed<'de>,
    {
        seed.deserialize(ValueDeserializer::new(self.value))
    }

    fn tuple_variant<V>(self, len: usize, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        match self.value.e {
            E::InlineTable(values) | E::DottedTable(values) => {
                let tuple_values = values
                    .into_iter()
                    .enumerate()
                    .map(|(index, (key, value))| match key.1.parse::<usize>() {
                        Ok(key_index) if key_index == index => Ok(value),
                        Ok(_) | Err(_) => Err(Error::from_kind(
                            Some(key.0.start),
                            ErrorKind::ExpectedTupleIndex {
                                expected: index,
                                found: key.1.to_string(),
                            },
                        )),
                    })
                    .collect::<Result<Vec<_>, _>>()?;

                if tuple_values.len() == len {
                    de::Deserializer::deserialize_seq(
                        ValueDeserializer::new(Value {
                            e: E::Array(tuple_values),
                            start: self.value.start,
                            end: self.value.end,
                        }),
                        visitor,
                    )
                } else {
                    Err(Error::from_kind(
                        Some(self.value.start),
                        ErrorKind::ExpectedTuple(len),
                    ))
                }
            }
            e => Err(Error::from_kind(
                Some(self.value.start),
                ErrorKind::Wanted {
                    expected: "table",
                    found: e.type_name(),
                },
            )),
        }
    }

    fn struct_variant<V>(
        self,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        de::Deserializer::deserialize_struct(
            ValueDeserializer::new(self.value).with_struct_key_validation(),
            "", // TODO: this should be the variant name
            fields,
            visitor,
        )
    }
}

impl<'a> Deserializer<'a> {
    fn new(input: &'a str) -> Deserializer<'a> {
        Deserializer {
            tokens: Tokenizer::new(input),
            input,
        }
    }

    fn tables(&mut self) -> Result<Vec<Table<'a>>, Box<Error>> {
        let mut tables = Vec::new();
        let mut cur_table = Table {
            at: 0,
            header: Vec::new(),
            values: None,
            array: false,
        };

        while let Some(line) = self.line()? {
            match line {
                Line::Table {
                    at,
                    mut header,
                    array,
                } => {
                    if !cur_table.header.is_empty() || cur_table.values.is_some() {
                        tables.push(cur_table);
                    }
                    cur_table = Table {
                        at,
                        header: Vec::new(),
                        values: Some(Vec::new()),
                        array,
                    };
                    loop {
                        let part = header.next().map_err(|e| self.token_error(e));
                        match part? {
                            Some(part) => cur_table.header.push(part),
                            None => break,
                        }
                    }
                }
                Line::KeyValue(key, value) => {
                    if cur_table.values.is_none() {
                        cur_table.values = Some(Vec::new());
                    }
                    self.add_dotted_key(key, value, cur_table.values.as_mut().unwrap())?;
                }
            }
        }
        if !cur_table.header.is_empty() || cur_table.values.is_some() {
            tables.push(cur_table);
        }
        Ok(tables)
    }

    fn line(&mut self) -> Result<Option<Line<'a>>, Box<Error>> {
        loop {
            self.eat_whitespace();
            if self.eat_comment()? {
                continue;
            }
            if self.eat(Token::Newline)? {
                continue;
            }
            break;
        }

        match self.peek()? {
            Some((_, Token::LeftBracket)) => self.table_header().map(Some),
            Some(_) => self.key_value().map(Some),
            None => Ok(None),
        }
    }

    fn table_header(&mut self) -> Result<Line<'a>, Box<Error>> {
        let start = self.tokens.current();
        self.expect(Token::LeftBracket)?;
        let array = self.eat(Token::LeftBracket)?;
        let ret = Header::new(self.tokens.clone(), array);
        self.tokens.skip_to_newline();
        Ok(Line::Table {
            at: start,
            header: ret,
            array,
        })
    }

    fn key_value(&mut self) -> Result<Line<'a>, Box<Error>> {
        let key = self.dotted_key()?;
        self.eat_whitespace();
        self.expect(Token::Equals)?;
        self.eat_whitespace();

        let value = self.value()?;
        self.eat_whitespace();
        if !self.eat_comment()? {
            self.eat_newline_or_eof()?;
        }

        Ok(Line::KeyValue(key, value))
    }

    fn value(&mut self) -> Result<Value<'a>, Box<Error>> {
        let at = self.tokens.current();
        let value = match self.next()? {
            Some((Span { start, end }, Token::String { val, .. })) => Value {
                e: E::String(val),
                start,
                end,
            },
            Some((Span { start, end }, Token::Keylike("true"))) => Value {
                e: E::Boolean(true),
                start,
                end,
            },
            Some((Span { start, end }, Token::Keylike("false"))) => Value {
                e: E::Boolean(false),
                start,
                end,
            },
            Some((span, Token::Keylike(key))) => self.parse_keylike(at, span, key)?,
            Some((span, Token::Plus)) => self.number_leading_plus(span)?,
            Some((Span { start, .. }, Token::LeftBrace)) => {
                self.inline_table().map(|(Span { end, .. }, table)| Value {
                    e: E::InlineTable(table),
                    start,
                    end,
                })?
            }
            Some((Span { start, .. }, Token::LeftBracket)) => {
                self.array().map(|(Span { end, .. }, array)| Value {
                    e: E::Array(array),
                    start,
                    end,
                })?
            }
            Some(token) => {
                return Err(self.error(
                    at,
                    ErrorKind::Wanted {
                        expected: "a value",
                        found: token.1.describe(),
                    },
                ));
            }
            None => return Err(self.eof()),
        };
        Ok(value)
    }

    fn parse_keylike(
        &mut self,
        at: usize,
        span: Span,
        key: &'a str,
    ) -> Result<Value<'a>, Box<Error>> {
        if key == "inf" || key == "nan" {
            return self.number(span, key);
        }

        let first_char = key.chars().next().expect("key should not be empty here");
        match first_char {
            '-' | '0'..='9' => self.number(span, key),
            _ => Err(self.error(at, ErrorKind::UnquotedString)),
        }
    }

    fn number(&mut self, Span { start, end }: Span, s: &'a str) -> Result<Value<'a>, Box<Error>> {
        let to_integer = |f| Value {
            e: E::Integer(f),
            start,
            end,
        };
        if let Some(s) = s.strip_prefix("0x") {
            self.integer(s, 16).map(to_integer)
        } else if let Some(s) = s.strip_prefix("0o") {
            self.integer(s, 8).map(to_integer)
        } else if let Some(s) = s.strip_prefix("0b") {
            self.integer(s, 2).map(to_integer)
        } else if s.contains('e') || s.contains('E') {
            self.float(s, None).map(|f| Value {
                e: E::Float(f),
                start,
                end,
            })
        } else if self.eat(Token::Period)? {
            let at = self.tokens.current();
            match self.next()? {
                Some((Span { start, end }, Token::Keylike(after))) => {
                    self.float(s, Some(after)).map(|f| Value {
                        e: E::Float(f),
                        start,
                        end,
                    })
                }
                _ => Err(self.error(at, ErrorKind::NumberInvalid)),
            }
        } else if s == "inf" {
            Ok(Value {
                e: E::Float(f64::INFINITY),
                start,
                end,
            })
        } else if s == "-inf" {
            Ok(Value {
                e: E::Float(f64::NEG_INFINITY),
                start,
                end,
            })
        } else if s == "nan" {
            Ok(Value {
                e: E::Float(f64::NAN.copysign(1.0)),
                start,
                end,
            })
        } else if s == "-nan" {
            Ok(Value {
                e: E::Float(f64::NAN.copysign(-1.0)),
                start,
                end,
            })
        } else {
            self.integer(s, 10).map(to_integer)
        }
    }

    fn number_leading_plus(&mut self, Span { start, .. }: Span) -> Result<Value<'a>, Box<Error>> {
        let start_token = self.tokens.current();
        match self.next()? {
            Some((Span { end, .. }, Token::Keylike(s))) => self.number(Span { start, end }, s),
            _ => Err(self.error(start_token, ErrorKind::NumberInvalid)),
        }
    }

    fn integer(&self, s: &'a str, radix: u32) -> Result<i64, Box<Error>> {
        let allow_sign = radix == 10;
        let allow_leading_zeros = radix != 10;
        let (prefix, suffix) = self.parse_integer(s, allow_sign, allow_leading_zeros, radix)?;
        let start = self.tokens.substr_offset(s);
        if !suffix.is_empty() {
            return Err(self.error(start, ErrorKind::NumberInvalid));
        }
        i64::from_str_radix(prefix.replace('_', "").trim_start_matches('+'), radix)
            .map_err(|_e| self.error(start, ErrorKind::NumberInvalid))
    }

    fn parse_integer(
        &self,
        s: &'a str,
        allow_sign: bool,
        allow_leading_zeros: bool,
        radix: u32,
    ) -> Result<(&'a str, &'a str), Box<Error>> {
        let start = self.tokens.substr_offset(s);

        let mut first = true;
        let mut first_zero = false;
        let mut underscore = false;
        let mut end = s.len();
        for (i, c) in s.char_indices() {
            let at = i + start;
            if i == 0 && (c == '+' || c == '-') && allow_sign {
                continue;
            }

            if c == '0' && first {
                first_zero = true;
            } else if c.is_digit(radix) {
                if !first && first_zero && !allow_leading_zeros {
                    return Err(self.error(at, ErrorKind::NumberInvalid));
                }
                underscore = false;
            } else if c == '_' && first {
                return Err(self.error(at, ErrorKind::NumberInvalid));
            } else if c == '_' && !underscore {
                underscore = true;
            } else {
                end = i;
                break;
            }
            first = false;
        }
        if first || underscore {
            return Err(self.error(start, ErrorKind::NumberInvalid));
        }
        Ok((&s[..end], &s[end..]))
    }

    fn float(&mut self, s: &'a str, after_decimal: Option<&'a str>) -> Result<f64, Box<Error>> {
        let (integral, mut suffix) = self.parse_integer(s, true, false, 10)?;
        let start = self.tokens.substr_offset(integral);

        let mut fraction = None;
        if let Some(after) = after_decimal {
            if !suffix.is_empty() {
                return Err(self.error(start, ErrorKind::NumberInvalid));
            }
            let (a, b) = self.parse_integer(after, false, true, 10)?;
            fraction = Some(a);
            suffix = b;
        }

        let mut exponent = None;
        if suffix.starts_with('e') || suffix.starts_with('E') {
            let (a, b) = if suffix.len() == 1 {
                self.eat(Token::Plus)?;
                match self.next()? {
                    Some((_, Token::Keylike(s))) => self.parse_integer(s, false, true, 10)?,
                    _ => return Err(self.error(start, ErrorKind::NumberInvalid)),
                }
            } else {
                self.parse_integer(&suffix[1..], true, true, 10)?
            };
            if !b.is_empty() {
                return Err(self.error(start, ErrorKind::NumberInvalid));
            }
            exponent = Some(a);
        } else if !suffix.is_empty() {
            return Err(self.error(start, ErrorKind::NumberInvalid));
        }

        let mut number = integral
            .trim_start_matches('+')
            .chars()
            .filter(|c| *c != '_')
            .collect::<String>();
        if let Some(fraction) = fraction {
            number.push('.');
            number.extend(fraction.chars().filter(|c| *c != '_'));
        }
        if let Some(exponent) = exponent {
            number.push('E');
            number.extend(exponent.chars().filter(|c| *c != '_'));
        }
        number
            .parse()
            .map_err(|_e| self.error(start, ErrorKind::NumberInvalid))
            .and_then(|n: f64| {
                if n.is_finite() {
                    Ok(n)
                } else {
                    Err(self.error(start, ErrorKind::NumberInvalid))
                }
            })
    }

    // TODO(#140): shouldn't buffer up this entire table in memory, it'd be
    // great to defer parsing everything until later.
    fn inline_table(&mut self) -> Result<(Span, Vec<TablePair<'a>>), Box<Error>> {
        let mut ret = Vec::new();
        self.eat_whitespace();
        if let Some(span) = self.eat_spanned(Token::RightBrace)? {
            return Ok((span, ret));
        }
        loop {
            let key = self.dotted_key()?;
            self.eat_whitespace();
            self.expect(Token::Equals)?;
            self.eat_whitespace();
            let value = self.value()?;
            self.add_dotted_key(key, value, &mut ret)?;

            self.eat_whitespace();
            if let Some(span) = self.eat_spanned(Token::RightBrace)? {
                return Ok((span, ret));
            }
            self.expect(Token::Comma)?;
            self.eat_whitespace();
        }
    }

    // TODO(#140): shouldn't buffer up this entire array in memory, it'd be
    // great to defer parsing everything until later.
    fn array(&mut self) -> Result<(Span, Vec<Value<'a>>), Box<Error>> {
        let mut ret = Vec::new();

        let intermediate = |me: &mut Deserializer| -> Result<(), Box<Error>> {
            loop {
                me.eat_whitespace();
                if !me.eat(Token::Newline)? && !me.eat_comment()? {
                    break;
                }
            }
            Ok(())
        };

        loop {
            intermediate(self)?;
            if let Some(span) = self.eat_spanned(Token::RightBracket)? {
                return Ok((span, ret));
            }
            let value = self.value()?;
            ret.push(value);
            intermediate(self)?;
            if !self.eat(Token::Comma)? {
                break;
            }
        }
        intermediate(self)?;
        let span = self.expect_spanned(Token::RightBracket)?;
        Ok((span, ret))
    }

    fn table_key(&mut self) -> Result<(Span, Cow<'a, str>), Box<Error>> {
        self.tokens.table_key().map_err(|e| self.token_error(e))
    }

    fn dotted_key(&mut self) -> Result<Vec<(Span, Cow<'a, str>)>, Box<Error>> {
        let mut result = Vec::new();
        result.push(self.table_key()?);
        self.eat_whitespace();
        while self.eat(Token::Period)? {
            self.eat_whitespace();
            result.push(self.table_key()?);
            self.eat_whitespace();
        }
        Ok(result)
    }

    /// Stores a value in the appropriate hierarchical structure positioned based on the dotted key.
    ///
    /// Given the following definition: `multi.part.key = "value"`, `multi` and `part` are
    /// intermediate parts which are mapped to the relevant fields in the deserialized type's data
    /// hierarchy.
    ///
    /// # Parameters
    ///
    /// * `key_parts`: Each segment of the dotted key, e.g. `part.one` maps to
    ///   `vec![Cow::Borrowed("part"), Cow::Borrowed("one")].`
    /// * `value`: The parsed value.
    /// * `values`: The `Vec` to store the value in.
    fn add_dotted_key(
        &self,
        mut key_parts: Vec<(Span, Cow<'a, str>)>,
        value: Value<'a>,
        values: &mut Vec<TablePair<'a>>,
    ) -> Result<(), Box<Error>> {
        let key = key_parts.remove(0);
        if key_parts.is_empty() {
            values.push((key, value));
            return Ok(());
        }
        match values.iter_mut().find(|&&mut (ref k, _)| *k.1 == key.1) {
            Some(&mut (
                _,
                Value {
                    e: E::DottedTable(ref mut v),
                    ..
                },
            )) => {
                return self.add_dotted_key(key_parts, value, v);
            }
            Some(&mut (_, Value { start, .. })) => {
                return Err(self.error(start, ErrorKind::DottedKeyInvalidType));
            }
            None => {}
        }
        // The start/end value is somewhat misleading here.
        let table_values = Value {
            e: E::DottedTable(Vec::new()),
            start: value.start,
            end: value.end,
        };
        values.push((key, table_values));
        let last_i = values.len() - 1;
        if let (
            _,
            Value {
                e: E::DottedTable(ref mut v),
                ..
            },
        ) = values[last_i]
        {
            self.add_dotted_key(key_parts, value, v)?;
        }
        Ok(())
    }

    fn eat_whitespace(&mut self) {
        self.tokens.eat_whitespace();
    }

    fn eat_comment(&mut self) -> Result<bool, Box<Error>> {
        self.tokens.eat_comment().map_err(|e| self.token_error(e))
    }

    fn eat_newline_or_eof(&mut self) -> Result<(), Box<Error>> {
        self.tokens
            .eat_newline_or_eof()
            .map_err(|e| self.token_error(e))
    }

    fn eat(&mut self, expected: Token<'a>) -> Result<bool, Box<Error>> {
        self.tokens.eat(expected).map_err(|e| self.token_error(e))
    }

    fn eat_spanned(&mut self, expected: Token<'a>) -> Result<Option<Span>, Box<Error>> {
        self.tokens
            .eat_spanned(expected)
            .map_err(|e| self.token_error(e))
    }

    fn expect(&mut self, expected: Token<'a>) -> Result<(), Box<Error>> {
        self.tokens
            .expect(expected)
            .map_err(|e| self.token_error(e))
    }

    fn expect_spanned(&mut self, expected: Token<'a>) -> Result<Span, Box<Error>> {
        self.tokens
            .expect_spanned(expected)
            .map_err(|e| self.token_error(e))
    }

    fn next(&mut self) -> Result<Option<(Span, Token<'a>)>, Box<Error>> {
        self.tokens.next().map_err(|e| self.token_error(e))
    }

    fn peek(&mut self) -> Result<Option<(Span, Token<'a>)>, Box<Error>> {
        self.tokens.peek().map_err(|e| self.token_error(e))
    }

    fn eof(&self) -> Box<Error> {
        self.error(self.input.len(), ErrorKind::UnexpectedEof)
    }

    fn token_error(&self, error: TokenError) -> Box<Error> {
        match error {
            TokenError::InvalidCharInString(at, ch) => {
                self.error(at, ErrorKind::InvalidCharInString(ch))
            }
            TokenError::InvalidEscape(at, ch) => self.error(at, ErrorKind::InvalidEscape(ch)),
            TokenError::InvalidEscapeValue(at, v) => {
                self.error(at, ErrorKind::InvalidEscapeValue(v))
            }
            TokenError::InvalidHexEscape(at, ch) => self.error(at, ErrorKind::InvalidHexEscape(ch)),
            TokenError::NewlineInString(at) => self.error(at, ErrorKind::NewlineInString),
            TokenError::Unexpected(at, ch) => self.error(at, ErrorKind::Unexpected(ch)),
            TokenError::UnterminatedString(at) => self.error(at, ErrorKind::UnterminatedString),
            TokenError::NewlineInTableKey(at) => self.error(at, ErrorKind::NewlineInTableKey),
            TokenError::Wanted {
                at,
                expected,
                found,
            } => self.error(at, ErrorKind::Wanted { expected, found }),
            TokenError::MultilineStringKey(at) => self.error(at, ErrorKind::MultilineStringKey),
        }
    }

    fn error(&self, at: usize, kind: ErrorKind) -> Box<Error> {
        let mut err = Error::from_kind(Some(at), kind);
        err.fix_linecol(|at| self.to_linecol(at));
        err
    }

    /// Converts a byte offset from an error message to a (line, column) pair
    ///
    /// All indexes are 0-based.
    fn to_linecol(&self, offset: usize) -> (usize, usize) {
        let mut cur = 0;
        // Use split_terminator instead of lines so that if there is a `\r`, it
        // is included in the offset calculation. The `+1` values below account
        // for the `\n`.
        for (i, line) in self.input.split_terminator('\n').enumerate() {
            if cur + line.len() + 1 > offset {
                return (i, offset - cur);
            }
            cur += line.len() + 1;
        }
        (self.input.lines().count(), 0)
    }
}

impl Error {
    pub(crate) fn line_col(&self) -> Option<(usize, usize)> {
        self.line.map(|line| (line, self.col))
    }

    fn from_kind(at: Option<usize>, kind: ErrorKind) -> Box<Self> {
        Box::new(Error {
            kind,
            line: None,
            col: 0,
            at,
            message: String::new(),
            key: Vec::new(),
        })
    }

    fn custom(at: Option<usize>, s: String) -> Box<Self> {
        Box::new(Error {
            kind: ErrorKind::Custom,
            line: None,
            col: 0,
            at,
            message: s,
            key: Vec::new(),
        })
    }

    pub(crate) fn add_key_context(&mut self, key: &str) {
        self.key.insert(0, key.to_string());
    }

    fn fix_offset<F>(&mut self, f: F)
    where
        F: FnOnce() -> Option<usize>,
    {
        // An existing offset is always better positioned than anything we might
        // want to add later.
        if self.at.is_none() {
            self.at = f();
        }
    }

    fn fix_linecol<F>(&mut self, f: F)
    where
        F: FnOnce(usize) -> (usize, usize),
    {
        if let Some(at) = self.at {
            let (line, col) = f(at);
            self.line = Some(line);
            self.col = col;
        }
    }
}

impl std::convert::From<Error> for std::io::Error {
    fn from(e: Error) -> Self {
        std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string())
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match &self.kind {
            ErrorKind::UnexpectedEof => "unexpected eof encountered".fmt(f)?,
            ErrorKind::InvalidCharInString(c) => write!(
                f,
                "invalid character in string: `{}`",
                c.escape_default().collect::<String>()
            )?,
            ErrorKind::InvalidEscape(c) => write!(
                f,
                "invalid escape character in string: `{}`",
                c.escape_default().collect::<String>()
            )?,
            ErrorKind::InvalidHexEscape(c) => write!(
                f,
                "invalid hex escape character in string: `{}`",
                c.escape_default().collect::<String>()
            )?,
            ErrorKind::InvalidEscapeValue(c) => write!(f, "invalid escape value: `{}`", c)?,
            ErrorKind::NewlineInString => "newline in string found".fmt(f)?,
            ErrorKind::Unexpected(ch) => write!(
                f,
                "unexpected character found: `{}`",
                ch.escape_default().collect::<String>()
            )?,
            ErrorKind::UnterminatedString => "unterminated string".fmt(f)?,
            ErrorKind::NewlineInTableKey => "found newline in table key".fmt(f)?,
            ErrorKind::Wanted { expected, found } => {
                write!(f, "expected {}, found {}", expected, found)?;
            }
            ErrorKind::NumberInvalid => "invalid number".fmt(f)?,
            ErrorKind::DuplicateTable(ref s) => {
                write!(f, "redefinition of table `{}`", s)?;
            }
            ErrorKind::DuplicateKey(ref s) => {
                write!(f, "duplicate key: `{}`", s)?;
            }
            ErrorKind::RedefineAsArray => "table redefined as array".fmt(f)?,
            ErrorKind::MultilineStringKey => "multiline strings are not allowed for key".fmt(f)?,
            ErrorKind::Custom => self.message.fmt(f)?,
            ErrorKind::ExpectedTuple(l) => write!(f, "expected table with length {}", l)?,
            ErrorKind::ExpectedTupleIndex {
                expected,
                ref found,
            } => write!(f, "expected table key `{}`, but was `{}`", expected, found)?,
            ErrorKind::ExpectedEmptyTable => "expected empty table".fmt(f)?,
            ErrorKind::DottedKeyInvalidType => {
                "dotted key attempted to extend non-table type".fmt(f)?;
            }
            ErrorKind::UnexpectedKeys {
                ref keys,
                available,
            } => write!(
                f,
                "unexpected keys in table: `{:?}`, available keys: `{:?}`",
                keys, available
            )?,
            ErrorKind::UnquotedString => write!(
                f,
                "invalid TOML value, did you mean to use a quoted string?"
            )?,
        }

        if !self.key.is_empty() {
            write!(f, " for key `")?;
            for (i, k) in self.key.iter().enumerate() {
                if i > 0 {
                    write!(f, ".")?;
                }
                write!(f, "{}", k)?;
            }
            write!(f, "`")?;
        }

        if let Some(line) = self.line {
            write!(f, " at line {} column {}", line + 1, self.col + 1)?;
        }

        Ok(())
    }
}

impl error::Error for Error {}

impl de::Error for Box<Error> {
    fn custom<T: Display>(msg: T) -> Self {
        Error::custom(None, msg.to_string())
    }
}

enum Line<'a> {
    Table {
        at: usize,
        header: Header<'a>,
        array: bool,
    },
    KeyValue(Vec<(Span, Cow<'a, str>)>, Value<'a>),
}

struct Header<'a> {
    first: bool,
    array: bool,
    tokens: Tokenizer<'a>,
}

impl<'a> Header<'a> {
    fn new(tokens: Tokenizer<'a>, array: bool) -> Header<'a> {
        Header {
            first: true,
            array,
            tokens,
        }
    }

    fn next(&mut self) -> Result<Option<(Span, Cow<'a, str>)>, TokenError> {
        self.tokens.eat_whitespace();

        if self.first || self.tokens.eat(Token::Period)? {
            self.first = false;
            self.tokens.eat_whitespace();
            self.tokens.table_key().map(Some)
        } else {
            self.tokens.expect(Token::RightBracket)?;
            if self.array {
                self.tokens.expect(Token::RightBracket)?;
            }

            self.tokens.eat_whitespace();
            if !self.tokens.eat_comment()? {
                self.tokens.eat_newline_or_eof()?;
            }
            Ok(None)
        }
    }
}

#[derive(Debug)]
struct Value<'a> {
    e: E<'a>,
    start: usize,
    end: usize,
}

#[derive(Debug)]
enum E<'a> {
    Integer(i64),
    Float(f64),
    Boolean(bool),
    String(Cow<'a, str>),
    Array(Vec<Value<'a>>),
    InlineTable(Vec<TablePair<'a>>),
    DottedTable(Vec<TablePair<'a>>),
}

impl<'a> E<'a> {
    fn type_name(&self) -> &'static str {
        match *self {
            E::String(..) => "string",
            E::Integer(..) => "integer",
            E::Float(..) => "float",
            E::Boolean(..) => "boolean",
            E::Array(..) => "array",
            E::InlineTable(..) => "inline table",
            E::DottedTable(..) => "dotted table",
        }
    }
}
