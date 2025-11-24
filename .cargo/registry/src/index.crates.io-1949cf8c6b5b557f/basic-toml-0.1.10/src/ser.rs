use serde::ser::{self, Serialize};
use std::cell::Cell;
use std::error;
use std::fmt::{self, Display, Write};

/// Serialize the given data structure as a String of TOML.
///
/// Serialization can fail if `T`'s implementation of `Serialize` decides to
/// fail, if `T` contains a map with non-string keys, or if `T` attempts to
/// serialize an unsupported datatype such as an enum, tuple, or tuple struct.
pub fn to_string<T>(value: &T) -> Result<String, crate::Error>
where
    T: ?Sized + Serialize,
{
    let mut dst = String::with_capacity(128);
    value.serialize(&mut Serializer::new(&mut dst))?;
    Ok(dst)
}

#[derive(Debug)]
pub(crate) enum Error {
    /// Indicates that a Rust type was requested to be serialized but it was not
    /// supported.
    ///
    /// Currently the TOML format does not support serializing types such as
    /// enums, tuples and tuple structs.
    UnsupportedType,

    /// The key of all TOML maps must be strings, but serialization was
    /// attempted where the key of a map was not a string.
    KeyNotString,

    /// All values in a TOML table must be emitted before further tables are
    /// emitted. If a value is emitted *after* a table then this error is
    /// generated.
    ValueAfterTable,

    /// None was attempted to be serialized, but it's not supported.
    UnsupportedNone,

    /// A custom error which could be generated when serializing a particular
    /// type.
    Custom(String),
}

struct Serializer<'a> {
    dst: &'a mut String,
    state: State<'a>,
}

#[derive(Debug, Copy, Clone)]
enum ArrayState {
    Started,
    StartedAsATable,
}

#[derive(Debug, Clone)]
enum State<'a> {
    Table {
        key: &'a str,
        parent: &'a State<'a>,
        first: &'a Cell<bool>,
        table_emitted: &'a Cell<bool>,
    },
    Array {
        parent: &'a State<'a>,
        first: &'a Cell<bool>,
        type_: &'a Cell<Option<ArrayState>>,
        len: Option<usize>,
    },
    End,
}

struct SerializeSeq<'a, 'b> {
    ser: &'b mut Serializer<'a>,
    first: Cell<bool>,
    type_: Cell<Option<ArrayState>>,
    len: Option<usize>,
}

struct SerializeTable<'a, 'b> {
    ser: &'b mut Serializer<'a>,
    key: String,
    first: Cell<bool>,
    table_emitted: Cell<bool>,
}

