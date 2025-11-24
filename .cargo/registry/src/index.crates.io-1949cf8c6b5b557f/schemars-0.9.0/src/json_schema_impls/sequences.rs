use crate::SchemaGenerator;
use crate::_alloc_prelude::*;
use crate::{json_schema, JsonSchema, Schema};
use alloc::borrow::Cow;

macro_rules! seq_impl {
    ($($desc:tt)+) => {
        impl $($desc)+
        where
            T: JsonSchema,
        {
            inline_schema!();

            fn schema_name() -> Cow<'static, str> {
                format!("Array_of_{}", T::schema_name()).into()
            }

            fn schema_id() -> Cow<'static, str> {
                format!("[{}]", T::schema_id()).into()
            }

            fn json_schema(generator: &mut SchemaGenerator) -> Schema {
                json_schema!({
                    "type": "array",
                    "items": generator.subschema_for::<T>(),
                })
            }
        }
    };
}

macro_rules! set_impl {
    ($($desc:tt)+) => {
        impl $($desc)+
        where
            T: JsonSchema,
        {
            inline_schema!();

            fn schema_name() -> Cow<'static, str> {
                format!("Set_of_{}", T::schema_name()).into()
            }

            fn schema_id() -> Cow<'static, str> {
                format!("Set<{}>", T::schema_id()).into()
            }

            fn json_schema(generator: &mut SchemaGenerator) -> Schema {
                json_schema!({
                    "type": "array",
                    "uniqueItems": true,
                    "items": generator.subschema_for::<T>(),
                })
            }
        }
    };
}

seq_impl!(<T> JsonSchema for alloc::collections::BinaryHeap<T>);
seq_impl!(<T> JsonSchema for alloc::collections::LinkedList<T>);
seq_impl!(<T> JsonSchema for [T]);
seq_impl!(<T> JsonSchema for alloc::vec::Vec<T>);
seq_impl!(<T> JsonSchema for alloc::collections::VecDeque<T>);

set_impl!(<T> JsonSchema for alloc::collections::BTreeSet<T>);

#[cfg(feature = "std")]
set_impl!(<T, H> JsonSchema for std::collections::HashSet<T, H>);
