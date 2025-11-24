use crate::{json_schema, JsonSchema, Schema, SchemaGenerator};
use alloc::borrow::Cow;
use jiff02::civil::{Date, DateTime, Time};
use jiff02::{SignedDuration, Span, Timestamp, Zoned};

macro_rules! formatted_string_impl {
    ($ty:ident, $format:literal) => {
        formatted_string_impl!($ty, $format, JsonSchema for $ty);
    };
    ($ty:ident, $format:literal, $($desc:tt)+) => {
        impl $($desc)+ {
            inline_schema!();

            fn schema_name() -> Cow<'static, str> {
                stringify!($ty).into()
            }

            fn schema_id() -> Cow<'static, str>  {
                stringify!(jiff::$ty).into()
            }

            fn json_schema(_: &mut SchemaGenerator) -> Schema {
                json_schema!({
                    "type": "string",
                    "format": $format
                })
            }
        }
    };
}

formatted_string_impl!(SignedDuration, "duration");
formatted_string_impl!(Span, "duration");
formatted_string_impl!(Timestamp, "date-time");
formatted_string_impl!(Zoned, "zoned-date-time");
formatted_string_impl!(Date, "date");
formatted_string_impl!(Time, "partial-time");
formatted_string_impl!(DateTime, "partial-date-time");
