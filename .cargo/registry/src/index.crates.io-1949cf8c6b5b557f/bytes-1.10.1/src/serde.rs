use super::{Bytes, BytesMut};
use alloc::string::String;
use alloc::vec::Vec;
use core::{cmp, fmt};
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};

macro_rules! serde_impl {
    ($ty:ident, $visitor_ty:ident, $from_slice:ident, $from_vec:ident) => {
        impl Serialize for $ty {
            #[inline]
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                serializer.serialize_bytes(&self)
            }
        }

        struct $visitor_ty;

        impl<'de> de::Visitor<'de> for $visitor_ty {
            type Value = $ty;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("byte array")
            }

            #[inline]
            fn visit_seq<V>(self, mut seq: V) -> Result<Self::Value, V::Error>
            where
                V: de::SeqAccess<'de>,
            {
                let len = cmp::min(seq.size_hint().unwrap_or(0), 4096);
                let mut values: Vec<u8> = Vec::with_capacity(len);

                while let Some(value) = seq.next_element()? {
                    values.push(value);
                }

                Ok($ty::$from_vec(values))
            }

            #[inline]
            fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok($ty::$from_slice(v))
            }

            #[inline]
            fn visit_byte_buf<E>(self, v: Vec<u8>) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok($ty::$from_vec(v))
            }

            #[inline]
            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok($ty::$from_slice(v.as_bytes()))
            }

            #[inline]
            fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok($ty::$from_vec(v.into_bytes()))
            }
        }

        impl<'de> Deserialize<'de> for $ty {
            #[inline]
            fn deserialize<D>(deserializer: D) -> Result<$ty, D::Error>
            where
                D: Deserializer<'de>,
            {
                deserializer.deserialize_byte_buf($visitor_ty)
            }
        }
    };
}

serde_impl!(Bytes, BytesVisitor, copy_from_slice, from);
serde_impl!(BytesMut, BytesMutVisitor, from, from_vec);