impl<'a> Serializer<'a> {
    fn new(dst: &'a mut String) -> Serializer<'a> {
        Serializer {
            dst,
            state: State::End,
        }
    }

    fn display<T: Display>(&mut self, t: T, type_: ArrayState) -> Result<(), Error> {
        self.emit_key(type_)?;
        write!(self.dst, "{}", t).map_err(ser::Error::custom)?;
        if let State::Table { .. } = self.state {
            self.dst.push('\n');
        }
        Ok(())
    }

    fn emit_key(&mut self, type_: ArrayState) -> Result<(), Error> {
        self.array_type(type_);
        let state = self.state.clone();
        self.do_emit_key(&state)
    }

    // recursive implementation of `emit_key` above
    fn do_emit_key(&mut self, state: &State) -> Result<(), Error> {
        match *state {
            State::End => Ok(()),
            State::Array {
                parent,
                first,
                type_,
                len,
            } => {
                assert!(type_.get().is_some());
                if first.get() {
                    self.do_emit_key(parent)?;
                }
                self.emit_array(first, len);
                Ok(())
            }
            State::Table {
                parent,
                first,
                table_emitted,
                key,
            } => {
                if table_emitted.get() {
                    return Err(Error::ValueAfterTable);
                }
                if first.get() {
                    self.emit_table_header(parent)?;
                    first.set(false);
                }
                self.escape_key(key)?;
                self.dst.push_str(" = ");
                Ok(())
            }
        }
    }

    fn emit_array(&mut self, first: &Cell<bool>, _len: Option<usize>) {
        if first.get() {
            self.dst.push('[');
        } else {
            self.dst.push_str(", ");
        }
    }

    fn array_type(&mut self, type_: ArrayState) {
        let prev = match self.state {
            State::Array { type_, .. } => type_,
            _ => return,
        };
        if prev.get().is_none() {
            prev.set(Some(type_));
        }
    }

    fn escape_key(&mut self, key: &str) -> Result<(), Error> {
        let ok = !key.is_empty()
            && key.chars().all(|c| match c {
                'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' => true,
                _ => false,
            });
        if ok {
            write!(self.dst, "{}", key).map_err(ser::Error::custom)?;
        } else {
            self.emit_str(key)?;
        }
        Ok(())
    }

    fn emit_str(&mut self, value: &str) -> Result<(), Error> {
        self.dst.push('"');
        for ch in value.chars() {
            match ch {
                '\u{8}' => self.dst.push_str("\\b"),
                '\u{9}' => self.dst.push_str("\\t"),
                '\u{a}' => self.dst.push_str("\\n"),
                '\u{c}' => self.dst.push_str("\\f"),
                '\u{d}' => self.dst.push_str("\\r"),
                '\u{22}' => self.dst.push_str("\\\""),
                '\u{5c}' => self.dst.push_str("\\\\"),
                c if c <= '\u{1f}' || c == '\u{7f}' => {
                    write!(self.dst, "\\u{:04X}", ch as u32).map_err(ser::Error::custom)?;
                }
                ch => self.dst.push(ch),
            }
        }
        self.dst.push('"');
        Ok(())
    }

    fn emit_table_header(&mut self, state: &State) -> Result<(), Error> {
        let array_of_tables = match *state {
            State::End => return Ok(()),
            State::Array { .. } => true,
            State::Table { .. } => false,
        };

        // Unlike [..]s, we can't omit [[..]] ancestors, so be sure to emit
        // table headers for them.
        let mut p = state;
        if let State::Array { first, parent, .. } = *state {
            if first.get() {
                p = parent;
            }
        }
        while let State::Table { first, parent, .. } = *p {
            p = parent;
            if !first.get() {
                break;
            }
            if let State::Array {
                parent: &State::Table { .. },
                ..
            } = *parent
            {
                self.emit_table_header(parent)?;
                break;
            }
        }

        match *state {
            State::Table { first, .. } => {
                if !first.get() {
                    // Newline if we are a table that is not the first table in
                    // the document.
                    self.dst.push('\n');
                }
            }
            State::Array { parent, first, .. } => {
                if !first.get() {
                    // Always newline if we are not the first item in the
                    // table-array
                    self.dst.push('\n');
                } else if let State::Table { first, .. } = *parent {
                    if !first.get() {
                        // Newline if we are not the first item in the document
                        self.dst.push('\n');
                    }
                }
            }
            State::End => {}
        }
        self.dst.push('[');
        if array_of_tables {
            self.dst.push('[');
        }
        self.emit_key_part(state)?;
        if array_of_tables {
            self.dst.push(']');
        }
        self.dst.push_str("]\n");
        Ok(())
    }

    fn emit_key_part(&mut self, key: &State) -> Result<bool, Error> {
        match *key {
            State::Array { parent, .. } => self.emit_key_part(parent),
            State::End => Ok(true),
            State::Table {
                key,
                parent,
                table_emitted,
                ..
            } => {
                table_emitted.set(true);
                let first = self.emit_key_part(parent)?;
                if !first {
                    self.dst.push('.');
                }
                self.escape_key(key)?;
                Ok(false)
            }
        }
    }
}

macro_rules! serialize_float {
    ($this:expr, $v:expr) => {{
        $this.emit_key(ArrayState::Started)?;
        match ($v.is_sign_negative(), $v.is_nan(), $v == 0.0) {
            (_, true, _) => write!($this.dst, "nan"),
            (true, false, true) => write!($this.dst, "-0.0"),
            (false, false, true) => write!($this.dst, "0.0"),
            (_, false, false) => write!($this.dst, "{}", $v).and_then(|()| {
                if $v % 1.0 == 0.0 {
                    write!($this.dst, ".0")
                } else {
                    Ok(())
                }
            }),
        }
        .map_err(ser::Error::custom)?;

        if let State::Table { .. } = $this.state {
            $this.dst.push_str("\n");
        }
        return Ok(());
    }};
}

impl<'a, 'b> ser::Serializer for &'b mut Serializer<'a> {
    type Ok = ();
    type Error = Error;
    type SerializeSeq = SerializeSeq<'a, 'b>;
    type SerializeTuple = SerializeSeq<'a, 'b>;
    type SerializeTupleStruct = SerializeSeq<'a, 'b>;
    type SerializeTupleVariant = ser::Impossible<(), Error>;
    type SerializeMap = SerializeTable<'a, 'b>;
    type SerializeStruct = SerializeTable<'a, 'b>;
    type SerializeStructVariant = ser::Impossible<(), Error>;

