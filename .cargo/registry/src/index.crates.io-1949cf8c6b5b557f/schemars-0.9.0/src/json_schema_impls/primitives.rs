use crate::SchemaGenerator;
use crate::_alloc_prelude::*;
use crate::{json_schema, JsonSchema, Schema};
use alloc::borrow::Cow;

macro_rules! simple_impl {
    ($type:ty => $instance_type:literal) => {
        impl JsonSchema for $type {
            inline_schema!();

            fn schema_name() -> Cow<'static, str> {
                $instance_type.into()
            }

            fn json_schema(_: &mut SchemaGenerator) -> Schema {
                json_schema!({
                    "type": $instance_type
                })
            }
        }
    };
    ($type:ty => $instance_type:literal, $format:literal) => {
        impl JsonSchema for $type {
            inline_schema!();

            fn schema_name() -> Cow<'static, str> {
                $format.into()
            }

            fn json_schema(_: &mut SchemaGenerator) -> Schema {
                json_schema!({
                    "type": $instance_type,
                    "format": $format
                })
            }
        }
    };
}

macro_rules! ranged_impl {
    ($type:ty => $instance_type:literal, $format:literal) => {
        impl JsonSchema for $type {
            inline_schema!();

            fn schema_name() -> Cow<'static, str> {
                $format.into()
            }

            fn json_schema(_: &mut SchemaGenerator) -> Schema {
                json_schema!({
                    "type": $instance_type,
                    "format": $format,
                    "minimum": <$type>::MIN,
                    "maximum": <$type>::MAX
                })
            }
        }
    };
}

simple_impl!(str => "string");
simple_impl!(String => "string");
simple_impl!(bool => "boolean");
simple_impl!(f32 => "number", "float");
simple_impl!(f64 => "number", "double");
ranged_impl!(i8 => "integer", "int8");
ranged_impl!(i16 => "integer", "int16");
simple_impl!(i32 => "integer", "int32");
simple_impl!(i64 => "integer", "int64");
simple_impl!(i128 => "integer", "int128");
simple_impl!(isize => "integer", "int");
simple_impl!(() => "null");

#[cfg(feature = "std")]
mod std_types {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6};
    use std::path::{Path, PathBuf};

    simple_impl!(Path => "string");
    simple_impl!(PathBuf => "string");

    simple_impl!(Ipv4Addr => "string", "ipv4");
    simple_impl!(Ipv6Addr => "string", "ipv6");
    simple_impl!(IpAddr => "string", "ip");

    simple_impl!(SocketAddr => "string");
    simple_impl!(SocketAddrV4 => "string");
    simple_impl!(SocketAddrV6 => "string");
}

macro_rules! unsigned_impl {
    ($type:ty => $instance_type:literal, $format:literal) => {
        impl JsonSchema for $type {
            inline_schema!();

            fn schema_name() -> Cow<'static, str> {
                $format.into()
            }

            fn json_schema(_: &mut SchemaGenerator) -> Schema {
                json_schema!({
                    "type": $instance_type,
                    "format": $format,
                    "minimum": 0
                })
            }
        }
    };
}

ranged_impl!(u8 => "integer", "uint8");
ranged_impl!(u16 => "integer", "uint16");
unsigned_impl!(u32 => "integer", "uint32");
unsigned_impl!(u64 => "integer", "uint64");
unsigned_impl!(u128 => "integer", "uint128");
unsigned_impl!(usize => "integer", "uint");

impl JsonSchema for char {
    inline_schema!();

    fn schema_name() -> Cow<'static, str> {
        "Character".into()
    }

    fn schema_id() -> Cow<'static, str> {
        "char".into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "type": "string",
            "minLength": 1,
            "maxLength": 1,
        })
    }
}
