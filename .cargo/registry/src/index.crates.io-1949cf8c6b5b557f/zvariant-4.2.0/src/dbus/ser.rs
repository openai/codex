use serde::{ser, ser::SerializeSeq, Serialize};
use static_assertions::assert_impl_all;
use std::{
    io::{Seek, Write},
    str,
};

use crate::{
    container_depths::ContainerDepths,
    serialized::{Context, Format},
    signature_parser::SignatureParser,
    utils::*,
    Basic, Error, ObjectPath, Result, Signature, WriteBytes,
};

#[cfg(unix)]
use crate::Fd;

/// Our D-Bus serialization implementation.
pub(crate) struct Serializer<'ser, 'sig, W>(pub(crate) crate::SerializerCommon<'ser, 'sig, W>);

assert_impl_all!(Serializer<'_, '_, i32>: Send, Sync, Unpin);

impl<'ser, 'sig, W> Serializer<'ser, 'sig, W>
where
    W: Write + Seek,
{
    /// Create a D-Bus Serializer struct instance.
    ///
    /// On Windows, there is no `fds` argument.
    pub fn new<'w: 'ser, 'f: 'ser, S>(
        signature: S,
        writer: &'w mut W,
        #[cfg(unix)] fds: &'f mut crate::ser::FdList,
        ctxt: Context,
    ) -> Result<Self>
    where
        S: TryInto<Signature<'sig>>,
        S::Error: Into<Error>,
    {
        assert_eq!(ctxt.format(), Format::DBus);

        let signature = signature.try_into().map_err(Into::into)?;
        let sig_parser = SignatureParser::new(signature);
        Ok(Self(crate::SerializerCommon {
            ctxt,
            sig_parser,
            writer,
            #[cfg(unix)]
            fds,
            bytes_written: 0,
            value_sign: None,
            container_depths: Default::default(),
        }))
    }
}

macro_rules! serialize_basic {
    ($method:ident($type:ty) $write_method:ident) => {
        serialize_basic!($method($type) $write_method($type));
    };
    ($method:ident($type:ty) $write_method:ident($as:ty)) => {
        fn $method(self, v: $type) -> Result<()> {
            self.0.prep_serialize_basic::<$type>()?;
            self.0.$write_method(self.0.ctxt.endian(), v as $as).map_err(|e| Error::InputOutput(e.into()))
        }
    };
}