    fn serialize_bool(self, v: bool) -> Result<(), Self::Error> {
        self.display(v, ArrayState::Started)
    }

    fn serialize_i8(self, v: i8) -> Result<(), Self::Error> {
        self.display(v, ArrayState::Started)
    }

    fn serialize_i16(self, v: i16) -> Result<(), Self::Error> {
        self.display(v, ArrayState::Started)
    }

    fn serialize_i32(self, v: i32) -> Result<(), Self::Error> {
        self.display(v, ArrayState::Started)
    }

    fn serialize_i64(self, v: i64) -> Result<(), Self::Error> {
        self.display(v, ArrayState::Started)
    }

    fn serialize_u8(self, v: u8) -> Result<(), Self::Error> {
        self.display(v, ArrayState::Started)
    }

    fn serialize_u16(self, v: u16) -> Result<(), Self::Error> {
        self.display(v, ArrayState::Started)
    }

    fn serialize_u32(self, v: u32) -> Result<(), Self::Error> {
        self.display(v, ArrayState::Started)
    }

    fn serialize_u64(self, v: u64) -> Result<(), Self::Error> {
        self.display(v, ArrayState::Started)
    }

    fn serialize_f32(self, v: f32) -> Result<(), Self::Error> {
        serialize_float!(self, v)
    }

    fn serialize_f64(self, v: f64) -> Result<(), Self::Error> {
        serialize_float!(self, v)
    }

    fn serialize_char(self, v: char) -> Result<(), Self::Error> {
        let mut buf = [0; 4];
        self.serialize_str(v.encode_utf8(&mut buf))
    }

    fn serialize_str(self, value: &str) -> Result<(), Self::Error> {
        self.emit_key(ArrayState::Started)?;
        self.emit_str(value)?;
        if let State::Table { .. } = self.state {
            self.dst.push('\n');
        }
        Ok(())
    }

    fn serialize_bytes(self, value: &[u8]) -> Result<(), Self::Error> {
        value.serialize(self)
    }

    fn serialize_none(self) -> Result<(), Self::Error> {
        Err(Error::UnsupportedNone)
    }

    fn serialize_some<T>(self, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<(), Self::Error> {
        Err(Error::UnsupportedType)
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<(), Self::Error> {
        Err(Error::UnsupportedType)
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<(), Self::Error> {
        self.serialize_str(variant)
    }

    fn serialize_newtype_struct<T>(self, _name: &'static str, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T>(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _value: &T,
    ) -> Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        Err(Error::UnsupportedType)
    }

    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        self.array_type(ArrayState::Started);
        Ok(SerializeSeq {
            ser: self,
            first: Cell::new(true),
            type_: Cell::new(None),
            len,
        })
    }

    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        self.serialize_seq(Some(len))
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        self.serialize_seq(Some(len))
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        Err(Error::UnsupportedType)
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        self.array_type(ArrayState::StartedAsATable);
        Ok(SerializeTable {
            ser: self,
            key: String::new(),
            first: Cell::new(true),
            table_emitted: Cell::new(false),
        })
    }

