use crate::SchemaGenerator;
use crate::_alloc_prelude::*;
use crate::_private::allow_null;
use crate::{json_schema, JsonSchema, Schema};
use alloc::borrow::Cow;
use core::ops::{Bound, Range, RangeInclusive};

impl<T: JsonSchema> JsonSchema for Option<T> {
    inline_schema!();

    fn schema_name() -> Cow<'static, str> {
        format!("Nullable_{}", T::schema_name()).into()
    }

    fn schema_id() -> Cow<'static, str> {
        format!("Option<{}>", T::schema_id()).into()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        let mut schema = generator.subschema_for::<T>();

        allow_null(generator, &mut schema);

        schema
    }

    fn _schemars_private_non_optional_json_schema(generator: &mut SchemaGenerator) -> Schema {
        T::_schemars_private_non_optional_json_schema(generator)
    }

    fn _schemars_private_is_option() -> bool {
        true
    }
}

impl<T: JsonSchema, E: JsonSchema> JsonSchema for Result<T, E> {
    fn schema_name() -> Cow<'static, str> {
        format!("Result_of_{}_or_{}", T::schema_name(), E::schema_name()).into()
    }

    fn schema_id() -> Cow<'static, str> {
        format!("Result<{}, {}>", T::schema_id(), E::schema_id()).into()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "oneOf": [
                {
                    "type": "object",
                    "properties": {
                        "Ok": generator.subschema_for::<T>()
                    },
                    "required": ["Ok"]
                },
                {
                    "type": "object",
                    "properties": {
                        "Err": generator.subschema_for::<E>()
                    },
                    "required": ["Err"]
                }
            ]
        })
    }
}

impl<T: JsonSchema> JsonSchema for Bound<T> {
    fn schema_name() -> Cow<'static, str> {
        format!("Bound_of_{}", T::schema_name()).into()
    }

    fn schema_id() -> Cow<'static, str> {
        format!("Bound<{}>", T::schema_id()).into()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "oneOf": [
                {
                    "type": "object",
                    "properties": {
                        "Included": generator.subschema_for::<T>()
                    },
                    "required": ["Included"]
                },
                {
                    "type": "object",
                    "properties": {
                        "Excluded": generator.subschema_for::<T>()
                    },
                    "required": ["Excluded"]
                },
                {
                    "type": "string",
                    "const": "Unbounded"
                }
            ]
        })
    }
}

impl<T: JsonSchema> JsonSchema for Range<T> {
    fn schema_name() -> Cow<'static, str> {
        format!("Range_of_{}", T::schema_name()).into()
    }

    fn schema_id() -> Cow<'static, str> {
        format!("Range<{}>", T::schema_id()).into()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        let subschema = generator.subschema_for::<T>();
        json_schema!({
            "type": "object",
            "properties": {
                "start": subschema,
                "end": subschema
            },
            "required": ["start", "end"]
        })
    }
}

forward_impl!((<T: JsonSchema> JsonSchema for RangeInclusive<T>) => Range<T>);

forward_impl!((<T: ?Sized> JsonSchema for core::marker::PhantomData<T>) => ());

forward_impl!((<'a> JsonSchema for core::fmt::Arguments<'a>) => String);