impl<'ser, 'sig, 'b, W> ser::Serializer for &'b mut Serializer<'ser, 'sig, W>
where
    W: Write + Seek,
{
    type Ok = ();
    type Error = Error;

    type SerializeSeq = SeqSerializer<'ser, 'sig, 'b, W>;
    type SerializeTuple = StructSeqSerializer<'ser, 'sig, 'b, W>;
    type SerializeTupleStruct = StructSeqSerializer<'ser, 'sig, 'b, W>;
    type SerializeTupleVariant = StructSeqSerializer<'ser, 'sig, 'b, W>;
    type SerializeMap = SeqSerializer<'ser, 'sig, 'b, W>;
    type SerializeStruct = StructSeqSerializer<'ser, 'sig, 'b, W>;
    type SerializeStructVariant = StructSeqSerializer<'ser, 'sig, 'b, W>;

    serialize_basic!(serialize_bool(bool) write_u32(u32));
    // No i8 type in D-Bus/GVariant, let's pretend it's i16
    serialize_basic!(serialize_i8(i8) write_i16(i16));
    serialize_basic!(serialize_i16(i16) write_i16);
    serialize_basic!(serialize_i64(i64) write_i64);

    fn serialize_i32(self, v: i32) -> Result<()> {
        match self.0.sig_parser.next_char()? {
            #[cfg(unix)]
            Fd::SIGNATURE_CHAR => {
                self.0.sig_parser.skip_char()?;
                self.0.add_padding(u32::alignment(Format::DBus))?;
                let idx = self.0.add_fd(v)?;
                self.0
                    .write_u32(self.0.ctxt.endian(), idx)
                    .map_err(|e| Error::InputOutput(e.into()))
            }
            _ => {
                self.0.prep_serialize_basic::<i32>()?;
                self.0
                    .write_i32(self.0.ctxt.endian(), v)
                    .map_err(|e| Error::InputOutput(e.into()))
            }
        }
    }

    fn serialize_u8(self, v: u8) -> Result<()> {
        self.0.prep_serialize_basic::<u8>()?;
        // Endianness is irrelevant for single bytes.
        self.0
            .write_u8(self.0.ctxt.endian(), v)
            .map_err(|e| Error::InputOutput(e.into()))
    }

    serialize_basic!(serialize_u16(u16) write_u16);
    serialize_basic!(serialize_u32(u32) write_u32);
    serialize_basic!(serialize_u64(u64) write_u64);
    // No f32 type in D-Bus/GVariant, let's pretend it's f64
    serialize_basic!(serialize_f32(f32) write_f64(f64));
    serialize_basic!(serialize_f64(f64) write_f64);

    fn serialize_char(self, v: char) -> Result<()> {
        // No char type in D-Bus, let's pretend it's a string
        self.serialize_str(&v.to_string())
    }

    fn serialize_str(self, v: &str) -> Result<()> {
        if v.contains('\0') {
            return Err(serde::de::Error::invalid_value(
                serde::de::Unexpected::Char('\0'),
                &"D-Bus string type must not contain interior null bytes",
            ));
        }
        let c = self.0.sig_parser.next_char()?;
        if c == VARIANT_SIGNATURE_CHAR {
            self.0.value_sign = Some(signature_string!(v));
        }

        match c {
            ObjectPath::SIGNATURE_CHAR | <&str>::SIGNATURE_CHAR => {
                self.0.add_padding(<&str>::alignment(Format::DBus))?;
                self.0
                    .write_u32(self.0.ctxt.endian(), usize_to_u32(v.len()))
                    .map_err(|e| Error::InputOutput(e.into()))?;
            }
            Signature::SIGNATURE_CHAR | VARIANT_SIGNATURE_CHAR => {
                self.0
                    .write_u8(self.0.ctxt.endian(), usize_to_u8(v.len()))
                    .map_err(|e| Error::InputOutput(e.into()))?;
            }
            _ => {
                let expected = format!(
                    "`{}`, `{}`, `{}` or `{}`",
                    <&str>::SIGNATURE_STR,
                    Signature::SIGNATURE_STR,
                    ObjectPath::SIGNATURE_STR,
                    VARIANT_SIGNATURE_CHAR,
                );
                return Err(serde::de::Error::invalid_type(
                    serde::de::Unexpected::Char(c),
                    &expected.as_str(),
                ));
            }
        }

        self.0.sig_parser.skip_char()?;
        self.0
            .write_all(v.as_bytes())
            .map_err(|e| Error::InputOutput(e.into()))?;
        self.0
            .write_all(&b"\0"[..])
            .map_err(|e| Error::InputOutput(e.into()))?;

        Ok(())
    }

    fn serialize_bytes(self, v: &[u8]) -> Result<()> {
        let seq = self.serialize_seq(Some(v.len()))?;
        seq.ser
            .0
            .write(v)
            .map_err(|e| Error::InputOutput(e.into()))?;
        seq.end()
    }

    fn serialize_none(self) -> Result<()> {
        #[cfg(feature = "option-as-array")]
        {
            let seq = self.serialize_seq(Some(0))?;
            seq.end()
        }

        #[cfg(not(feature = "option-as-array"))]
        unreachable!(
            "Can only encode Option<T> in D-Bus format if `option-as-array` feature is enabled",
        );
    }

    fn serialize_some<T>(self, #[allow(unused)] value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        #[cfg(feature = "option-as-array")]
        {
            let mut seq = self.serialize_seq(Some(1))?;
            seq.serialize_element(value)?;
            seq.end()
        }

        #[cfg(not(feature = "option-as-array"))]
        unreachable!(
            "Can only encode Option<T> in D-Bus format if `option-as-array` feature is enabled",
        );
    }

    fn serialize_unit(self) -> Result<()> {
        Ok(())
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<()> {
        self.serialize_unit()
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        variant_index: u32,
        variant: &'static str,
    ) -> Result<()> {
        if self.0.sig_parser.next_char()? == <&str>::SIGNATURE_CHAR {
            variant.serialize(self)
        } else {
            variant_index.serialize(self)
        }
    }

    fn serialize_newtype_struct<T>(self, _name: &'static str, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(self)?;

        Ok(())
    }

    fn serialize_newtype_variant<T>(
        self,
        _name: &'static str,
        variant_index: u32,
        _variant: &'static str,
        value: &T,
    ) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        self.0.prep_serialize_enum_variant(variant_index)?;
        value.serialize(&mut *self)?;
        // Skip the `)`.
        self.0.sig_parser.skip_char()?;

        Ok(())
    }

    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq> {
        self.0.sig_parser.skip_char()?;
        self.0.add_padding(ARRAY_ALIGNMENT_DBUS)?;
        // Length in bytes (unfortunately not the same as len passed to us here) which we
        // initially set to 0.
        self.0
            .write_u32(self.0.ctxt.endian(), 0_u32)
            .map_err(|e| Error::InputOutput(e.into()))?;

        let element_signature = self.0.sig_parser.next_signature()?;
        let element_signature_len = element_signature.len();
        let element_alignment = alignment_for_signature(&element_signature, self.0.ctxt.format())?;

        // D-Bus expects us to add padding for the first element even when there is no first
        // element (i-e empty array) so we add padding already.
        let first_padding = self.0.add_padding(element_alignment)?;
        let start = self.0.bytes_written;
        self.0.container_depths = self.0.container_depths.inc_array()?;

        Ok(SeqSerializer {
            ser: self,
            start,
            element_alignment,
            element_signature_len,
            first_padding,
        })
    }

    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple> {
        self.serialize_struct("", len)
    }

    fn serialize_tuple_struct(
        self,
        name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct> {
        self.serialize_struct(name, len)
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant> {
        self.0.prep_serialize_enum_variant(variant_index)?;

        StructSerializer::enum_variant(self).map(StructSeqSerializer::Struct)
    }

    fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap> {
        self.serialize_seq(len)
    }

    fn serialize_struct(self, _name: &'static str, len: usize) -> Result<Self::SerializeStruct> {
        if len == 0 {
            return StructSerializer::unit(self).map(StructSeqSerializer::Struct);
        }

        match self.0.sig_parser.next_char()? {
            VARIANT_SIGNATURE_CHAR => {
                StructSerializer::variant(self).map(StructSeqSerializer::Struct)
            }
            ARRAY_SIGNATURE_CHAR => self.serialize_seq(Some(len)).map(StructSeqSerializer::Seq),
            _ => StructSerializer::structure(self).map(StructSeqSerializer::Struct),
        }
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant> {
        self.0.prep_serialize_enum_variant(variant_index)?;

        StructSerializer::enum_variant(self).map(StructSeqSerializer::Struct)
    }

    fn is_human_readable(&self) -> bool {
        false
    }
}

#[doc(hidden)]
pub struct SeqSerializer<'ser, 'sig, 'b, W> {
    ser: &'b mut Serializer<'ser, 'sig, W>,
    start: usize,
    // alignment of element
    element_alignment: usize,
    // size of element signature
    element_signature_len: usize,
    // First element's padding
    first_padding: usize,
}

impl<'ser, 'sig, 'b, W> SeqSerializer<'ser, 'sig, 'b, W>
where
    W: Write + Seek,
{
    pub(self) fn end_seq(self) -> Result<()> {
        self.ser
            .0
            .sig_parser
            .skip_chars(self.element_signature_len)?;

        // Set size of array in bytes
        let array_len = self.ser.0.bytes_written - self.start;
        let len = usize_to_u32(array_len);
        let total_array_len = (array_len + self.first_padding + 4) as i64;
        self.ser
            .0
            .writer
            .seek(std::io::SeekFrom::Current(-total_array_len))
            .map_err(|e| Error::InputOutput(e.into()))?;
        self.ser
            .0
            .writer
            .write_u32(self.ser.0.ctxt.endian(), len)
            .map_err(|e| Error::InputOutput(e.into()))?;
        self.ser
            .0
            .writer
            .seek(std::io::SeekFrom::Current(total_array_len - 4))
            .map_err(|e| Error::InputOutput(e.into()))?;

        self.ser.0.container_depths = self.ser.0.container_depths.dec_array();

        Ok(())
    }
}

impl<'ser, 'sig, 'b, W> ser::SerializeSeq for SeqSerializer<'ser, 'sig, 'b, W>
where
    W: Write + Seek,
{
    type Ok = ();
    type Error = Error;

    fn serialize_element<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        // We want to keep parsing the same signature repeatedly for each element so we use a
        // disposable clone.
        let sig_parser = self.ser.0.sig_parser.clone();
        self.ser.0.sig_parser = sig_parser.clone();

        value.serialize(&mut *self.ser)?;
        self.ser.0.sig_parser = sig_parser;

        Ok(())
    }

    fn end(self) -> Result<()> {
        self.end_seq()
    }
}

#[doc(hidden)]
pub struct StructSerializer<'ser, 'sig, 'b, W> {
    ser: &'b mut Serializer<'ser, 'sig, W>,
    // The number of `)` in the signature to skip at the end.
    end_parens: u8,
    // The original container depths. We restore to that at the end.
    container_depths: ContainerDepths,
}