    fn serialize_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        self.array_type(ArrayState::StartedAsATable);
        Ok(SerializeTable {
            ser: self,
            key: String::new(),
            first: Cell::new(true),
            table_emitted: Cell::new(false),
        })
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        Err(Error::UnsupportedType)
    }
}

impl<'a, 'b> ser::SerializeSeq for SerializeSeq<'a, 'b> {
    type Ok = ();
    type Error = Error;

    fn serialize_element<T>(&mut self, value: &T) -> Result<(), Error>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(&mut Serializer {
            dst: &mut *self.ser.dst,
            state: State::Array {
                parent: &self.ser.state,
                first: &self.first,
                type_: &self.type_,
                len: self.len,
            },
        })?;
        self.first.set(false);
        Ok(())
    }

    fn end(self) -> Result<(), Error> {
        match self.type_.get() {
            Some(ArrayState::StartedAsATable) => return Ok(()),
            Some(ArrayState::Started) => self.ser.dst.push(']'),
            None => {
                assert!(self.first.get());
                self.ser.emit_key(ArrayState::Started)?;
                self.ser.dst.push_str("[]");
            }
        }
        if let State::Table { .. } = self.ser.state {
            self.ser.dst.push('\n');
        }
        Ok(())
    }
}

impl<'a, 'b> ser::SerializeTuple for SerializeSeq<'a, 'b> {
    type Ok = ();
    type Error = Error;

    fn serialize_element<T>(&mut self, value: &T) -> Result<(), Error>
    where
        T: ?Sized + Serialize,
    {
        ser::SerializeSeq::serialize_element(self, value)
    }

    fn end(self) -> Result<(), Error> {
        ser::SerializeSeq::end(self)
    }
}

impl<'a, 'b> ser::SerializeTupleStruct for SerializeSeq<'a, 'b> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, value: &T) -> Result<(), Error>
    where
        T: ?Sized + Serialize,
    {
        ser::SerializeSeq::serialize_element(self, value)
    }

    fn end(self) -> Result<(), Error> {
        ser::SerializeSeq::end(self)
    }
}

impl<'a, 'b> ser::SerializeMap for SerializeTable<'a, 'b> {
    type Ok = ();
    type Error = Error;

    fn serialize_key<T>(&mut self, input: &T) -> Result<(), Error>
    where
        T: ?Sized + Serialize,
    {
        self.key = input.serialize(StringExtractor)?;
        Ok(())
    }

    fn serialize_value<T>(&mut self, value: &T) -> Result<(), Error>
    where
        T: ?Sized + Serialize,
    {
        let res = value.serialize(&mut Serializer {
            dst: &mut *self.ser.dst,
            state: State::Table {
                key: &self.key,
                parent: &self.ser.state,
                first: &self.first,
                table_emitted: &self.table_emitted,
            },
        });
        match res {
            Ok(()) => self.first.set(false),
            Err(Error::UnsupportedNone) => {}
            Err(e) => return Err(e),
        }
        Ok(())
    }

    fn end(self) -> Result<(), Error> {
        if self.first.get() {
            let state = self.ser.state.clone();
            self.ser.emit_table_header(&state)?;
        }
        Ok(())
    }
}

impl<'a, 'b> ser::SerializeStruct for SerializeTable<'a, 'b> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<(), Error>
    where
        T: ?Sized + Serialize,
    {
        let res = value.serialize(&mut Serializer {
            dst: &mut *self.ser.dst,
            state: State::Table {
                key,
                parent: &self.ser.state,
                first: &self.first,
                table_emitted: &self.table_emitted,
            },
        });
        match res {
            Ok(()) => self.first.set(false),
            Err(Error::UnsupportedNone) => {}
            Err(e) => return Err(e),
        }
        Ok(())
    }

    fn end(self) -> Result<(), Error> {
        if self.first.get() {
            let state = self.ser.state.clone();
            self.ser.emit_table_header(&state)?;
        }
        Ok(())
    }
}

