#![allow(clippy::unusual_byte_groupings)]
#![deny(rust_2018_idioms)]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/dbus2/zbus/9f7a90d2b594ddc48b7a5f39fda5e00cd56a7dfb/logo.png"
)]
#![doc = include_str!("../README.md")]
#![doc(test(attr(
    warn(unused),
    deny(warnings),
    allow(dead_code),
    // W/o this, we seem to get some bogus warning about `extern crate zbus`.
    allow(unused_extern_crates),
)))]
#![cfg_attr(test, recursion_limit = "256")]

#[macro_use]
mod utils;
pub use utils::*;

mod array;
pub use array::*;

mod basic;
pub use basic::*;

mod dict;
pub use dict::*;

#[deprecated(since = "4.0.0", note = "Use `serialized::Context` instead")]
#[doc(hidden)]
pub type EncodingContext = serialized::Context;
#[deprecated(since = "4.0.0", note = "Use `serialized::Format` instead")]
#[doc(hidden)]
pub type EncodingFormat = serialized::Format;

pub mod serialized;

#[cfg(unix)]
mod fd;
#[cfg(unix)]
pub use fd::*;

mod object_path;
pub use crate::object_path::*;

mod ser;
pub use ser::*;

mod de;

pub mod dbus;
#[cfg(feature = "gvariant")]
pub mod gvariant;

mod signature;
pub use crate::signature::*;

mod complete_type;
pub use complete_type::*;

mod str;
pub use crate::str::*;

mod structure;
pub use crate::structure::*;

#[cfg(feature = "gvariant")]
mod maybe;
#[cfg(feature = "gvariant")]
pub use crate::maybe::*;

mod optional;
pub use crate::optional::*;

mod value;
pub use value::*;

mod serialize_value;
pub use serialize_value::*;

mod deserialize_value;
pub use deserialize_value::*;

mod error;
pub use error::*;

#[macro_use]
mod r#type;
pub use r#type::*;

mod tuple;
pub use tuple::*;

mod from_value;

mod into_value;

mod owned_value;
pub use owned_value::*;

#[cfg(feature = "gvariant")]
mod framing_offset_size;
#[cfg(feature = "gvariant")]
mod framing_offsets;
mod signature_parser;

mod container_depths;

pub use zvariant_derive::{DeserializeDict, OwnedValue, SerializeDict, Type, Value};

// Required for the macros to function within this crate.
extern crate self as zvariant;

// Macro support module, not part of the public API.
#[doc(hidden)]
pub mod export {
    pub use serde;
}

// Re-export all of the `endi` API for ease of use.
pub use endi::*;

#[cfg(test)]
#[allow(clippy::disallowed_names)]
mod tests {
    use std::{
        collections::{BTreeMap, HashMap},
        net::{IpAddr, Ipv4Addr, Ipv6Addr},
    };

    #[cfg(feature = "arrayvec")]
    use arrayvec::{ArrayString, ArrayVec};
    #[cfg(feature = "arrayvec")]
    use std::str::FromStr;

    #[cfg(feature = "gvariant")]
    use glib::{variant::FromVariant, Bytes, Variant};
    use serde::{Deserialize, Serialize};

    use crate::{serialized::Data, to_bytes, to_bytes_for_signature, MaxDepthExceeded};

    #[cfg(unix)]
    use crate::Fd;
    use crate::{
        serialized::{Context, Format},
        Array, Basic, DeserializeDict, DeserializeValue, Dict, Error, ObjectPath, Result,
        SerializeDict, SerializeValue, Signature, Str, Structure, Type, Value, BE, LE,
        NATIVE_ENDIAN,
    };

    // Test through both generic and specific API (wrt byte order)
    macro_rules! basic_type_test {
        ($endian:expr, $format:ident, $test_value:expr, $expected_len:expr, $expected_ty:ty, $align:literal) => {{
            // Lie that we're starting at byte 1 in the overall message to test padding
            let ctxt = Context::new(Format::$format, $endian, 1);
            let encoded = to_bytes(ctxt, &$test_value).unwrap();
            let padding = crate::padding_for_n_bytes(1, $align);
            assert_eq!(
                encoded.len(),
                $expected_len + padding,
                "invalid encoding using `to_bytes`"
            );
            let (decoded, parsed): ($expected_ty, _) = encoded.deserialize().unwrap();
            assert!(decoded == $test_value, "invalid decoding");
            assert!(parsed == encoded.len(), "invalid parsing");

            // Now encode w/o padding
            let ctxt = Context::new(Format::$format, $endian, 0);
            let encoded = to_bytes(ctxt, &$test_value).unwrap();
            assert_eq!(
                encoded.len(),
                $expected_len,
                "invalid encoding using `to_bytes`"
            );

            encoded
        }};
        ($endian:expr, $format:ident, $test_value:expr, $expected_len:expr, $expected_ty:ty, $align:literal, $kind:ident, $expected_value_len:expr) => {{
            let encoded = basic_type_test!(
                $endian,
                $format,
                $test_value,
                $expected_len,
                $expected_ty,
                $align
            );

            // As Value
            let v: Value<'_> = $test_value.into();
            assert_eq!(v.value_signature(), <$expected_ty>::SIGNATURE_STR);
            assert_eq!(v, Value::$kind($test_value));
            value_test!(LE, $format, v, $expected_value_len);

            let v: $expected_ty = v.try_into().unwrap();
            assert_eq!(v, $test_value);

            encoded
        }};
    }