impl<'ser, 'sig, 'b, W> StructSerializer<'ser, 'sig, 'b, W>
where
    W: Write + Seek,
{
    fn variant(ser: &'b mut Serializer<'ser, 'sig, W>) -> Result<Self> {
        ser.0.add_padding(VARIANT_ALIGNMENT_DBUS)?;
        let container_depths = ser.0.container_depths;
        ser.0.container_depths = ser.0.container_depths.inc_variant()?;

        Ok(Self {
            ser,
            end_parens: 0,
            container_depths,
        })
    }

    fn structure(ser: &'b mut Serializer<'ser, 'sig, W>) -> Result<Self> {
        let c = ser.0.sig_parser.next_char()?;
        if c != STRUCT_SIG_START_CHAR && c != DICT_ENTRY_SIG_START_CHAR {
            let expected = format!("`{STRUCT_SIG_START_STR}` or `{DICT_ENTRY_SIG_START_STR}`",);

            return Err(serde::de::Error::invalid_type(
                serde::de::Unexpected::Char(c),
                &expected.as_str(),
            ));
        }

        let signature = ser.0.sig_parser.next_signature()?;
        let alignment = alignment_for_signature(&signature, Format::DBus)?;
        ser.0.add_padding(alignment)?;

        ser.0.sig_parser.skip_char()?;
        let container_depths = ser.0.container_depths;
        ser.0.container_depths = ser.0.container_depths.inc_structure()?;

        Ok(Self {
            ser,
            end_parens: 1,
            container_depths,
        })
    }

    fn unit(ser: &'b mut Serializer<'ser, 'sig, W>) -> Result<Self> {
        // serialize as a `0u8`
        serde::Serializer::serialize_u8(&mut *ser, 0)?;

        let container_depths = ser.0.container_depths;
        Ok(Self {
            ser,
            end_parens: 0,
            container_depths,
        })
    }

    fn enum_variant(ser: &'b mut Serializer<'ser, 'sig, W>) -> Result<Self> {
        let mut ser = Self::structure(ser)?;
        ser.end_parens += 1;

        Ok(ser)
    }

    fn serialize_struct_element<T>(&mut self, name: Option<&'static str>, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        match name {
            Some("zvariant::Value::Value") => {
                // Serializing the value of a Value, which means signature was serialized
                // already, and also put aside for us to be picked here.
                let signature = self
                    .ser
                    .0
                    .value_sign
                    .take()
                    .expect("Incorrect Value encoding");

                let sig_parser = SignatureParser::new(signature);
                let bytes_written = self.ser.0.bytes_written;
                let mut ser = Serializer(crate::SerializerCommon::<W> {
                    ctxt: self.ser.0.ctxt,
                    sig_parser,
                    writer: self.ser.0.writer,
                    #[cfg(unix)]
                    fds: self.ser.0.fds,
                    bytes_written,
                    value_sign: None,
                    container_depths: self.ser.0.container_depths,
                });
                value.serialize(&mut ser)?;
                self.ser.0.bytes_written = ser.0.bytes_written;

                Ok(())
            }
            _ => value.serialize(&mut *self.ser),
        }
    }

    fn end_struct(self) -> Result<()> {
        if self.end_parens > 0 {
            self.ser.0.sig_parser.skip_chars(self.end_parens as usize)?;
        }
        // Restore the original container depths.
        self.ser.0.container_depths = self.container_depths;

        Ok(())
    }
}

