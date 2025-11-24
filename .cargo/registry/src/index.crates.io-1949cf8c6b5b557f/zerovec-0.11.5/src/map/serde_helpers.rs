// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

// @@@@@@@@@@@@@@@@
// THIS FILE IS SHARED BETWEEN LITEMAP AND ZEROVEC. PLEASE KEEP IT IN SYNC FOR ALL EDITS
// @@@@@@@@@@@@@@@@

use serde::ser::{Impossible, Serialize, Serializer};

pub fn is_num_or_string<T: Serialize + ?Sized>(k: &T) -> bool {
    // Serializer that errors in the same cases as serde_json::ser::MapKeySerializer
    struct MapKeySerializerDryRun;
    impl Serializer for MapKeySerializerDryRun {
        type Ok = ();
        // Singleton error type that implements serde::ser::Error
        type Error = core::fmt::Error;

        type SerializeSeq = Impossible<(), Self::Error>;
        type SerializeTuple = Impossible<(), Self::Error>;
        type SerializeTupleStruct = Impossible<(), Self::Error>;
        type SerializeTupleVariant = Impossible<(), Self::Error>;
        type SerializeMap = Impossible<(), Self::Error>;
        type SerializeStruct = Impossible<(), Self::Error>;
        type SerializeStructVariant = Impossible<(), Self::Error>;

        fn serialize_str(self, _value: &str) -> Result<Self::Ok, Self::Error> {
            Ok(())
        }
        fn serialize_unit_variant(
            self,
            _name: &'static str,
            _variant_index: u32,
            _variant: &'static str,
        ) -> Result<Self::Ok, Self::Error> {
            Ok(())
        }
        fn serialize_newtype_struct<T: Serialize + ?Sized>(
            self,
            _name: &'static str,
            value: &T,
        ) -> Result<Self::Ok, Self::Error> {
            // Recurse
            value.serialize(self)
        }
        fn serialize_bool(self, _value: bool) -> Result<Self::Ok, Self::Error> {
            Err(core::fmt::Error)
        }
        fn serialize_i8(self, _value: i8) -> Result<Self::Ok, Self::Error> {
            Ok(())
        }
        fn serialize_i16(self, _value: i16) -> Result<Self::Ok, Self::Error> {
            Ok(())
        }
        fn serialize_i32(self, _value: i32) -> Result<Self::Ok, Self::Error> {
            Ok(())
        }
        fn serialize_i64(self, _value: i64) -> Result<Self::Ok, Self::Error> {
            Ok(())
        }
        fn serialize_i128(self, _value: i128) -> Result<Self::Ok, Self::Error> {
            Ok(())
        }
        fn serialize_u8(self, _value: u8) -> Result<Self::Ok, Self::Error> {
            Ok(())
        }
        fn serialize_u16(self, _value: u16) -> Result<Self::Ok, Self::Error> {
            Ok(())
        }
        fn serialize_u32(self, _value: u32) -> Result<Self::Ok, Self::Error> {
            Ok(())
        }
        fn serialize_u64(self, _value: u64) -> Result<Self::Ok, Self::Error> {
            Ok(())
        }
        fn serialize_u128(self, _value: u128) -> Result<Self::Ok, Self::Error> {
            Ok(())
        }
        fn serialize_f32(self, _value: f32) -> Result<Self::Ok, Self::Error> {
            Err(core::fmt::Error)
        }
        fn serialize_f64(self, _value: f64) -> Result<Self::Ok, Self::Error> {
            Err(core::fmt::Error)
        }
        fn serialize_char(self, _value: char) -> Result<Self::Ok, Self::Error> {
            Ok(())
        }
        fn serialize_bytes(self, _value: &[u8]) -> Result<Self::Ok, Self::Error> {
            Err(core::fmt::Error)
        }
        fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
            Err(core::fmt::Error)
        }
        fn serialize_unit_struct(self, _name: &'static str) -> Result<Self::Ok, Self::Error> {
            Err(core::fmt::Error)
        }
        fn serialize_newtype_variant<T: Serialize + ?Sized>(
            self,
            _name: &'static str,
            _variant_index: u32,
            _variant: &'static str,
            _value: &T,
        ) -> Result<Self::Ok, Self::Error> {
            Err(core::fmt::Error)
        }
        fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
            Err(core::fmt::Error)
        }
        fn serialize_some<T: Serialize + ?Sized>(
            self,
            _value: &T,
        ) -> Result<Self::Ok, Self::Error> {
            Err(core::fmt::Error)
        }
        fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
            Err(core::fmt::Error)
        }
        fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple, Self::Error> {
            Err(core::fmt::Error)
        }
        fn serialize_tuple_struct(
            self,
            _name: &'static str,
            _len: usize,
        ) -> Result<Self::SerializeTupleStruct, Self::Error> {
            Err(core::fmt::Error)
        }
        fn serialize_tuple_variant(
            self,
            _name: &'static str,
            _variant_index: u32,
            _variant: &'static str,
            _len: usize,
        ) -> Result<Self::SerializeTupleVariant, Self::Error> {
            Err(core::fmt::Error)
        }
        fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
            Err(core::fmt::Error)
        }
        fn serialize_struct(
            self,
            _name: &'static str,
            _len: usize,
        ) -> Result<Self::SerializeStruct, Self::Error> {
            Err(core::fmt::Error)
        }
        fn serialize_struct_variant(
            self,
            _name: &'static str,
            _variant_index: u32,
            _variant: &'static str,
            _len: usize,
        ) -> Result<Self::SerializeStructVariant, Self::Error> {
            Err(core::fmt::Error)
        }
        fn collect_str<T: core::fmt::Display + ?Sized>(
            self,
            _value: &T,
        ) -> Result<Self::Ok, Self::Error> {
            Ok(())
        }
    }
    k.serialize(MapKeySerializerDryRun).is_ok()
}