struct StringExtractor;

impl ser::Serializer for StringExtractor {
    type Ok = String;
    type Error = Error;
    type SerializeSeq = ser::Impossible<String, Error>;
    type SerializeTuple = ser::Impossible<String, Error>;
    type SerializeTupleStruct = ser::Impossible<String, Error>;
    type SerializeTupleVariant = ser::Impossible<String, Error>;
    type SerializeMap = ser::Impossible<String, Error>;
    type SerializeStruct = ser::Impossible<String, Error>;
    type SerializeStructVariant = ser::Impossible<String, Error>;

    fn serialize_bool(self, _v: bool) -> Result<String, Self::Error> {
        Err(Error::KeyNotString)
    }

    fn serialize_i8(self, _v: i8) -> Result<String, Self::Error> {
        Err(Error::KeyNotString)
    }

    fn serialize_i16(self, _v: i16) -> Result<String, Self::Error> {
        Err(Error::KeyNotString)
    }

    fn serialize_i32(self, _v: i32) -> Result<String, Self::Error> {
        Err(Error::KeyNotString)
    }

    fn serialize_i64(self, _v: i64) -> Result<String, Self::Error> {
        Err(Error::KeyNotString)
    }

    fn serialize_u8(self, _v: u8) -> Result<String, Self::Error> {
        Err(Error::KeyNotString)
    }

    fn serialize_u16(self, _v: u16) -> Result<String, Self::Error> {
        Err(Error::KeyNotString)
    }

    fn serialize_u32(self, _v: u32) -> Result<String, Self::Error> {
        Err(Error::KeyNotString)
    }

    fn serialize_u64(self, _v: u64) -> Result<String, Self::Error> {
        Err(Error::KeyNotString)
    }

    fn serialize_f32(self, _v: f32) -> Result<String, Self::Error> {
        Err(Error::KeyNotString)
    }

    fn serialize_f64(self, _v: f64) -> Result<String, Self::Error> {
        Err(Error::KeyNotString)
    }

    fn serialize_char(self, _v: char) -> Result<String, Self::Error> {
        Err(Error::KeyNotString)
    }

    fn serialize_str(self, value: &str) -> Result<String, Self::Error> {
        Ok(value.to_string())
    }

    fn serialize_bytes(self, _value: &[u8]) -> Result<String, Self::Error> {
        Err(Error::KeyNotString)
    }

    fn serialize_none(self) -> Result<String, Self::Error> {
        Err(Error::KeyNotString)
    }

    fn serialize_some<T>(self, _value: &T) -> Result<String, Self::Error>
    where
        T: ?Sized + Serialize,
    {
        Err(Error::KeyNotString)
    }

    fn serialize_unit(self) -> Result<String, Self::Error> {
        Err(Error::KeyNotString)
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<String, Self::Error> {
        Err(Error::KeyNotString)
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
    ) -> Result<String, Self::Error> {
        Err(Error::KeyNotString)
    }

    fn serialize_newtype_struct<T>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<String, Self::Error>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T>(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _value: &T,
    ) -> Result<String, Self::Error>
    where
        T: ?Sized + Serialize,
    {
        Err(Error::KeyNotString)
    }

    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        Err(Error::KeyNotString)
    }

    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        Err(Error::KeyNotString)
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        Err(Error::KeyNotString)
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        Err(Error::KeyNotString)
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        Err(Error::KeyNotString)
    }

    fn serialize_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        Err(Error::KeyNotString)
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        Err(Error::KeyNotString)
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::UnsupportedType => "unsupported Rust type".fmt(f),
            Error::KeyNotString => "map key was not a string".fmt(f),
            Error::ValueAfterTable => "values must be emitted before tables".fmt(f),
            Error::UnsupportedNone => "unsupported None value".fmt(f),
            Error::Custom(ref s) => s.fmt(f),
        }
    }
}

impl error::Error for Error {}

impl ser::Error for Error {
    fn custom<T: Display>(msg: T) -> Error {
        Error::Custom(msg.to_string())
    }
}