#[doc(hidden)]
/// Allows us to serialize a struct as an ARRAY.
pub enum StructSeqSerializer<'ser, 'sig, 'b, W> {
    Struct(StructSerializer<'ser, 'sig, 'b, W>),
    Seq(SeqSerializer<'ser, 'sig, 'b, W>),
}

macro_rules! serialize_struct_anon_fields {
    ($trait:ident $method:ident) => {
        impl<'ser, 'sig, 'b, W> ser::$trait for StructSerializer<'ser, 'sig, 'b, W>
        where
            W: Write + Seek,
        {
            type Ok = ();
            type Error = Error;

            fn $method<T>(&mut self, value: &T) -> Result<()>
            where
                T: ?Sized + Serialize,
            {
                self.serialize_struct_element(None, value)
            }

            fn end(self) -> Result<()> {
                self.end_struct()
            }
        }

        impl<'ser, 'sig, 'b, W> ser::$trait for StructSeqSerializer<'ser, 'sig, 'b, W>
        where
            W: Write + Seek,
        {
            type Ok = ();
            type Error = Error;

            fn $method<T>(&mut self, value: &T) -> Result<()>
            where
                T: ?Sized + Serialize,
            {
                match self {
                    StructSeqSerializer::Struct(ser) => ser.$method(value),
                    StructSeqSerializer::Seq(ser) => ser.serialize_element(value),
                }
            }

            fn end(self) -> Result<()> {
                match self {
                    StructSeqSerializer::Struct(ser) => ser.end_struct(),
                    StructSeqSerializer::Seq(ser) => ser.end_seq(),
                }
            }
        }
    };
}
serialize_struct_anon_fields!(SerializeTuple serialize_element);
serialize_struct_anon_fields!(SerializeTupleStruct serialize_field);
serialize_struct_anon_fields!(SerializeTupleVariant serialize_field);

impl<'ser, 'sig, 'b, W> ser::SerializeMap for SeqSerializer<'ser, 'sig, 'b, W>
where
    W: Write + Seek,
{
    type Ok = ();
    type Error = Error;

    fn serialize_key<T>(&mut self, key: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        self.ser.0.add_padding(self.element_alignment)?;

        // We want to keep parsing the same signature repeatedly for each key so we use a
        // disposable clone.
        let sig_parser = self.ser.0.sig_parser.clone();
        self.ser.0.sig_parser = sig_parser.clone();

        // skip `{`
        self.ser.0.sig_parser.skip_char()?;

        key.serialize(&mut *self.ser)?;
        self.ser.0.sig_parser = sig_parser;

        Ok(())
    }

    fn serialize_value<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        // We want to keep parsing the same signature repeatedly for each key so we use a
        // disposable clone.
        let sig_parser = self.ser.0.sig_parser.clone();
        self.ser.0.sig_parser = sig_parser.clone();

        // skip `{` and key char
        self.ser.0.sig_parser.skip_chars(2)?;

        value.serialize(&mut *self.ser)?;
        // Restore the original parser
        self.ser.0.sig_parser = sig_parser;

        Ok(())
    }

    fn end(self) -> Result<()> {
        self.end_seq()
    }
}