    macro_rules! value_test {
        ($endian:expr, $format:ident, $test_value:expr, $expected_len:expr) => {{
            let ctxt = Context::new(Format::$format, $endian, 0);
            let encoded = to_bytes(ctxt, &$test_value).unwrap();
            assert_eq!(
                encoded.len(),
                $expected_len,
                "invalid encoding using `to_bytes`"
            );
            let (decoded, parsed): (Value<'_>, _) = encoded.deserialize().unwrap();
            assert!(decoded == $test_value, "invalid decoding");
            assert!(parsed == encoded.len(), "invalid parsing");

            encoded
        }};
    }

    fn f64_type_test(
        format: Format,
        value: f64,
        expected_len: usize,
        expected_value_len: usize,
    ) -> crate::serialized::Data<'static, 'static> {
        // Lie that we're starting at byte 1 in the overall message to test padding
        let ctxt = Context::new(format, NATIVE_ENDIAN, 1);
        let encoded = to_bytes(ctxt, &value).unwrap();
        let padding = crate::padding_for_n_bytes(1, 8);
        assert_eq!(
            encoded.len(),
            expected_len + padding,
            "invalid encoding using `to_bytes`"
        );

        let decoded: f64 = encoded.deserialize().unwrap().0;
        assert!(
            (decoded - value).abs() < f64::EPSILON,
            "invalid decoding using `from_slice`"
        );

        // Now encode w/o padding
        let ctxt = Context::new(format, NATIVE_ENDIAN, 0);
        let encoded = to_bytes(ctxt, &value).unwrap();
        assert_eq!(
            encoded.len(),
            expected_len,
            "invalid encoding using `to_bytes`"
        );

        f64_type_test_as_value(format, value, expected_value_len);
        encoded
    }

    fn f64_type_test_as_value(format: Format, value: f64, expected_value_len: usize) {
        let v: Value<'_> = value.into();
        assert_eq!(v.value_signature(), f64::SIGNATURE_STR);
        assert_eq!(v, Value::F64(value));
        f64_value_test(format, v.try_clone().unwrap(), expected_value_len);
        let v: f64 = v.try_into().unwrap();
        assert!((v - value).abs() < f64::EPSILON);
    }

    fn f64_value_test(format: Format, v: Value<'_>, expected_value_len: usize) {
        let ctxt = Context::new(format, LE, 0);
        let encoded = to_bytes(ctxt, &v).unwrap();
        assert_eq!(
            encoded.len(),
            expected_value_len,
            "invalid encoding using `to_bytes`"
        );
        let decoded: Value<'_> = encoded.deserialize().unwrap().0;
        assert!(decoded == v, "invalid decoding using `from_slice`");
    }

    /// Decode with gvariant and compare with expected value (if provided).
    #[cfg(feature = "gvariant")]
    fn decode_with_gvariant<B, T>(encoded: B, expected_value: Option<T>) -> T
    where
        B: AsRef<[u8]> + Send + 'static,
        T: glib::variant::FromVariant + std::fmt::Debug + PartialEq,
    {
        let bytes = Bytes::from_owned(encoded);
        let gv = Variant::from_bytes::<T>(&bytes);
        let v = gv.get::<T>().unwrap();
        if let Some(expected_value) = expected_value {
            assert_eq!(v, expected_value);
        }

        v
    }

    /// Decode a number with gvariant and compare with expected value (if provided).
    ///
    /// `expected_value` is a tuple of (little endian value, big endian value).
    #[cfg(feature = "gvariant")]
    fn decode_num_with_gvariant<B, T>(encoded: B, expected_value: Option<(T, T)>) -> T
    where
        B: AsRef<[u8]> + Send + 'static,
        T: glib::variant::FromVariant + std::fmt::Debug + PartialEq,
    {
        #[allow(unused_variables)]
        let expected_value = expected_value.map(|(le, be)| {
            #[cfg(target_endian = "little")]
            {
                le
            }

            #[cfg(target_endian = "big")]
            {
                be
            }
        });
        decode_with_gvariant(encoded, expected_value)
    }

    // All fixed size types have the same encoding in DBus and GVariant formats.
    //
    // NB: Value (i-e VARIANT type) isn't a fixed size type.

    #[test]
    fn u8_value() {
        let encoded = basic_type_test!(LE, DBus, 77_u8, 1, u8, 1, U8, 4);
        assert_eq!(encoded.len(), 1);
        #[cfg(feature = "gvariant")]
        {
            decode_num_with_gvariant::<_, u8>(encoded, Some((77u8, 77u8)));
            basic_type_test!(LE, GVariant, 77_u8, 1, u8, 1, U8, 3);
        }
    }

    #[test]
    fn i8_value() {
        basic_type_test!(LE, DBus, 77_i8, 2, i8, 2);
        #[cfg(feature = "gvariant")]
        basic_type_test!(LE, GVariant, 77_i8, 2, i8, 2);
    }

    #[cfg(unix)]
    macro_rules! fd_value_test {
        ($endian:expr, $format:ident, $test_value:expr, $expected_len:expr, $align:literal, $expected_value_len:expr) => {{
            use std::os::fd::AsFd;

            // Lie that we're starting at byte 1 in the overall message to test padding
            let ctxt = Context::new(Format::$format, $endian, 1);
            let encoded = to_bytes(ctxt, &$test_value).unwrap();
            let padding = crate::padding_for_n_bytes(1, $align);
            assert_eq!(
                encoded.len(),
                $expected_len + padding,
                "invalid encoding using `to_bytes`"
            );
            #[cfg(unix)]
            let (_, parsed): (Fd<'_>, _) = encoded.deserialize().unwrap();
            assert!(
                parsed == encoded.len(),
                "invalid parsing using `from_slice`"
            );

            // Now encode w/o padding
            let ctxt = Context::new(Format::$format, $endian, 0);
            let encoded = to_bytes(ctxt, &$test_value).unwrap();
            assert_eq!(
                encoded.len(),
                $expected_len,
                "invalid encoding using `to_bytes`"
            );

            // As Value
            let v: Value<'_> = $test_value.into();
            assert_eq!(v.value_signature(), Fd::SIGNATURE_STR);
            assert_eq!(v, Value::Fd($test_value));
            let encoded = to_bytes(ctxt, &v).unwrap();
            assert_eq!(encoded.fds().len(), 1, "invalid encoding using `to_bytes`");
            assert_eq!(
                encoded.len(),
                $expected_value_len,
                "invalid encoding using `to_bytes`"
            );
            let (decoded, parsed): (Value<'_>, _) = encoded.deserialize().unwrap();
            assert_eq!(
                decoded,
                Fd::from(encoded.fds()[0].as_fd()).into(),
                "invalid decoding using `from_slice`"
            );
            assert_eq!(parsed, encoded.len(), "invalid parsing using `from_slice`");

            let v: Fd<'_> = v.try_into().unwrap();
            assert_eq!(v, $test_value);
        }};
    }

    #[cfg(unix)]
    #[test]
    fn fd_value() {
        use std::os::fd::AsFd;

        let stdout = std::io::stdout();
        let fd = stdout.as_fd();
        fd_value_test!(LE, DBus, Fd::from(fd), 4, 4, 8);
        #[cfg(feature = "gvariant")]
        fd_value_test!(LE, GVariant, Fd::from(fd), 4, 4, 6);
    }

    #[test]
    fn u16_value() {
        let encoded = basic_type_test!(BE, DBus, 0xABBA_u16, 2, u16, 2, U16, 6);
        assert_eq!(encoded.len(), 2);
        #[cfg(feature = "gvariant")]
        {
            decode_num_with_gvariant::<_, u16>(encoded, Some((0xBAAB_u16, 0xABBA_u16)));
            basic_type_test!(BE, GVariant, 0xABBA_u16, 2, u16, 2, U16, 4);
        }
    }

    #[test]
    fn i16_value() {
        let encoded = basic_type_test!(BE, DBus, -0xAB0_i16, 2, i16, 2, I16, 6);
        assert_eq!(LE.read_i16(&encoded), 0x50F5_i16);
        #[cfg(feature = "gvariant")]
        {
            decode_num_with_gvariant::<_, i16>(encoded, Some((0x50F5_i16, -0xAB0_i16)));
            basic_type_test!(BE, GVariant, -0xAB0_i16, 2, i16, 2, I16, 4);
        }
    }

    #[test]
    fn u32_value() {
        let encoded = basic_type_test!(BE, DBus, 0xABBA_ABBA_u32, 4, u32, 4, U32, 8);
        assert_eq!(encoded.len(), 4);
        #[cfg(feature = "gvariant")]
        {
            decode_num_with_gvariant::<_, u32>(encoded, Some((0xBAAB_BAAB_u32, 0xABBA_ABBA_u32)));
            basic_type_test!(BE, GVariant, 0xABBA_ABBA_u32, 4, u32, 4, U32, 6);
        }
    }

    #[test]
    fn i32_value() {
        let encoded = basic_type_test!(BE, DBus, -0xABBA_AB0_i32, 4, i32, 4, I32, 8);
        assert_eq!(LE.read_i32(&encoded), 0x5055_44F5_i32);
        #[cfg(feature = "gvariant")]
        {
            decode_num_with_gvariant::<_, i32>(encoded, Some((0x5055_44F5_i32, -0xABBA_AB0_i32)));
            basic_type_test!(BE, GVariant, -0xABBA_AB0_i32, 4, i32, 4, I32, 6);
        }
    }

    // u64 is covered by `value_value` test below

    #[test]
    fn i64_value() {
        let encoded = basic_type_test!(BE, DBus, -0xABBA_ABBA_ABBA_AB0_i64, 8, i64, 8, I64, 16);
        assert_eq!(LE.read_i64(&encoded), 0x5055_4455_4455_44F5_i64);
        #[cfg(feature = "gvariant")]
        {
            decode_num_with_gvariant::<_, i64>(
                encoded,
                Some((0x5055_4455_4455_44F5_i64, -0xABBA_ABBA_ABBA_AB0_i64)),
            );
            basic_type_test!(BE, GVariant, -0xABBA_ABBA_ABBA_AB0_i64, 8, i64, 8, I64, 10);
        }
    }

    #[test]
    fn f64_value() {
        let encoded = f64_type_test(Format::DBus, 99999.99999_f64, 8, 16);
        assert!((NATIVE_ENDIAN.read_f64(&encoded) - 99999.99999_f64).abs() < f64::EPSILON);
        #[cfg(feature = "gvariant")]
        {
            assert!(
                (decode_with_gvariant::<_, f64>(encoded, None) - 99999.99999_f64).abs()
                    < f64::EPSILON
            );
            f64_type_test(Format::GVariant, 99999.99999_f64, 8, 10);
        }
    }

    #[test]
    fn str_value() {
        let string = String::from("hello world");
        basic_type_test!(LE, DBus, string, 16, String, 4);
        basic_type_test!(LE, DBus, string, 16, &str, 4);

        // GVariant format now
        #[cfg(feature = "gvariant")]
        {
            let encoded = basic_type_test!(LE, GVariant, string, 12, String, 1);
            decode_with_gvariant::<_, String>(encoded, Some(String::from("hello world")));
        }

        let string = "hello world";
        basic_type_test!(LE, DBus, string, 16, &str, 4);
        basic_type_test!(LE, DBus, string, 16, String, 4);

        // As Value
        let v: Value<'_> = string.into();
        assert_eq!(v.value_signature(), "s");
        assert_eq!(v, Value::new("hello world"));
        value_test!(LE, DBus, v, 20);
        #[cfg(feature = "gvariant")]
        {
            let encoded = value_test!(LE, GVariant, v, 14);

            // Check encoding against GLib
            let bytes = Bytes::from_owned(encoded);
            let gv = Variant::from_bytes::<Variant>(&bytes);
            let variant = gv.as_variant().unwrap();
            assert_eq!(variant.str().unwrap(), "hello world");
        }

        let v: String = v.try_into().unwrap();
        assert_eq!(v, "hello world");

        // Check for interior null bytes which are not allowed
        let ctxt = Context::new_dbus(LE, 0);
        assert!(Data::new(&b"\x0b\0\0\0hello\0world\0"[..], ctxt)
            .deserialize::<&str>()
            .is_err());
        assert!(to_bytes(ctxt, &"hello\0world").is_err());

        // GVariant format doesn't allow null bytes either
        #[cfg(feature = "gvariant")]
        {
            let ctxt = Context::new_gvariant(LE, 0);
            assert!(Data::new(&b"\x0b\0\0\0hello\0world\0"[..], ctxt)
                .deserialize::<&str>()
                .is_err());
            assert!(to_bytes(ctxt, &"hello\0world").is_err());
        }

        // Characters are treated as strings
        basic_type_test!(LE, DBus, 'c', 6, char, 4);
        #[cfg(feature = "gvariant")]
        basic_type_test!(LE, GVariant, 'c', 2, char, 1);

        // As Value
        let v: Value<'_> = "c".into();
        assert_eq!(v.value_signature(), "s");
        let ctxt = Context::new_dbus(LE, 0);
        let encoded = to_bytes(ctxt, &v).unwrap();
        assert_eq!(encoded.len(), 10);
        let (v, _) = encoded.deserialize::<Value<'_>>().unwrap();
        assert_eq!(v, Value::new("c"));
    }

    #[cfg(feature = "arrayvec")]
    #[test]
    fn array_string_value() {
        let s = ArrayString::<32>::from_str("hello world!").unwrap();
        let ctxt = Context::new_dbus(LE, 0);
        let encoded = to_bytes(ctxt, &s).unwrap();
        assert_eq!(encoded.len(), 17);
        let decoded: ArrayString<32> = encoded.deserialize().unwrap().0;
        assert_eq!(&decoded, "hello world!");
    }

    #[cfg(feature = "heapless")]
    #[test]
    fn heapless_string_value() {
        use heapless::String;

        let s = String::<32>::try_from("hello world!").unwrap();
        let ctxt = Context::new_dbus(LE, 0);
        let encoded = to_bytes(ctxt, &s).unwrap();
        assert_eq!(encoded.len(), 17);
        let decoded: String<32> = encoded.deserialize().unwrap().0;
        assert_eq!(&decoded, "hello world!");
    }

    #[test]
    fn signature_value() {
        let sig = Signature::try_from("yys").unwrap();
        basic_type_test!(LE, DBus, sig, 5, Signature<'_>, 1);

        #[cfg(feature = "gvariant")]
        {
            let encoded = basic_type_test!(LE, GVariant, sig, 4, Signature<'_>, 1);
            decode_with_gvariant::<_, String>(encoded, Some(String::from("yys")));
        }

        // As Value
        let v: Value<'_> = sig.into();
        assert_eq!(v.value_signature(), "g");
        let encoded = value_test!(LE, DBus, v, 8);
        let v = encoded.deserialize::<Value<'_>>().unwrap().0;
        assert_eq!(v, Value::Signature(Signature::try_from("yys").unwrap()));

        // GVariant format now
        #[cfg(feature = "gvariant")]
        {
            let encoded = value_test!(LE, GVariant, v, 6);
            let v = encoded.deserialize::<Value<'_>>().unwrap().0;
            assert_eq!(v, Value::Signature(Signature::try_from("yys").unwrap()));
        }
    }

    #[test]
    fn object_path_value() {
        let o = ObjectPath::try_from("/hello/world").unwrap();
        basic_type_test!(LE, DBus, o, 17, ObjectPath<'_>, 4);

        #[cfg(feature = "gvariant")]
        {
            let encoded = basic_type_test!(LE, GVariant, o, 13, ObjectPath<'_>, 1);
            decode_with_gvariant::<_, String>(encoded, Some(String::from("/hello/world")));
        }

        // As Value
        let v: Value<'_> = o.into();
        assert_eq!(v.value_signature(), "o");
        let encoded = value_test!(LE, DBus, v, 21);
        let v = encoded.deserialize::<Value<'_>>().unwrap().0;
        assert_eq!(
            v,
            Value::ObjectPath(ObjectPath::try_from("/hello/world").unwrap())
        );

        // GVariant format now
        #[cfg(feature = "gvariant")]
        {
            let encoded = value_test!(LE, GVariant, v, 15);
            let v = encoded.deserialize::<Value<'_>>().unwrap().0;
            assert_eq!(
                v,
                Value::ObjectPath(ObjectPath::try_from("/hello/world").unwrap())
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn unit_fds() {
        let ctxt = Context::new_dbus(BE, 0);
        let encoded = to_bytes(ctxt, &()).unwrap();
        assert_eq!(encoded.len(), 0, "invalid encoding using `to_bytes`");
        let _: () = encoded
            .deserialize()
            .expect("invalid decoding using `from_slice`")
            .0;
    }

    #[test]
    fn unit() {
        let ctxt = Context::new_dbus(BE, 0);
        let encoded = to_bytes(ctxt, &()).unwrap();
        assert_eq!(encoded.len(), 0, "invalid encoding using `to_bytes`");
        let _: () = encoded
            .deserialize()
            .expect("invalid decoding using `from_slice`")
            .0;
    }

    #[test]
    fn array_value() {
        // Let's use D-Bus/GVariant terms

        //
        // Array of u8
        //
        // First a normal Rust array that is actually serialized as a struct (thank you Serde!)
        assert_eq!(<[u8; 2]>::signature(), "(yy)");
        let ay = [77u8, 88];
        let ctxt = Context::new_dbus(LE, 0);
        let encoded = to_bytes(ctxt, &ay).unwrap();
        assert_eq!(encoded.len(), 2);
        let decoded: [u8; 2] = encoded.deserialize().unwrap().0;
        assert_eq!(&decoded, &[77u8, 88]);

        // Then rest of the tests just use ArrayVec, heapless::Vec or Vec
        #[cfg(feature = "arrayvec")]
        let ay = ArrayVec::from([77u8, 88]);
        #[cfg(all(not(feature = "arrayvec"), feature = "heapless"))]
        let ay = heapless::Vec::<_, 2>::from_slice(&[77u8, 88]).unwrap();
        #[cfg(all(not(feature = "arrayvec"), not(feature = "heapless")))]
        let ay = vec![77u8, 88];
        let ctxt = Context::new_dbus(LE, 0);
        let encoded = to_bytes(ctxt, &ay).unwrap();
        assert_eq!(encoded.len(), 6);

        #[cfg(feature = "arrayvec")]
        let decoded: ArrayVec<u8, 2> = encoded.deserialize().unwrap().0;
        #[cfg(all(not(feature = "arrayvec"), feature = "heapless"))]
        let decoded: heapless::Vec<u8, 2> = encoded.deserialize().unwrap().0;
        #[cfg(all(not(feature = "arrayvec"), not(feature = "heapless")))]
        let decoded: Vec<u8> = encoded.deserialize().unwrap().0;
        assert_eq!(&decoded.as_slice(), &[77u8, 88]);

        // GVariant format now
        #[cfg(feature = "gvariant")]
        {
            let ctxt = Context::new_gvariant(LE, 0);
            let gv_encoded = to_bytes(ctxt, &ay).unwrap();
            assert_eq!(gv_encoded.len(), 2);

            // Check encoding against GLib
            let bytes = Bytes::from_owned(gv_encoded);
            let variant = Variant::from_bytes::<&[u8]>(&bytes);
            assert_eq!(variant.n_children(), 2);
            assert_eq!(variant.child_value(0).get::<u8>().unwrap(), 77);
            assert_eq!(variant.child_value(1).get::<u8>().unwrap(), 88);
        }
        let ctxt = Context::new_dbus(LE, 0);

        // As Value
        let v: Value<'_> = ay[..].into();
        assert_eq!(v.value_signature(), "ay");
        let encoded = to_bytes(ctxt, &v).unwrap();
        assert_eq!(encoded.len(), 10);
        let v = encoded.deserialize::<Value<'_>>().unwrap().0;
        if let Value::Array(array) = v {
            assert_eq!(*array.element_signature(), "y");
            assert_eq!(array.len(), 2);
            assert_eq!(array.get(0).unwrap(), Some(77u8));
            assert_eq!(array.get(1).unwrap(), Some(88u8));
        } else {
            panic!();
        }

        // Now try as Vec
        let vec = ay.to_vec();
        let encoded = to_bytes(ctxt, &vec).unwrap();
        assert_eq!(encoded.len(), 6);

        // Vec as Value
        let v: Value<'_> = Array::from(&vec).into();
        assert_eq!(v.value_signature(), "ay");
        let encoded = to_bytes(ctxt, &v).unwrap();
        assert_eq!(encoded.len(), 10);

        // Empty array
        let at: Vec<u64> = vec![];
        let encoded = to_bytes(ctxt, &at).unwrap();
        assert_eq!(encoded.len(), 8);

        // GVariant format now
        #[cfg(feature = "gvariant")]
        {
            let ctxt = Context::new_gvariant(LE, 0);
            let gv_encoded = to_bytes(ctxt, &at).unwrap();
            assert_eq!(gv_encoded.len(), 0);
            let at = encoded.deserialize::<Vec<u64>>().unwrap().0;
            assert_eq!(at.len(), 0);
        }
        let ctxt = Context::new_dbus(LE, 0);

        // As Value
        let v: Value<'_> = at[..].into();
        assert_eq!(v.value_signature(), "at");
        let encoded = to_bytes(ctxt, &v).unwrap();
        assert_eq!(encoded.len(), 8);
        let v = encoded.deserialize::<Value<'_>>().unwrap().0;
        if let Value::Array(array) = v {
            assert_eq!(*array.element_signature(), "t");
            assert_eq!(array.len(), 0);
        } else {
            panic!();
        }

        // GVariant format now
        #[cfg(feature = "gvariant")]
        {
            let ctxt = Context::new_gvariant(LE, 0);
            let v: Value<'_> = at[..].into();
            let gv_encoded = to_bytes(ctxt, &v).unwrap();
            assert_eq!(gv_encoded.len(), 3);
            let v = gv_encoded.deserialize::<Value<'_>>().unwrap().0;
            if let Value::Array(array) = v {
                assert_eq!(*array.element_signature(), "t");
                assert_eq!(array.len(), 0);
            } else {
                panic!();
            }

            // Check encoding against GLib
            let bytes = Bytes::from_owned(gv_encoded);
            let variant = Variant::from_bytes::<&[&str]>(&bytes);
            assert_eq!(variant.n_children(), 0);
        }
        let ctxt = Context::new_dbus(LE, 0);

        //
        // Array of strings
        //
        // Can't use 'as' as it's a keyword
        let as_ = vec!["Hello", "World", "Now", "Bye!"];
        let encoded = to_bytes(ctxt, &as_).unwrap();
        assert_eq!(encoded.len(), 45);
        let decoded = encoded.deserialize::<Vec<&str>>().unwrap().0;
        assert_eq!(decoded.len(), 4);
        assert_eq!(decoded[0], "Hello");
        assert_eq!(decoded[1], "World");

        let decoded = encoded.deserialize::<Vec<String>>().unwrap().0;
        assert_eq!(decoded.as_slice(), as_.as_slice());

        // Decode just the second string
        let slice = encoded.slice(14..);
        let decoded: &str = slice.deserialize().unwrap().0;
        assert_eq!(decoded, "World");

        // As Value
        let v: Value<'_> = as_[..].into();
        assert_eq!(v.value_signature(), "as");
        let encoded = to_bytes(ctxt, &v).unwrap();
        assert_eq!(encoded.len(), 49);
        let v = encoded.deserialize().unwrap().0;
        if let Value::Array(array) = v {
            assert_eq!(*array.element_signature(), "s");
            assert_eq!(array.len(), 4);
            assert_eq!(array[0], Value::new("Hello"));
            assert_eq!(array[1], Value::new("World"));
        } else {
            panic!();
        }

        let v: Value<'_> = as_[..].into();
        let a: Array<'_> = v.try_into().unwrap();
        let _ve: Vec<String> = a.try_into().unwrap();

        // GVariant format now
        #[cfg(feature = "gvariant")]
        {
            let ctxt = Context::new_gvariant(LE, 0);
            let v: Value<'_> = as_[..].into();
            let gv_encoded = to_bytes(ctxt, &v).unwrap();
            assert_eq!(gv_encoded.len(), 28);

            // Check encoding against GLib
            let bytes = Bytes::from_owned(gv_encoded);
            let variant = Variant::from_bytes::<Variant>(&bytes);
            assert_eq!(variant.n_children(), 1);
            let decoded: Vec<String> = variant.child_value(0).get().unwrap();
            assert_eq!(decoded[0], "Hello");
            assert_eq!(decoded[1], "World");
        }

        // Array of Struct, which in turn containin an Array (We gotta go deeper!)
        // Signature: "a(yu(xbxas)s)");
        let ar = vec![(
            // top-most simple fields
            u8::MAX,
            u32::MAX,
            (
                // 2nd level simple fields
                i64::MAX,
                true,
                i64::MAX,
                // 2nd level array field
                &["Hello", "World"][..],
            ),
            // one more top-most simple field
            "hello",
        )];
        let ctxt = Context::new_dbus(LE, 0);
        let encoded = to_bytes(ctxt, &ar).unwrap();
        assert_eq!(encoded.len(), 78);
        #[allow(clippy::type_complexity)]
        let decoded: Vec<(u8, u32, (i64, bool, i64, Vec<&str>), &str)> =
            encoded.deserialize().unwrap().0;
        assert_eq!(decoded.len(), 1);
        let r = &decoded[0];
        assert_eq!(r.0, u8::MAX);
        assert_eq!(r.1, u32::MAX);
        let inner_r = &r.2;
        assert_eq!(inner_r.0, i64::MAX);
        assert!(inner_r.1);
        assert_eq!(inner_r.2, i64::MAX);
        let as_ = &inner_r.3;
        assert_eq!(as_.len(), 2);
        assert_eq!(as_[0], "Hello");
        assert_eq!(as_[1], "World");
        assert_eq!(r.3, "hello");

        // GVariant format now
        #[cfg(feature = "gvariant")]
        {
            let ctxt = Context::new_gvariant(LE, 0);
            let gv_encoded = to_bytes(ctxt, &ar).unwrap();
            assert_eq!(gv_encoded.len(), 54);
            let decoded: Vec<(u8, u32, (i64, bool, i64, Vec<&str>), &str)> =
                gv_encoded.deserialize().unwrap().0;
            assert_eq!(decoded.len(), 1);
            let r = &decoded[0];
            assert_eq!(r.0, u8::MAX);
            assert_eq!(r.1, u32::MAX);
            let inner_r = &r.2;
            assert_eq!(inner_r.0, i64::MAX);
            assert!(inner_r.1);
            assert_eq!(inner_r.2, i64::MAX);
            let as_ = &inner_r.3;
            assert_eq!(as_.len(), 2);
            assert_eq!(as_[0], "Hello");
            assert_eq!(as_[1], "World");
            assert_eq!(r.3, "hello");

            // Check encoding against GLib
            let bytes = Bytes::from_owned(gv_encoded);
            let variant = Variant::from_bytes::<
                Vec<(u8, u32, (i64, bool, i64, Vec<String>), String)>,
            >(&bytes);
            assert_eq!(variant.n_children(), 1);
            let r: (u8, u32, (i64, bool, i64, Vec<String>), String) =
                variant.child_value(0).get().unwrap();
            assert_eq!(r.0, u8::MAX);
            assert_eq!(r.1, u32::MAX);
        }
        let ctxt = Context::new_dbus(LE, 0);

        // As Value
        let v: Value<'_> = ar[..].into();
        assert_eq!(v.value_signature(), "a(yu(xbxas)s)");
        let encoded = to_bytes(ctxt, &v).unwrap();
        assert_eq!(encoded.len(), 94);
        let v = encoded.deserialize::<Value<'_>>().unwrap().0;
        if let Value::Array(array) = v.try_clone().unwrap() {
            assert_eq!(*array.element_signature(), "(yu(xbxas)s)");
            assert_eq!(array.len(), 1);
            let r = &array[0];
            if let Value::Structure(r) = r {
                let fields = r.fields();
                assert_eq!(fields[0], Value::U8(u8::MAX));
                assert_eq!(fields[1], Value::U32(u32::MAX));
                if let Value::Structure(r) = &fields[2] {
                    let fields = r.fields();
                    assert_eq!(fields[0], Value::I64(i64::MAX));
                    assert_eq!(fields[1], Value::Bool(true));
                    assert_eq!(fields[2], Value::I64(i64::MAX));
                    if let Value::Array(as_) = &fields[3] {
                        assert_eq!(as_.len(), 2);
                        assert_eq!(as_[0], Value::new("Hello"));
                        assert_eq!(as_[1], Value::new("World"));
                    } else {
                        panic!();
                    }
                } else {
                    panic!();
                }
                assert_eq!(fields[3], Value::new("hello"));
            } else {
                panic!();
            }
        } else {
            panic!();
        }

        // GVariant format now
        #[cfg(feature = "gvariant")]
        {
            use rand::{distributions::Alphanumeric, thread_rng, Rng};

            let ctxt = Context::new_gvariant(LE, 0);
            let gv_encoded = to_bytes(ctxt, &v).unwrap();
            assert_eq!(gv_encoded.len(), 68);
            let v: Value<'_> = gv_encoded.deserialize().unwrap().0;
            if let Value::Array(array) = v {
                assert_eq!(*array.element_signature(), "(yu(xbxas)s)");
                assert_eq!(array.len(), 1);
                let r = &array.get(0).unwrap().unwrap();
                if let Value::Structure(r) = r {
                    let fields = r.fields();
                    assert_eq!(fields[0], Value::U8(u8::MAX));
                    assert_eq!(fields[1], Value::U32(u32::MAX));
                    if let Value::Structure(r) = &fields[2] {
                        let fields = r.fields();
                        assert_eq!(fields[0], Value::I64(i64::MAX));
                        assert_eq!(fields[1], Value::Bool(true));
                        assert_eq!(fields[2], Value::I64(i64::MAX));
                        if let Value::Array(as_) = &fields[3] {
                            assert_eq!(as_.len(), 2);
                            assert_eq!(as_.get(0).unwrap(), Some("Hello"));
                            assert_eq!(as_.get(1).unwrap(), Some("World"));
                        } else {
                            panic!();
                        }
                    } else {
                        panic!();
                    }
                    assert_eq!(fields[3], Value::new("hello"));
                } else {
                    panic!();
                }
            } else {
                panic!();
            }

            // Check encoding against GLib
            let bytes = Bytes::from_owned(gv_encoded);
            let variant = Variant::from_bytes::<Variant>(&bytes);
            assert_eq!(variant.n_children(), 1);
            let child: Variant = variant.child_value(0);
            let r: (u8, u32, (i64, bool, i64, Vec<String>), String) =
                child.child_value(0).get().unwrap();
            assert_eq!(r.0, u8::MAX);
            assert_eq!(r.1, u32::MAX);

            let mut rng = thread_rng();
            // Let's test GVariant ser/de of a 254 byte array with variable-width elements as to
            // ensure no problems with non-normal BS of GVariant.
            let as_ = vec![
                (&mut rng)
                    .sample_iter(Alphanumeric)
                    .map(char::from)
                    .take(126)
                    .collect::<String>(),
                (&mut rng)
                    .sample_iter(Alphanumeric)
                    .map(char::from)
                    .take(126)
                    .collect::<String>(),
            ];
            let gv_encoded = to_bytes(ctxt, &as_).unwrap();
            // 252 chars + 2 null terminator bytes doesn't leave room for 2 framing offset bytes so
            // a 2-byte offset is chosen by the serializer.
            assert_eq!(gv_encoded.len(), 258);

            // Check encoding against GLib
            let bytes = Bytes::from_owned(gv_encoded.to_owned());
            let variant = Variant::from_bytes::<Vec<String>>(&bytes);
            assert_eq!(variant.n_children(), 2);
            assert_eq!(variant.child_value(0).get::<String>().unwrap(), as_[0]);
            assert_eq!(variant.child_value(1).get::<String>().unwrap(), as_[1]);
            // Also check if our own deserializer does the right thing
            let as2: Vec<String> = gv_encoded.deserialize().unwrap().0;
            assert_eq!(as2, as_);

            // Test conversion of Array of Value to Vec<Value>
            let v = Value::new(vec![Value::new(43), Value::new("bonjour")]);
            let av = <Array<'_>>::try_from(v).unwrap();
            let av = <Vec<Value<'_>>>::try_from(av).unwrap();
            assert_eq!(av[0], Value::new(43));
            assert_eq!(av[1], Value::new("bonjour"));

            let vec = vec![1, 2];
            let val = Value::new(&vec);
            assert_eq!(TryInto::<Vec<i32>>::try_into(val).unwrap(), vec);
        }
    }

    #[test]
    fn struct_byte_array() {
        let ctxt = Context::new_dbus(LE, 0);
        let value: (Vec<u8>, HashMap<String, Value<'_>>) = (Vec::new(), HashMap::new());
        let value = zvariant::to_bytes(ctxt, &value).unwrap();
        #[cfg(feature = "serde_bytes")]
        let (bytes, map): (&serde_bytes::Bytes, HashMap<&str, Value<'_>>) = value
            .deserialize()
            .expect("Could not deserialize serde_bytes::Bytes in struct.")
            .0;
        #[cfg(not(feature = "serde_bytes"))]
        let (bytes, map): (&[u8], HashMap<&str, Value<'_>>) = value
            .deserialize()
            .expect("Could not deserialize u8 slice in struct")
            .0;

        assert!(bytes.is_empty());
        assert!(map.is_empty());
    }

    #[test]
    fn struct_value() {
        // Struct->Value
        let s: Value<'_> = ("a", "b", (1, 2)).into();

        let ctxt = Context::new_dbus(LE, 0);
        let encoded = to_bytes(ctxt, &s).unwrap();
        assert_eq!(dbg!(encoded.len()), 40);
        let decoded: Value<'_> = encoded.deserialize().unwrap().0;
        let s = <Structure<'_>>::try_from(decoded).unwrap();
        let outer = <(Str<'_>, Str<'_>, Structure<'_>)>::try_from(s).unwrap();
        assert_eq!(outer.0, "a");
        assert_eq!(outer.1, "b");

        let inner = <(i32, i32)>::try_from(outer.2).unwrap();
        assert_eq!(inner.0, 1);
        assert_eq!(inner.1, 2);

        #[derive(Serialize, Deserialize, Type, PartialEq, Debug)]
        struct Foo {
            val: u32,
        }

        let foo = Foo { val: 99 };
        let v = SerializeValue(&foo);
        let encoded = to_bytes(ctxt, &v).unwrap();
        let decoded: DeserializeValue<'_, Foo> = encoded.deserialize().unwrap().0;
        assert_eq!(decoded.0, foo);

        // Unit struct should be treated as a 0-sized tuple (the same as unit type)
        #[derive(Serialize, Deserialize, Type, PartialEq, Debug)]
        struct Unit;

        assert_eq!(Unit::signature(), "");
        let encoded = to_bytes(ctxt, &Unit).unwrap();
        assert_eq!(encoded.len(), 0);
        let _decoded: Unit = encoded.deserialize().unwrap().0;

        // Structs w/o fields should be treated as a unit struct.
        #[derive(Serialize, Deserialize, Type, PartialEq, Debug)]
        struct NoFields {}

        assert_eq!(NoFields::signature(), "y");
        let encoded = to_bytes(ctxt, &NoFields {}).unwrap();
        assert_eq!(encoded.len(), 1);
        let _decoded: NoFields = encoded.deserialize().unwrap().0;

        #[cfg(feature = "gvariant")]
        {
            let ctxt = Context::new_gvariant(LE, 0);
            let encoded = to_bytes(ctxt, &NoFields {}).unwrap();
            assert_eq!(encoded.len(), 1);
            let _decoded: NoFields = encoded.deserialize().unwrap().0;
        }
    }

    #[test]
    fn struct_ref() {
        let ctxt = Context::new_dbus(LE, 0);
        let encoded = to_bytes(ctxt, &(&1u32, &2u32)).unwrap();
        let decoded: [u32; 2] = encoded.deserialize().unwrap().0;
        assert_eq!(decoded, [1u32, 2u32]);
    }

    #[test]
    fn dict_value() {
        let mut map: HashMap<i64, &str> = HashMap::new();
        map.insert(1, "123");
        map.insert(2, "456");
        let ctxt = Context::new_dbus(LE, 0);
        let encoded = to_bytes(ctxt, &map).unwrap();
        assert_eq!(dbg!(encoded.len()), 40);
        let decoded: HashMap<i64, &str> = encoded.deserialize().unwrap().0;
        assert_eq!(decoded[&1], "123");
        assert_eq!(decoded[&2], "456");

        // GVariant format now
        #[cfg(feature = "gvariant")]
        {
            let ctxt = Context::new_gvariant(NATIVE_ENDIAN, 0);
            let gv_encoded = to_bytes(ctxt, &map).unwrap();
            assert_eq!(gv_encoded.len(), 30);
            let map: HashMap<i64, &str> = encoded.deserialize().unwrap().0;
            assert_eq!(map[&1], "123");
            assert_eq!(map[&2], "456");

            // Check encoding against GLib
            let bytes = Bytes::from_owned(gv_encoded);
            let variant = Variant::from_bytes::<HashMap<i64, &str>>(&bytes);
            assert_eq!(variant.n_children(), 2);
            let map: HashMap<i64, String> = HashMap::from_variant(&variant).unwrap();
            assert_eq!(map[&1], "123");
            assert_eq!(map[&2], "456");
        }
        let ctxt = Context::new_dbus(LE, 0);

        // As Value
        let v: Value<'_> = Dict::from(map).into();
        assert_eq!(v.value_signature(), "a{xs}");
        let encoded = to_bytes(ctxt, &v).unwrap();
        assert_eq!(encoded.len(), 48);
        // Convert it back
        let dict: Dict<'_, '_> = v.try_into().unwrap();
        let map: HashMap<i64, String> = dict.try_clone().unwrap().try_into().unwrap();
        assert_eq!(map[&1], "123");
        assert_eq!(map[&2], "456");
        // Also decode it back
        let v = encoded.deserialize().unwrap().0;
        if let Value::Dict(dict) = v {
            assert_eq!(dict.get::<i64, &str>(&1).unwrap().unwrap(), "123");
            assert_eq!(dict.get::<i64, &str>(&2).unwrap().unwrap(), "456");
        } else {
            panic!();
        }
        // Convert it to a BTreeMap too.
        let map: BTreeMap<i64, String> = dict.try_into().unwrap();
        assert_eq!(map[&1], "123");
        assert_eq!(map[&2], "456");
        // Use iterator
        let mut dict = Dict::from(map);
        let expect = vec![
            (Value::from(1i64), Value::from("123")),
            (Value::from(2i64), Value::from("456")),
        ];
        let expect_iter = expect.iter().map(|(k, v)| (k, v)).collect::<Vec<_>>();
        let actual = dict.iter().collect::<Vec<_>>();
        assert_eq!(actual, expect_iter);
        let actual = dict.iter().collect::<Vec<_>>();
        assert_eq!(actual, expect_iter);
        let actual = dict.iter().collect::<Vec<_>>();
        assert_eq!(actual, expect_iter);
        for (_, v) in dict.iter_mut() {
            if let Value::Str(vv) = v {
                *vv = Str::from(vv.to_string() + "-hello");
            }
        }
        let actual = dict.into_iter().collect::<Vec<_>>();
        let expect = vec![
            (Value::from(1i64), Value::from("123-hello")),
            (Value::from(2i64), Value::from("456-hello")),
        ];
        assert_eq!(actual, expect);

        #[cfg(feature = "gvariant")]
        {
            // GVariant-format requires framing offsets for dict entries with variable-length keys
            // so let's test that.
            let mut map: HashMap<&str, &str> = HashMap::new();
            map.insert("hi", "1234");
            map.insert("world", "561");
            let ctxt = Context::new_gvariant(NATIVE_ENDIAN, 0);
            let gv_encoded = to_bytes(ctxt, &map).unwrap();
            assert_eq!(gv_encoded.len(), 22);
            let map: HashMap<&str, &str> = gv_encoded.deserialize().unwrap().0;
            assert_eq!(map["hi"], "1234");
            assert_eq!(map["world"], "561");

            // Ensure SerializeValue produces the same result as Value
            // Tests for https://github.com/dbus2/zbus/issues/868
            let mut map = std::collections::HashMap::<&str, &str>::new();
            map.insert("k", "v");
            let gv_ser_value_encoded =
                zvariant::to_bytes(ctxt, &zvariant::SerializeValue(&map)).unwrap();
            let gv_value_encoded = to_bytes(ctxt, &zvariant::Value::new(map)).unwrap();
            assert_eq!(gv_value_encoded.as_ref(), gv_ser_value_encoded.as_ref());

            // Check encoding against GLib
            let bytes = Bytes::from_owned(gv_encoded);
            let variant = Variant::from_bytes::<HashMap<&str, &str>>(&bytes);
            assert_eq!(variant.n_children(), 2);
            let map: HashMap<String, String> = HashMap::from_variant(&variant).unwrap();
            assert_eq!(map["hi"], "1234");
            assert_eq!(map["world"], "561");

            // Now the same but empty dict this time
            let map: HashMap<&str, &str> = HashMap::new();
            let gv_encoded = to_bytes(ctxt, &map).unwrap();
            assert_eq!(gv_encoded.len(), 0);
            let map: HashMap<&str, &str> = gv_encoded.deserialize().unwrap().0;
            assert_eq!(map.len(), 0);
        }
        let ctxt = Context::new_dbus(LE, 0);

        // Now a hand-crafted Dict Value but with a Value as value
        let mut dict = Dict::new(<&str>::signature(), Value::signature());
        dict.add("hello", Value::new("there")).unwrap();
        dict.add("bye", Value::new("now")).unwrap();
        let v: Value<'_> = dict.into();
        assert_eq!(v.value_signature(), "a{sv}");
        let encoded = to_bytes(ctxt, &v).unwrap();
        assert_eq!(dbg!(encoded.len()), 66);
        let v: Value<'_> = encoded.deserialize().unwrap().0;
        if let Value::Dict(dict) = v {
            assert_eq!(
                dict.get::<&str, Value<'_>>(&"hello").unwrap().unwrap(),
                Value::new("there")
            );
            assert_eq!(
                dict.get::<_, Value<'_>>(&"bye").unwrap().unwrap(),
                Value::new("now")
            );

            // Try converting to a HashMap
            let map = <HashMap<String, Value<'_>>>::try_from(dict.try_clone().unwrap()).unwrap();
            assert_eq!(map["hello"], Value::new("there"));
            assert_eq!(map["bye"], Value::new("now"));

            // Try converting to a BTreeMap
            let map = <BTreeMap<String, Value<'_>>>::try_from(dict).unwrap();
            assert_eq!(map["hello"], Value::new("there"));
            assert_eq!(map["bye"], Value::new("now"));
        } else {
            panic!();
        }

        #[derive(SerializeDict, DeserializeDict, Type, PartialEq, Debug)]
        #[zvariant(signature = "a{sv}")]
        struct Test {
            process_id: Option<u32>,
            group_id: Option<u32>,
            user: String,
        }
        let test = Test {
            process_id: Some(42),
            group_id: None,
            user: "me".to_string(),
        };

        let encoded = to_bytes(ctxt, &test).unwrap();
        assert_eq!(encoded.len(), 51);

        let decoded: HashMap<&str, Value<'_>> = encoded.deserialize().unwrap().0;
        assert_eq!(decoded["process_id"], Value::U32(42));
        assert_eq!(decoded["user"], Value::new("me"));
        assert!(!decoded.contains_key("group_id"));

        let decoded: Test = encoded.deserialize().unwrap().0;
        assert_eq!(decoded, test);

        #[derive(SerializeDict, DeserializeDict, Type, PartialEq, Debug)]
        #[zvariant(signature = "a{sv}")]
        struct TestMissing {
            process_id: Option<u32>,
            group_id: Option<u32>,
            user: String,
            quota: u8,
        }
        let decoded: Result<(TestMissing, _)> = encoded.deserialize();
        assert_eq!(
            decoded.unwrap_err(),
            Error::Message("missing field `quota`".to_string())
        );

        #[derive(SerializeDict, DeserializeDict, Type, PartialEq, Debug)]
        #[zvariant(signature = "a{sv}")]
        struct TestSkipUnknown {
            process_id: Option<u32>,
            group_id: Option<u32>,
        }
        let _: TestSkipUnknown = encoded.deserialize().unwrap().0;

        #[derive(SerializeDict, DeserializeDict, Type, PartialEq, Debug)]
        #[zvariant(deny_unknown_fields, signature = "a{sv}")]
        struct TestUnknown {
            process_id: Option<u32>,
            group_id: Option<u32>,
        }
        let decoded: Result<(TestUnknown, _)> = encoded.deserialize();
        assert_eq!(
            decoded.unwrap_err(),
            Error::Message("unknown field `user`, expected `process_id` or `group_id`".to_string())
        );
    }

    #[test]
    fn dict_compare() {
        // the order in which a dict has been constructed must not play a role
        // https://github.com/dbus2/zbus/issues/484
        let mut dict1 = Dict::new(<&str>::signature(), Value::signature());
        dict1.add("first", Value::new("value")).unwrap();
        dict1.add("second", Value::new("value")).unwrap();

        let mut dict2 = Dict::new(<&str>::signature(), Value::signature());
        dict2.add("second", Value::new("value")).unwrap();
        dict2.add("first", Value::new("value")).unwrap();

        assert_eq!(dict1, dict2);
    }

    #[test]
    fn value_value() {
        let ctxt = Context::new_dbus(BE, 0);
        let encoded = to_bytes(ctxt, &0xABBA_ABBA_ABBA_ABBA_u64).unwrap();
        assert_eq!(encoded.len(), 8);
        assert_eq!(LE.read_u64(&encoded), 0xBAAB_BAAB_BAAB_BAAB_u64);
        let decoded: u64 = encoded.deserialize().unwrap().0;
        assert_eq!(decoded, 0xABBA_ABBA_ABBA_ABBA);

        // Lie about there being bytes before
        let ctxt = Context::new_dbus(LE, 2);
        let encoded = to_bytes(ctxt, &0xABBA_ABBA_ABBA_ABBA_u64).unwrap();
        assert_eq!(encoded.len(), 14);
        let decoded: u64 = encoded.deserialize().unwrap().0;
        assert_eq!(decoded, 0xABBA_ABBA_ABBA_ABBA_u64);
        let ctxt = Context::new_dbus(LE, 0);

        // As Value
        let v: Value<'_> = 0xFEFE_u64.into();
        assert_eq!(v.value_signature(), "t");
        let encoded = to_bytes(ctxt, &v).unwrap();
        assert_eq!(encoded.len(), 16);
        let v = encoded.deserialize().unwrap().0;
        assert_eq!(v, Value::U64(0xFEFE));

        // And now as Value in a Value
        let v = Value::Value(Box::new(v));
        let encoded = to_bytes(ctxt, &v).unwrap();
        assert_eq!(encoded.len(), 16);
        let v = encoded.deserialize().unwrap().0;
        if let Value::Value(v) = v {
            assert_eq!(v.value_signature(), "t");
            assert_eq!(*v, Value::U64(0xFEFE));
        } else {
            panic!();
        }

        // Ensure Value works with other Serializer & Deserializer
        let v: Value<'_> = 0xFEFE_u64.into();
        let encoded = serde_json::to_string(&v).unwrap();
        let v = serde_json::from_str::<Value<'_>>(&encoded).unwrap();
        assert_eq!(v, Value::U64(0xFEFE));
    }

    #[test]
    fn enums() {
        use serde::{Deserialize, Serialize};

        #[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
        enum Unit {
            Variant1,
            Variant2,
            Variant3,
        }

        let ctxts_n_expected_lens = [
            // Unit variants are encoded as u32 and that has the same encoding in both formats.
            [
                (Context::new_dbus(BE, 0), 4usize),
                (Context::new_dbus(BE, 1), 7),
                (Context::new_dbus(BE, 2), 6),
                (Context::new_dbus(BE, 3), 5),
                (Context::new_dbus(BE, 4), 4),
            ],
            #[cfg(feature = "gvariant")]
            [
                (Context::new_gvariant(BE, 0), 4usize),
                (Context::new_gvariant(BE, 1), 7),
                (Context::new_gvariant(BE, 2), 6),
                (Context::new_gvariant(BE, 3), 5),
                (Context::new_gvariant(BE, 4), 4),
            ],
        ];
        for ctxts_n_expected_len in ctxts_n_expected_lens {
            for (ctxt, expected_len) in ctxts_n_expected_len {
                let encoded = to_bytes_for_signature(ctxt, "u", &Unit::Variant2).unwrap();
                assert_eq!(encoded.len(), expected_len);
                let decoded: Unit = encoded.deserialize_for_signature("u").unwrap().0;
                assert_eq!(decoded, Unit::Variant2);
            }
        }

        #[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
        enum NewType<'s> {
            Variant1(&'s str),
            Variant2(&'s str),
            Variant3(&'s str),
        }

        let ctxts_n_expected_lens = [
            [
                (Context::new_dbus(BE, 0), 14usize),
                (Context::new_dbus(BE, 1), 21),
                (Context::new_dbus(BE, 2), 20),
                (Context::new_dbus(BE, 3), 19),
                (Context::new_dbus(BE, 4), 18),
            ],
            #[cfg(feature = "gvariant")]
            [
                (Context::new_gvariant(BE, 0), 10usize),
                (Context::new_gvariant(BE, 1), 13),
                (Context::new_gvariant(BE, 2), 12),
                (Context::new_gvariant(BE, 3), 11),
                (Context::new_gvariant(BE, 4), 10),
            ],
        ];
        for ctxts_n_expected_len in ctxts_n_expected_lens {
            for (ctxt, expected_len) in ctxts_n_expected_len {
                let encoded =
                    to_bytes_for_signature(ctxt, "(us)", &NewType::Variant2("hello")).unwrap();
                assert_eq!(encoded.len(), expected_len);
                let decoded: NewType<'_> = encoded.deserialize_for_signature("(us)").unwrap().0;
                assert_eq!(decoded, NewType::Variant2("hello"));
            }
        }

        #[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
        enum Structs {
            Tuple(u8, u32),
            Struct { y: u8, t: u32 },
        }

        let ctxts_n_expected_lens = [
            [
                (Context::new_dbus(BE, 0), 16usize),
                (Context::new_dbus(BE, 1), 23),
                (Context::new_dbus(BE, 2), 22),
                (Context::new_dbus(BE, 3), 21),
                (Context::new_dbus(BE, 4), 20),
            ],
            #[cfg(feature = "gvariant")]
            [
                (Context::new_gvariant(BE, 0), 12usize),
                (Context::new_gvariant(BE, 1), 15),
                (Context::new_gvariant(BE, 2), 14),
                (Context::new_gvariant(BE, 3), 13),
                (Context::new_gvariant(BE, 4), 12),
            ],
        ];
        // TODO: Provide convenience API to create complex signatures
        let signature = "(u(yu))";
        for ctxts_n_expected_len in ctxts_n_expected_lens {
            for (ctxt, expected_len) in ctxts_n_expected_len {
                let encoded =
                    to_bytes_for_signature(ctxt, signature, &Structs::Tuple(42, 42)).unwrap();
                assert_eq!(encoded.len(), expected_len);
                let decoded: Structs = encoded.deserialize_for_signature(signature).unwrap().0;
                assert_eq!(decoded, Structs::Tuple(42, 42));

                let s = Structs::Struct { y: 42, t: 42 };
                let encoded = to_bytes_for_signature(ctxt, signature, &s).unwrap();
                assert_eq!(encoded.len(), expected_len);
                let decoded: Structs = encoded.deserialize_for_signature(signature).unwrap().0;
                assert_eq!(decoded, Structs::Struct { y: 42, t: 42 });
            }
        }
    }

    #[test]
    fn derive() {
        use serde::{Deserialize, Serialize};
        use serde_repr::{Deserialize_repr, Serialize_repr};

        #[derive(Deserialize, Serialize, Type, PartialEq, Debug)]
        struct Struct<'s> {
            field1: u16,
            field2: i64,
            field3: &'s str,
        }

        assert_eq!(Struct::signature(), "(qxs)");
        let s = Struct {
            field1: 0xFF_FF,
            field2: 0xFF_FF_FF_FF_FF_FF,
            field3: "hello",
        };
        let ctxt = Context::new_dbus(LE, 0);
        let encoded = to_bytes(ctxt, &s).unwrap();
        assert_eq!(encoded.len(), 26);
        let decoded: Struct<'_> = encoded.deserialize().unwrap().0;
        assert_eq!(decoded, s);

        #[derive(Deserialize, Serialize, Type)]
        struct UnitStruct;

        assert_eq!(UnitStruct::signature(), <()>::signature());
        let encoded = to_bytes(ctxt, &UnitStruct).unwrap();
        assert_eq!(encoded.len(), 0);
        let _: UnitStruct = encoded.deserialize().unwrap().0;

        #[repr(u8)]
        #[derive(Deserialize_repr, Serialize_repr, Type, Debug, PartialEq)]
        enum Enum {
            Variant1,
            Variant2,
            Variant3,
        }

        assert_eq!(Enum::signature(), u8::signature());
        let encoded = to_bytes(ctxt, &Enum::Variant3).unwrap();
        assert_eq!(encoded.len(), 1);
        let decoded: Enum = encoded.deserialize().unwrap().0;
        assert_eq!(decoded, Enum::Variant3);

        #[repr(i64)]
        #[derive(Deserialize_repr, Serialize_repr, Type, Debug, PartialEq)]
        enum Enum2 {
            Variant1,
            Variant2,
            Variant3,
        }

        assert_eq!(Enum2::signature(), i64::signature());
        let encoded = to_bytes(ctxt, &Enum2::Variant2).unwrap();
        assert_eq!(encoded.len(), 8);
        let decoded: Enum2 = encoded.deserialize().unwrap().0;
        assert_eq!(decoded, Enum2::Variant2);

        #[derive(Deserialize, Serialize, Type, Debug, PartialEq)]
        enum NoReprEnum {
            Variant1,
            Variant2,
            Variant3,
        }

        // issue#265: Panic on deserialization of a structure w/ a unit enum as its last field.
        let encoded = to_bytes(ctxt, &(NoReprEnum::Variant2,)).unwrap();
        let _: (NoReprEnum,) = encoded.deserialize().unwrap().0;

        assert_eq!(NoReprEnum::signature(), u32::signature());
        let encoded = to_bytes(ctxt, &NoReprEnum::Variant2).unwrap();
        assert_eq!(encoded.len(), 4);
        let decoded: NoReprEnum = encoded.deserialize().unwrap().0;
        assert_eq!(decoded, NoReprEnum::Variant2);

        #[derive(Deserialize, Serialize, Type, Debug, PartialEq)]
        #[zvariant(signature = "s")]
        enum StrEnum {
            Variant1,
            Variant2,
            Variant3,
        }

        assert_eq!(StrEnum::signature(), <&str>::signature());
        let encoded = to_bytes(ctxt, &StrEnum::Variant2).unwrap();
        assert_eq!(encoded.len(), 13);
        let decoded: StrEnum = encoded.deserialize().unwrap().0;
        assert_eq!(decoded, StrEnum::Variant2);

        #[derive(Deserialize, Serialize, Type)]
        enum NewType {
            Variant1(f64),
            Variant2(f64),
        }
        assert_eq!(NewType::signature(), "(ud)");

        #[derive(Deserialize, Serialize, Type)]
        enum StructFields {
            Variant1(u16, i64, &'static str),
            Variant2 {
                field1: u16,
                field2: i64,
                field3: &'static str,
            },
        }
        assert_eq!(StructFields::signature(), "(u(qxs))");

        #[derive(Deserialize, Serialize, Type, PartialEq, Debug)]
        struct AStruct<'s> {
            field1: u16,
            field2: &'s [u8],
            field3: &'s [u8],
            field4: i64,
        }
        assert_eq!(AStruct::signature(), "(qayayx)");
        let s = AStruct {
            field1: 0xFF_FF,
            field2: &[77u8; 8],
            field3: &[77u8; 8],
            field4: 0xFF_FF_FF_FF_FF_FF,
        };
        let encoded = to_bytes(ctxt, &s).unwrap();
        assert_eq!(encoded.len(), 40);
        let decoded: AStruct<'_> = encoded.deserialize().unwrap().0;
        assert_eq!(decoded, s);
    }

    #[test]
    fn serialized_size() {
        let ctxt = Context::new_dbus(LE, 0);
        let l = crate::serialized_size(ctxt, &()).unwrap();
        assert_eq!(*l, 0);

        #[cfg(unix)]
        {
            let stdout = std::io::stdout();
            let l = crate::serialized_size(ctxt, &Fd::from(&stdout)).unwrap();
            assert_eq!(*l, 4);
            assert_eq!(l.num_fds(), 1);
        }

        let l = crate::serialized_size(ctxt, &('a', "abc", &(1_u32, 2))).unwrap();
        assert_eq!(*l, 24);

        let v = vec![1, 2];
        let l = crate::serialized_size(ctxt, &('a', "abc", &v)).unwrap();
        assert_eq!(*l, 28);
    }

    #[test]
    #[cfg(feature = "serde_bytes")]
    fn serde_bytes() {
        use serde::{Deserialize, Serialize};
        use serde_bytes::*;

        let ctxt = Context::new_dbus(LE, 0);
        let ay = Bytes::new(&[77u8; 1_000_000]);
        let encoded = to_bytes(ctxt, &ay).unwrap();
        assert_eq!(encoded.len(), 1_000_004);
        let decoded: ByteBuf = encoded.deserialize().unwrap().0;
        assert_eq!(decoded.len(), 1_000_000);

        #[derive(Deserialize, Serialize, Type, PartialEq, Debug)]
        struct Struct<'s> {
            field1: u16,
            #[serde(with = "serde_bytes")]
            field2: &'s [u8],
            field3: i64,
        }
        assert_eq!(Struct::signature(), "(qayx)");
        let s = Struct {
            field1: 0xFF_FF,
            field2: &[77u8; 512],
            field3: 0xFF_FF_FF_FF_FF_FF,
        };
        let encoded = to_bytes(ctxt, &s).unwrap();
        assert_eq!(encoded.len(), 528);
        let decoded: Struct<'_> = encoded.deserialize().unwrap().0;
        assert_eq!(decoded, s);
    }

    #[test]
    #[cfg(all(feature = "serde_bytes", feature = "gvariant"))]
    fn serde_bytes_gvariant() {
        use serde::{Deserialize, Serialize};
        use serde_bytes::*;

        let ctxt = Context::new_gvariant(LE, 0);
        let ay = Bytes::new(&[77u8; 1_000_000]);
        let encoded = to_bytes(ctxt, &ay).unwrap();
        assert_eq!(encoded.len(), 1_000_000);
        let decoded: ByteBuf = encoded.deserialize().unwrap().0;
        assert_eq!(decoded.len(), 1_000_000);

        #[derive(Deserialize, Serialize, Type, PartialEq, Debug)]
        struct Struct<'s> {
            field1: u16,
            #[serde(with = "serde_bytes")]
            field2: &'s [u8],
            field3: i64,
        }
        assert_eq!(Struct::signature(), "(qayx)");
        let s = Struct {
            field1: 0xFF_FF,
            field2: &[77u8; 512],
            field3: 0xFF_FF_FF_FF_FF_FF,
        };
        let encoded = to_bytes(ctxt, &s).unwrap();
        assert_eq!(encoded.len(), 530);
        let decoded: Struct<'_> = encoded.deserialize().unwrap().0;
        assert_eq!(decoded, s);
    }

    #[test]
    #[cfg(any(feature = "gvariant", feature = "option-as-array"))]
    fn option_value() {
        #[cfg(all(feature = "gvariant", not(feature = "option-as-array")))]
        let ctxt = Context::new_gvariant(NATIVE_ENDIAN, 0);
        #[cfg(feature = "option-as-array")]
        let ctxt = Context::new_dbus(NATIVE_ENDIAN, 0);

        // First a Some fixed-sized value
        let mn = Some(16i16);
        let encoded = to_bytes(ctxt, &mn).unwrap();
        #[cfg(all(feature = "gvariant", not(feature = "option-as-array")))]
        assert_eq!(encoded.len(), 2);
        #[cfg(feature = "option-as-array")]
        assert_eq!(encoded.len(), 6);
        let decoded: Option<i16> = encoded.deserialize().unwrap().0;
        assert_eq!(decoded, mn);

        #[cfg(all(feature = "gvariant", not(feature = "option-as-array")))]
        {
            // Check encoding against GLib
            let bytes = Bytes::from_owned(encoded);
            let variant = Variant::from_bytes::<Option<i16>>(&bytes);
            assert_eq!(variant.get::<Option<i16>>().unwrap(), mn);
        }

        // As Value
        let v: Value<'_> = mn.into();
        let encoded = to_bytes(ctxt, &v).unwrap();
        #[cfg(all(feature = "gvariant", not(feature = "option-as-array")))]
        assert_eq!(encoded.len(), 5);
        #[cfg(feature = "option-as-array")]
        assert_eq!(encoded.len(), 10);
        let decoded: Value<'_> = encoded.deserialize().unwrap().0;
        match decoded {
            #[cfg(all(feature = "gvariant", not(feature = "option-as-array")))]
            Value::Maybe(maybe) => assert_eq!(maybe.get().unwrap(), mn),
            #[cfg(feature = "option-as-array")]
            Value::Array(array) => {
                assert_eq!(i16::try_from(array[0].try_clone().unwrap()).unwrap(), 16i16)
            }
            _ => panic!("unexpected value {decoded:?}"),
        }

        #[cfg(all(feature = "gvariant", not(feature = "option-as-array")))]
        {
            // Check encoding against GLib
            let bytes = Bytes::from_owned(encoded);
            let variant = Variant::from_bytes::<Variant>(&bytes);
            let decoded = variant.child_value(0).get::<Option<i16>>().unwrap();
            assert_eq!(decoded, mn);
        }

        // Now a None of the same type
        let mn: Option<i16> = None;
        let encoded = to_bytes(ctxt, &mn).unwrap();
        #[cfg(all(feature = "gvariant", not(feature = "option-as-array")))]
        assert_eq!(encoded.len(), 0);
        #[cfg(feature = "option-as-array")]
        assert_eq!(encoded.len(), 4);
        let decoded: Option<i16> = encoded.deserialize().unwrap().0;
        assert!(decoded.is_none());

        #[cfg(all(feature = "gvariant", not(feature = "option-as-array")))]
        {
            // Check encoding against GLib
            let bytes = Bytes::from_owned(encoded);
            let variant = Variant::from_bytes::<Option<i16>>(&bytes);
            assert!(variant.get::<Option<i16>>().unwrap().is_none());
        }

        // Next a Some variable-sized value
        let ms = Some("hello world");
        let encoded = to_bytes(ctxt, &ms).unwrap();
        #[cfg(all(feature = "gvariant", not(feature = "option-as-array")))]
        assert_eq!(encoded.len(), 13);
        #[cfg(feature = "option-as-array")]
        assert_eq!(encoded.len(), 20);
        let decoded: Option<&str> = encoded.deserialize().unwrap().0;
        assert_eq!(decoded, ms);

        #[cfg(all(feature = "gvariant", not(feature = "option-as-array")))]
        {
            // Check encoding against GLib
            let bytes = Bytes::from_owned(encoded);
            let variant = Variant::from_bytes::<Option<String>>(&bytes);
            assert_eq!(
                &variant.get::<Option<String>>().unwrap().unwrap(),
                ms.unwrap()
            );
        }

        // As Value
        let v: Value<'_> = ms.into();
        #[cfg(feature = "option-as-array")]
        match &v {
            Value::Array(array) => {
                assert_eq!(
                    String::try_from(array[0].try_clone().unwrap()).unwrap(),
                    ms.unwrap()
                )
            }
            _ => panic!("unexpected value {v:?}"),
        }

        let encoded = to_bytes(ctxt, &v).unwrap();
        #[cfg(all(feature = "gvariant", not(feature = "option-as-array")))]
        assert_eq!(encoded.len(), 16);
        #[cfg(feature = "option-as-array")]
        assert_eq!(encoded.len(), 24);
        let decoded: Value<'_> = encoded.deserialize().unwrap().0;
        match decoded {
            #[cfg(all(feature = "gvariant", not(feature = "option-as-array")))]
            Value::Maybe(maybe) => {
                assert_eq!(maybe.get::<String>().unwrap().as_deref(), ms);
            }
            #[cfg(feature = "option-as-array")]
            Value::Array(array) => {
                assert_eq!(
                    String::try_from(array[0].try_clone().unwrap()).unwrap(),
                    ms.unwrap()
                )
            }
            _ => panic!("unexpected value {decoded:?}"),
        }

        #[cfg(all(feature = "gvariant", not(feature = "option-as-array")))]
        {
            // Check encoding against GLib
            let bytes = Bytes::from_owned(encoded);
            let variant = Variant::from_bytes::<Variant>(&bytes);
            let decoded = variant.child_value(0).get::<Option<String>>().unwrap();
            assert_eq!(decoded.as_deref(), ms);
        }

        // Now a None of the same type
        let ms: Option<&str> = None;
        let encoded = to_bytes(ctxt, &ms).unwrap();
        #[cfg(all(feature = "gvariant", not(feature = "option-as-array")))]
        assert_eq!(encoded.len(), 0);
        #[cfg(feature = "option-as-array")]
        assert_eq!(encoded.len(), 4);
        let decoded: Option<&str> = encoded.deserialize().unwrap().0;
        assert!(decoded.is_none());

        #[cfg(all(feature = "gvariant", not(feature = "option-as-array")))]
        {
            // Check encoding against GLib
            let bytes = Bytes::from_owned(encoded);
            let variant = Variant::from_bytes::<Option<String>>(&bytes);
            assert!(variant.get::<Option<String>>().unwrap().is_none());
        }

        // In a seq type
        let ams = vec![
            Some(String::from("hello world")),
            Some(String::from("bye world")),
        ];
        let encoded = to_bytes(ctxt, &ams).unwrap();
        #[cfg(all(feature = "gvariant", not(feature = "option-as-array")))]
        assert_eq!(encoded.len(), 26);
        #[cfg(feature = "option-as-array")]
        assert_eq!(encoded.len(), 42);
        let decoded: Vec<Option<String>> = encoded.deserialize().unwrap().0;
        assert_eq!(decoded, ams);

        #[cfg(all(feature = "gvariant", not(feature = "option-as-array")))]
        {
            // Check encoding against GLib
            let bytes = Bytes::from_owned(encoded);
            let variant = Variant::from_bytes::<Vec<Option<String>>>(&bytes);
            let decoded = variant.get::<Vec<Option<String>>>().unwrap();
            assert_eq!(decoded, ams);
        }

        // As Value
        let v: Value<'_> = ams.clone().into();
        let encoded = to_bytes(ctxt, &v).unwrap();
        #[cfg(all(feature = "gvariant", not(feature = "option-as-array")))]
        assert_eq!(encoded.len(), 30);
        #[cfg(feature = "option-as-array")]
        assert_eq!(encoded.len(), 50);
        let decoded: Value<'_> = encoded.deserialize().unwrap().0;
        assert_eq!(v, decoded);

        #[cfg(all(feature = "gvariant", not(feature = "option-as-array")))]
        {
            // Check encoding against GLib
            let bytes = Bytes::from_owned(encoded);
            let variant = Variant::from_bytes::<Variant>(&bytes);
            let decoded = variant.child_value(0).get::<Vec<Option<String>>>().unwrap();
            assert_eq!(decoded, ams);
        }

        // In a struct
        let structure: (Option<String>, u64, Option<String>) =
            (Some(String::from("hello world")), 42u64, None);
        let encoded = to_bytes(ctxt, &structure).unwrap();
        #[cfg(all(feature = "gvariant", not(feature = "option-as-array")))]
        assert_eq!(encoded.len(), 25);
        #[cfg(feature = "option-as-array")]
        assert_eq!(encoded.len(), 36);
        let decoded: (Option<String>, u64, Option<String>) = encoded.deserialize().unwrap().0;
        assert_eq!(decoded, structure);

        #[cfg(all(feature = "gvariant", not(feature = "option-as-array")))]
        {
            // Check encoding against GLib
            let bytes = Bytes::from_owned(encoded);
            let variant = Variant::from_bytes::<(Option<String>, u64, Option<String>)>(&bytes);
            let decoded = variant
                .get::<(Option<String>, u64, Option<String>)>()
                .unwrap();
            assert_eq!(decoded, structure);
        }

        // As Value
        let v: Value<'_> = structure.clone().into();
        let encoded = to_bytes(ctxt, &v).unwrap();
        #[cfg(all(feature = "gvariant", not(feature = "option-as-array")))]
        assert_eq!(encoded.len(), 33);
        #[cfg(feature = "option-as-array")]
        assert_eq!(encoded.len(), 52);
        let decoded: Value<'_> = encoded.deserialize().unwrap().0;
        assert_eq!(v, decoded);

        #[cfg(all(feature = "gvariant", not(feature = "option-as-array")))]
        {
            // Check encoding against GLib
            let bytes = Bytes::from_owned(encoded);
            let variant = Variant::from_bytes::<Variant>(&bytes);
            let decoded = variant
                .child_value(0)
                .get::<(Option<String>, u64, Option<String>)>()
                .unwrap();
            assert_eq!(decoded, structure);
        }
    }

    #[test]
    fn struct_with_hashmap() {
        use serde::{Deserialize, Serialize};

        let mut hmap = HashMap::new();
        hmap.insert("key".into(), "value".into());

        #[derive(Type, Deserialize, Serialize, PartialEq, Debug)]
        struct Foo {
            hmap: HashMap<String, String>,
        }

        let foo = Foo { hmap };
        assert_eq!(Foo::signature(), "(a{ss})");

        let ctxt = Context::new_dbus(LE, 0);
        let encoded = to_bytes(ctxt, &(&foo, 1)).unwrap();
        let f: Foo = encoded.deserialize().unwrap().0;
        assert_eq!(f, foo);
    }

    #[test]
    fn issue_59() {
        // Ensure we don't panic on deserializing tuple of smaller than expected length.
        let ctxt = Context::new_dbus(LE, 0);
        let encoded = to_bytes(ctxt, &("hello",)).unwrap();
        let result: Result<((&str, &str), _)> = encoded.deserialize();
        assert!(result.is_err());
    }

    #[test]
    #[cfg(feature = "gvariant")]
    fn issue_99() {
        #[derive(Deserialize, Serialize, Type, PartialEq, Debug)]
        struct ZVStruct<'s>(#[serde(borrow)] HashMap<&'s str, Value<'s>>);

        let mut dict = HashMap::new();
        dict.insert("hi", Value::from("hello"));
        dict.insert("bye", Value::from("then"));

        let element = ZVStruct(dict);

        let ctxt = Context::new_gvariant(LE, 0);
        let signature = ZVStruct::signature();

        let encoded = to_bytes_for_signature(ctxt, &signature, &element).unwrap();
        let _: ZVStruct<'_> = encoded.deserialize_for_signature(signature).unwrap().0;
    }

    #[test]
    fn ip_addr() {
        let ctxt = Context::new_dbus(LE, 0);

        // First the bare specific types.
        let localhost_v4 = Ipv4Addr::new(127, 0, 0, 1);
        let encoded = to_bytes(ctxt, &localhost_v4).unwrap();
        let decoded: Ipv4Addr = encoded.deserialize().unwrap().0;
        assert_eq!(localhost_v4, decoded);

        let localhost_v6 = Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1);
        let encoded = to_bytes(ctxt, &localhost_v6).unwrap();
        let decoded: Ipv6Addr = encoded.deserialize().unwrap().0;
        assert_eq!(localhost_v6, decoded);

        // Now wrapper under the generic IpAddr.
        let localhost_v4 = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
        let encoded = to_bytes(ctxt, &localhost_v4).unwrap();
        let decoded: IpAddr = encoded.deserialize().unwrap().0;
        assert_eq!(localhost_v4, decoded);

        let localhost_v6 = IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1));
        let encoded = to_bytes(ctxt, &localhost_v6).unwrap();
        let decoded: IpAddr = encoded.deserialize().unwrap().0;
        assert_eq!(localhost_v6, decoded);
    }

    #[cfg(feature = "ostree-tests")]
    #[test]
    fn ostree_de() {
        #[derive(Deserialize, Serialize, Type, PartialEq, Debug)]
        struct Summary<'a>(Vec<Repo<'a>>, #[serde(borrow)] HashMap<&'a str, Value<'a>>);

        #[derive(Deserialize, Serialize, Type, PartialEq, Debug)]
        struct Repo<'a>(&'a str, #[serde(borrow)] Metadata<'a>);

        #[derive(Deserialize, Serialize, Type, PartialEq, Debug)]
        struct Metadata<'a>(u64, Vec<u8>, #[serde(borrow)] HashMap<&'a str, Value<'a>>);

        let encoded = std::fs::read("../test-data/flatpak-summary.dump").unwrap();
        let ctxt = Context::new_gvariant(LE, 0);
        let encoded = Data::new(encoded, ctxt);
        let _: Summary<'_> = encoded.deserialize().unwrap().0;
        // If we're able to deserialize all the data successfully, don't bother checking the summary
        // data.
    }

    #[test]
    #[cfg(feature = "time")]
    fn time() {
        // time::Date
        let date = time::Date::from_calendar_date(2011, time::Month::June, 21).unwrap();
        let ctxt = Context::new_dbus(LE, 0);
        let encoded = to_bytes(ctxt, &date).unwrap();
        let decoded: time::Date = encoded.deserialize().unwrap().0;
        assert_eq!(date, decoded);

        // time::Duration
        let duration = time::Duration::new(42, 123456789);
        let ctxt = Context::new_dbus(LE, 0);
        let encoded = to_bytes(ctxt, &duration).unwrap();
        let decoded: time::Duration = encoded.deserialize().unwrap().0;
        assert_eq!(duration, decoded);

        // time::OffsetDateTime
        let offset = time::OffsetDateTime::now_utc();
        let ctxt = Context::new_dbus(LE, 0);
        let encoded = to_bytes(ctxt, &offset).unwrap();
        let decoded: time::OffsetDateTime = encoded.deserialize().unwrap().0;
        assert_eq!(offset, decoded);

        // time::Time
        let time = time::Time::from_hms(23, 42, 59).unwrap();
        let ctxt = Context::new_dbus(LE, 0);
        let encoded = to_bytes(ctxt, &time).unwrap();
        let decoded: time::Time = encoded.deserialize().unwrap().0;
        assert_eq!(time, decoded);

        // time::PrimitiveDateTime
        let date = time::PrimitiveDateTime::new(date, time);
        let ctxt = Context::new_dbus(LE, 0);
        let encoded = to_bytes(ctxt, &date).unwrap();
        let decoded: time::PrimitiveDateTime = encoded.deserialize().unwrap().0;
        assert_eq!(date, decoded);
    }

    #[test]
    fn recursion_limits() {
        let ctxt = Context::new_dbus(LE, 0);
        // Total container depth exceeds limit (64)
        let mut value = Value::from(0u8);
        for _ in 0..64 {
            value = Value::Value(Box::new(value));
        }
        assert!(matches!(
            to_bytes(ctxt, &value),
            Err(Error::MaxDepthExceeded(MaxDepthExceeded::Container))
        ));

        // Array depth exceeds limit (32)
        let vec = vec![vec![vec![vec![vec![vec![vec![vec![vec![vec![vec![
            vec![vec![vec![vec![vec![vec![vec![vec![vec![vec![vec![
                vec![vec![vec![vec![vec![vec![vec![vec![vec![vec![vec![
                    0u8,
                ]]]]]]]]]]],
            ]]]]]]]]]]],
        ]]]]]]]]]]];
        assert!(matches!(
            to_bytes(ctxt, &vec),
            Err(Error::MaxDepthExceeded(MaxDepthExceeded::Array))
        ));

        // Struct depth exceeds limit (32)
        let tuple = ((((((((((((((((((((((
            (((((((((((0u8,),),),),),),),),),),),
        ),),),),),),),),),),),),),),),),),),),),),);
        assert!(matches!(
            to_bytes(ctxt, &tuple),
            Err(Error::MaxDepthExceeded(MaxDepthExceeded::Structure))
        ));

        // total depth exceeds limit (64) with struct, array and variant.
        let mut value = Value::from(0u8);
        for _ in 0..32 {
            value = Value::Value(Box::new(value));
        }
        let tuple_array =
            (
                ((((((((((((((((vec![vec![vec![vec![vec![vec![vec![vec![
                    vec![vec![vec![vec![vec![vec![vec![vec![value]]]]]]]],
                ]]]]]]]],),),),),),),),),),),),),),),),),
            );
        assert!(matches!(
            to_bytes(ctxt, &tuple_array),
            Err(Error::MaxDepthExceeded(MaxDepthExceeded::Container))
        ));

        // TODO:
        //
        // * Test deserializers.
        // * Test gvariant format.
    }
}
