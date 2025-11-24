use crate::_alloc_prelude::*;
use crate::{JsonSchema, Schema, SchemaGenerator};
use alloc::borrow::Cow;
use core::num::*;

macro_rules! nonzero_signed_impl {
    ($type:ty => $primitive:ty) => {
        impl JsonSchema for $type {
            inline_schema!();

            fn schema_name() -> Cow<'static, str> {
                stringify!($type).into()
            }

            fn schema_id() -> Cow<'static, str> {
                stringify!(std::num::$type).into()
            }

            fn json_schema(generator: &mut SchemaGenerator) -> Schema {
                let mut schema = <$primitive>::json_schema(generator);
                schema.insert("not".to_owned(), serde_json::json!({
                    "const": 0
                }));
                schema
            }
        }
    };
}

nonzero_signed_impl!(NonZeroI8 => i8);
nonzero_signed_impl!(NonZeroI16 => i16);
nonzero_signed_impl!(NonZeroI32 => i32);
nonzero_signed_impl!(NonZeroI64 => i64);
nonzero_signed_impl!(NonZeroI128 => i128);
nonzero_signed_impl!(NonZeroIsize => isize);

macro_rules! nonzero_unsigned_impl {
    ($type:ty => $primitive:ty) => {
        impl JsonSchema for $type {
            inline_schema!();

            fn schema_name() -> Cow<'static, str> {
                stringify!($type).into()
            }

            fn schema_id() -> Cow<'static, str> {
                stringify!(std::num::$type).into()
            }

            fn json_schema(generator: &mut SchemaGenerator) -> Schema {
                let mut schema = <$primitive>::json_schema(generator);
                schema.insert("minimum".to_owned(), 1.into());
                schema
            }
        }
    };
}

nonzero_unsigned_impl!(NonZeroU8 => u8);
nonzero_unsigned_impl!(NonZeroU16 => u16);
nonzero_unsigned_impl!(NonZeroU32 => u32);
nonzero_unsigned_impl!(NonZeroU64 => u64);
nonzero_unsigned_impl!(NonZeroU128 => u128);
nonzero_unsigned_impl!(NonZeroUsize => usize);