macro_rules! serialize_struct_named_fields {
    ($trait:ident) => {
        impl<'ser, 'sig, 'b, W> ser::$trait for StructSerializer<'ser, 'sig, 'b, W>
        where
            W: Write + Seek,
        {
            type Ok = ();
            type Error = Error;

            fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<()>
            where
                T: ?Sized + Serialize,
            {
                self.serialize_struct_element(Some(key), value)
            }

            fn end(self) -> Result<()> {
                self.end_struct()
            }
        }

        impl<'ser, 'sig, 'b, W> ser::$trait for StructSeqSerializer<'ser, 'sig, 'b, W>
        where
            W: Write + Seek,
        {
            type Ok = ();
            type Error = Error;

            fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<()>
            where
                T: ?Sized + Serialize,
            {
                match self {
                    StructSeqSerializer::Struct(ser) => ser.serialize_field(key, value),
                    StructSeqSerializer::Seq(ser) => ser.serialize_element(value),
                }
            }

            fn end(self) -> Result<()> {
                match self {
                    StructSeqSerializer::Struct(ser) => ser.end_struct(),
                    StructSeqSerializer::Seq(ser) => ser.end_seq(),
                }
            }
        }
    };
}
serialize_struct_named_fields!(SerializeStruct);
serialize_struct_named_fields!(SerializeStructVariant);
