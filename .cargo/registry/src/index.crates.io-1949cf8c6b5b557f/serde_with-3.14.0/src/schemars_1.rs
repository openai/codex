//! Integration with [schemars v1](schemars_1).
//!
//! This module is only available if using the `schemars_1` feature of the crate.
//!
//! If you would like to add support for schemars to your own `serde_with` helpers
//! see [`JsonSchemaAs`].

use crate::{
    formats::{Flexible, Format, PreferMany, PreferOne, Separator, Strict},
    prelude::{Schema as WrapSchema, *},
    utils::NumberExt as _,
};
use ::schemars_1::{json_schema, JsonSchema, Schema, SchemaGenerator};
use alloc::{
    borrow::Cow,
    collections::{BTreeMap, BTreeSet},
    format,
    rc::Rc,
    vec::Vec,
};
use serde_json::Value;

//===================================================================
// Trait Definition

/// A type which can be described as a JSON schema document.
///
/// This trait is as [`SerializeAs`] is to [`Serialize`] but for [`JsonSchema`].
/// You can use it to make your custom [`SerializeAs`] and [`DeserializeAs`]
/// types also support being described via JSON schemas.
///
/// It is used by the [`Schema`][1] type in order to implement [`JsonSchema`]
/// for the relevant types. [`Schema`][1] is used implicitly by the [`serde_as`]
/// macro to instruct `schemars` on how to generate JSON schemas for fields
/// annotated with `#[serde_as(as = "...")]` attributes.
///
/// # Examples
/// Suppose we have our very own `PositiveInt` type. Then we could add support
/// for generating a schema from it like this
///
/// ```
/// # extern crate schemars_1 as schemars;
/// # use serde::{Serialize, Serializer, Deserialize, Deserializer};
/// # use serde_with::{SerializeAs, DeserializeAs};
/// use serde_with::schemars_1::JsonSchemaAs;
/// use schemars::{json_schema, SchemaGenerator, Schema};
/// use std::borrow::Cow;
///
/// # #[allow(dead_code)]
/// struct PositiveInt;
///
/// impl SerializeAs<i32> for PositiveInt {
///     // ...
///     # fn serialize_as<S>(&value: &i32, ser: S) -> Result<S::Ok, S::Error>
///     # where
///     #    S: Serializer
///     # {
///     #    if value < 0 {
///     #        return Err(serde::ser::Error::custom(
///     #            "expected a positive integer value, got a negative one"
///     #        ));
///     #    }
///     #
///     #    value.serialize(ser)
///     # }
/// }
///
/// impl<'de> DeserializeAs<'de, i32> for PositiveInt {
///     // ...
///     # fn deserialize_as<D>(de: D) -> Result<i32, D::Error>
///     # where
///     #     D: Deserializer<'de>,
///     # {
///     #     match i32::deserialize(de) {
///     #         Ok(value) if value < 0 => Err(serde::de::Error::custom(
///     #             "expected a positive integer value, got a negative one"
///     #         )),
///     #         value => value
///     #     }
///     # }
/// }
///
/// impl JsonSchemaAs<i32> for PositiveInt {
///     fn schema_name() -> Cow<'static, str> {
///         "PositiveInt".into()
///     }
///
///     fn json_schema(_: &mut SchemaGenerator) -> Schema {
///         json_schema!({
///             "type": "integer",
///             "minimum": 0
///         })
///     }
/// }
/// ```
///
/// [0]: crate::serde_as
/// [1]: crate::Schema
pub trait JsonSchemaAs<T: ?Sized> {
    /// Whether JSON schemas generated for this type should be included directly
    /// in arent schemas, rather than being re-used where possible using the `$ref`
    /// keyword.
    ///
    /// For trivial types (such as primitives), this should return `true`. For
    /// more complex types, it should return `false`. For recursive types, this
    /// **must** return `false` to prevent infinite cycles when generating schemas.
    ///
    /// By default, this returns `false`.
    fn inline_schema() -> bool {
        false
    }

    /// The name of the generated JSON Schema.
    ///
    /// This is used as the title for root schemas, and the key within the root's `definitions` property for sub-schemas.
    ///
    /// As the schema name is used as as part of `$ref` it has to be a valid URI path segment according to
    /// [RFC 3986 Section-3](https://datatracker.ietf.org/doc/html/rfc3986#section-3).
    fn schema_name() -> Cow<'static, str>;

    /// Returns a string that uniquely identifies the schema produced by this type.
    ///
    /// This does not have to be a human-readable string, and the value will not itself be included in generated schemas.
    /// If two types produce different schemas, then they **must** have different `schema_id()`s,
    /// but two types that produce identical schemas should *ideally* have the same `schema_id()`.
    ///
    /// The default implementation returns the same value as `schema_name()`.
    fn schema_id() -> Cow<'static, str> {
        Self::schema_name()
    }

    /// Generates a JSON Schema for this type.
    ///
    /// If the returned schema depends on any [inlineable](JsonSchema::inline_schema) schemas, then this method will
    /// add them to the [`SchemaGenerator`]'s schema definitions.
    ///
    /// This should not return a `$ref` schema.
    fn json_schema(generator: &mut SchemaGenerator) -> Schema;
}

impl<T, TA> JsonSchema for WrapSchema<T, TA>
where
    T: ?Sized,
    TA: JsonSchemaAs<T>,
{
    fn schema_name() -> Cow<'static, str> {
        TA::schema_name()
    }

    fn schema_id() -> Cow<'static, str> {
        TA::schema_id()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        TA::json_schema(generator)
    }

    fn inline_schema() -> bool {
        TA::inline_schema()
    }
}

//===================================================================
// Macro helpers

macro_rules! forward_schema {
    ($fwd:ty) => {
        fn schema_name() -> Cow<'static, str> {
            <$fwd as JsonSchema>::schema_name()
        }

        fn schema_id() -> Cow<'static, str> {
            <$fwd as JsonSchema>::schema_id()
        }

        fn json_schema(gen: &mut SchemaGenerator) -> Schema {
            <$fwd as JsonSchema>::json_schema(gen)
        }

        fn inline_schema() -> bool {
            <$fwd as JsonSchema>::inline_schema()
        }
    };
}

//===================================================================
// Common definitions for various std types

impl<'a, T: 'a, TA: 'a> JsonSchemaAs<&'a T> for &'a TA
where
    T: ?Sized,
    TA: JsonSchemaAs<T>,
{
    forward_schema!(&'a WrapSchema<T, TA>);
}

impl<'a, T: 'a, TA: 'a> JsonSchemaAs<&'a mut T> for &'a mut TA
where
    T: ?Sized,
    TA: JsonSchemaAs<T>,
{
    forward_schema!(&'a mut WrapSchema<T, TA>);
}

impl<T, TA> JsonSchemaAs<Option<T>> for Option<TA>
where
    TA: JsonSchemaAs<T>,
{
    forward_schema!(Option<WrapSchema<T, TA>>);
}

impl<T, TA> JsonSchemaAs<Box<T>> for Box<TA>
where
    T: ?Sized,
    TA: JsonSchemaAs<T>,
{
    forward_schema!(Box<WrapSchema<T, TA>>);
}

impl<T, TA> JsonSchemaAs<Rc<T>> for Rc<TA>
where
    T: ?Sized,
    TA: JsonSchemaAs<T>,
{
    forward_schema!(Rc<WrapSchema<T, TA>>);
}

impl<T, TA> JsonSchemaAs<Arc<T>> for Arc<TA>
where
    T: ?Sized,
    TA: JsonSchemaAs<T>,
{
    forward_schema!(Arc<WrapSchema<T, TA>>);
}

impl<T, TA> JsonSchemaAs<Vec<T>> for Vec<TA>
where
    TA: JsonSchemaAs<T>,
{
    forward_schema!(Vec<WrapSchema<T, TA>>);
}

impl<T, TA> JsonSchemaAs<VecDeque<T>> for VecDeque<TA>
where
    TA: JsonSchemaAs<T>,
{
    forward_schema!(VecDeque<WrapSchema<T, TA>>);
}

// schemars only requires that V implement JsonSchema for BTreeMap<K, V>
impl<K, V, KA, VA> JsonSchemaAs<BTreeMap<K, V>> for BTreeMap<KA, VA>
where
    KA: JsonSchemaAs<K>,
    VA: JsonSchemaAs<V>,
{
    forward_schema!(BTreeMap<WrapSchema<K, KA>, WrapSchema<V, VA>>);
}

// schemars only requires that V implement JsonSchema for HashMap<K, V>
#[cfg(feature = "std")]
impl<K, V, S, KA, VA> JsonSchemaAs<HashMap<K, V, S>> for HashMap<KA, VA, S>
where
    KA: JsonSchemaAs<K>,
    VA: JsonSchemaAs<V>,
{
    forward_schema!(HashMap<WrapSchema<K, KA>, WrapSchema<V, VA>, S>);
}

impl<T, TA> JsonSchemaAs<BTreeSet<T>> for BTreeSet<TA>
where
    TA: JsonSchemaAs<T>,
{
    forward_schema!(BTreeSet<WrapSchema<T, TA>>);
}

#[cfg(feature = "std")]
impl<T, TA, S> JsonSchemaAs<T> for HashSet<TA, S>
where
    TA: JsonSchemaAs<T>,
{
    forward_schema!(HashSet<WrapSchema<T, TA>, S>);
}

impl<T, TA> JsonSchemaAs<Bound<T>> for Bound<TA>
where
    TA: JsonSchemaAs<T>,
{
    forward_schema!(Bound<WrapSchema<T, TA>>);
}

impl<T, TA> JsonSchemaAs<Range<T>> for Range<TA>
where
    TA: JsonSchemaAs<T>,
{
    forward_schema!(Range<WrapSchema<T, TA>>);
}

// Note: Not included in `schemars`
// impl<T, TA> JsonSchemaAs<RangeFrom<T>> for RangeFrom<TA>
// where
//     TA: JsonSchemaAs<T>,
// {
//     forward_schema!(RangeFrom<WrapSchema<T, TA>>);
// }

// impl<T, TA> JsonSchemaAs<RangeTo<T>> for RangeTo<TA>
// where
//     TA: JsonSchemaAs<T>,
// {
//     forward_schema!(RangeTo<WrapSchema<T, TA>>);
// }

impl<T, TA> JsonSchemaAs<RangeInclusive<T>> for RangeInclusive<TA>
where
    TA: JsonSchemaAs<T>,
{
    forward_schema!(RangeInclusive<WrapSchema<T, TA>>);
}

impl<T, TA, const N: usize> JsonSchemaAs<[T; N]> for [TA; N]
where
    TA: JsonSchemaAs<T>,
{
    fn schema_name() -> Cow<'static, str> {
        format!("[{}; {}]", <WrapSchema<T, TA>>::schema_name(), N).into()
    }

    fn schema_id() -> Cow<'static, str> {
        format!("[{}; {}]", <WrapSchema<T, TA>>::schema_id(), N).into()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        let (max, min) = match N.try_into() {
            Ok(len) => (Some(len), Some(len)),
            Err(_) => (None, Some(u32::MAX)),
        };

        json_schema!({
            "type": "array",
            "items": generator.subschema_for::<WrapSchema<T, TA>>(),
            "maxItems": max,
            "minItems": min
        })
    }

    fn inline_schema() -> bool {
        true
    }
}

macro_rules! schema_for_tuple {
    (
        ( $( $ts:ident )+ )
        ( $( $as:ident )+ )
    ) => {
        impl<$($ts,)+ $($as,)+> JsonSchemaAs<($($ts,)+)> for ($($as,)+)
        where
            $( $as: JsonSchemaAs<$ts>, )+
        {
            forward_schema!(( $( WrapSchema<$ts, $as>, )+ ));
        }
    }
}

impl JsonSchemaAs<()> for () {
    forward_schema!(());
}

// schemars only implements JsonSchema for tuples up to 15 elements so we do
// the same here.
schema_for_tuple!((T0)(A0));
schema_for_tuple!((T0 T1) (A0 A1));
schema_for_tuple!((T0 T1 T2) (A0 A1 A2));
schema_for_tuple!((T0 T1 T2 T3) (A0 A1 A2 A3));
schema_for_tuple!((T0 T1 T2 T3 T4) (A0 A1 A2 A3 A4));
schema_for_tuple!((T0 T1 T2 T3 T4 T5) (A0 A1 A2 A3 A4 A5));
schema_for_tuple!((T0 T1 T2 T3 T4 T5 T6) (A0 A1 A2 A3 A4 A5 A6));
schema_for_tuple!((T0 T1 T2 T3 T4 T5 T6 T7) (A0 A1 A2 A3 A4 A5 A6 A7));
schema_for_tuple!((T0 T1 T2 T3 T4 T5 T6 T7 T8) (A0 A1 A2 A3 A4 A5 A6 A7 A8));
schema_for_tuple!((T0 T1 T2 T3 T4 T5 T6 T7 T8 T9) (A0 A1 A2 A3 A4 A5 A6 A7 A8 A9));
schema_for_tuple!((T0 T1 T2 T3 T4 T5 T6 T7 T8 T9 T10) (A0 A1 A2 A3 A4 A5 A6 A7 A8 A9 A10));
schema_for_tuple!((T0 T1 T2 T3 T4 T5 T6 T7 T8 T9 T10 T11) (A0 A1 A2 A3 A4 A5 A6 A7 A8 A9 A10 A11));
schema_for_tuple!(
    (T0 T1 T2 T3 T4 T5 T6 T7 T8 T9 T10 T11 T12)
    (A0 A1 A2 A3 A4 A5 A6 A7 A8 A9 A10 A11 A12)
);
schema_for_tuple!(
    (T0 T1 T2 T3 T4 T5 T6 T7 T8 T9 T10 T11 T12 T13)
    (A0 A1 A2 A3 A4 A5 A6 A7 A8 A9 A10 A11 A12 A13)
);
schema_for_tuple!(
    (T0 T1 T2 T3 T4 T5 T6 T7 T8 T9 T10 T11 T12 T13 T14)
    (A0 A1 A2 A3 A4 A5 A6 A7 A8 A9 A10 A11 A12 A13 A14)
);
schema_for_tuple!(
    (T0 T1 T2 T3 T4 T5 T6 T7 T8 T9 T10 T11 T12 T13 T14 T15)
    (A0 A1 A2 A3 A4 A5 A6 A7 A8 A9 A10 A11 A12 A13 A14 A15)
);

//===================================================================
// Impls for serde_with types.

impl<T: JsonSchema> JsonSchemaAs<T> for Same {
    forward_schema!(T);
}

impl<T> JsonSchemaAs<T> for DisplayFromStr {
    forward_schema!(String);
}

impl JsonSchemaAs<bool> for BoolFromInt<Strict> {
    fn schema_name() -> Cow<'static, str> {
        "BoolFromInt<Strict>".into()
    }

    fn schema_id() -> Cow<'static, str> {
        "serde_with::BoolFromInt<Strict>".into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "type": "integer",
            "minimum": 0.0,
            "maximum": 1.0
        })
    }

    fn inline_schema() -> bool {
        true
    }
}

impl JsonSchemaAs<bool> for BoolFromInt<Flexible> {
    fn schema_name() -> Cow<'static, str> {
        "BoolFromInt<Flexible>".into()
    }

    fn schema_id() -> Cow<'static, str> {
        "serde_with::BoolFromInt<Flexible>".into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "type": "integer",
        })
    }

    fn inline_schema() -> bool {
        true
    }
}

impl<'a, T: 'a> JsonSchemaAs<Cow<'a, T>> for BorrowCow
where
    T: ?Sized + ToOwned,
    Cow<'a, T>: JsonSchema,
{
    forward_schema!(Cow<'a, T>);
}

impl<T> JsonSchemaAs<T> for Bytes {
    forward_schema!(Vec<u8>);
}

impl JsonSchemaAs<Vec<u8>> for BytesOrString {
    fn schema_name() -> Cow<'static, str> {
        "BytesOrString".into()
    }

    fn schema_id() -> Cow<'static, str> {
        "serde_with::BytesOrString".into()
    }

    fn json_schema(g: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "anyOf": [
                g.subschema_for::<Vec<u8>>(),
                {
                    "type": "string",
                    "writeOnly": true
                }
            ]
        })
    }

    fn inline_schema() -> bool {
        true
    }
}

impl<T, TA> JsonSchemaAs<T> for DefaultOnError<TA>
where
    TA: JsonSchemaAs<T>,
{
    forward_schema!(WrapSchema<T, TA>);
}

impl<T, TA> JsonSchemaAs<T> for DefaultOnNull<TA>
where
    TA: JsonSchemaAs<T>,
{
    forward_schema!(Option<WrapSchema<T, TA>>);
}

impl<O, T: JsonSchema> JsonSchemaAs<O> for FromInto<T> {
    forward_schema!(T);
}

impl<O, T: JsonSchema> JsonSchemaAs<O> for FromIntoRef<T> {
    forward_schema!(T);
}

impl<T, U: JsonSchema> JsonSchemaAs<T> for TryFromInto<U> {
    forward_schema!(U);
}

impl<T, U: JsonSchema> JsonSchemaAs<T> for TryFromIntoRef<U> {
    forward_schema!(U);
}

impl<T, TA, FA> JsonSchemaAs<T> for IfIsHumanReadable<TA, FA>
where
    TA: JsonSchemaAs<T>,
{
    // serde_json always has `is_human_readable` set to true so we just use the
    // schema for the human readable variant.
    forward_schema!(WrapSchema<T, TA>);
}

macro_rules! schema_for_map {
    ($type:ty) => {
        impl<K, V, KA, VA> JsonSchemaAs<$type> for Map<KA, VA>
        where
            KA: JsonSchemaAs<K>,
            VA: JsonSchemaAs<V>,
        {
            forward_schema!(WrapSchema<BTreeMap<K, V>, BTreeMap<KA, VA>>);
        }
    };
}

schema_for_map!([(K, V)]);
schema_for_map!(BTreeSet<(K, V)>);
schema_for_map!(BinaryHeap<(K, V)>);
schema_for_map!(Box<[(K, V)]>);
schema_for_map!(LinkedList<(K, V)>);
schema_for_map!(Vec<(K, V)>);
schema_for_map!(VecDeque<(K, V)>);

#[cfg(feature = "std")]
impl<K, V, S, KA, VA> JsonSchemaAs<HashSet<(K, V), S>> for Map<KA, VA>
where
    KA: JsonSchemaAs<K>,
    VA: JsonSchemaAs<V>,
{
    forward_schema!(WrapSchema<BTreeMap<K, V>, BTreeMap<KA, VA>>);
}

impl<T> JsonSchemaAs<Vec<T>> for EnumMap
where
    T: JsonSchema,
{
    fn schema_name() -> Cow<'static, str> {
        format!("EnumMap({})", T::schema_name()).into()
    }

    fn schema_id() -> Cow<'static, str> {
        format!("serde_with::EnumMap({})", T::schema_id()).into()
    }

    // We generate the schema here by going through all the variants of the
    // enum (the oneOf property) and sticking all their properties onto an
    // object.
    //
    // This will be wrong if the object is not an externally tagged enum but in
    // that case serialization and deserialization will fail so it is probably
    // OK.
    fn json_schema(g: &mut SchemaGenerator) -> Schema {
        let mut inner_schema = T::json_schema(g);
        let inner = inner_schema.ensure_object();

        let one_of = match inner.get_mut("oneOf") {
            Some(Value::Array(one_of)) => one_of,
            _ => return inner_schema,
        };

        let mut properties = serde_json::Map::new();
        for schema in one_of {
            let schema = match schema {
                Value::Object(schema) => schema,
                _ => continue,
            };

            if let Some(Value::Object(props)) = schema.get_mut("properties") {
                properties.extend(core::mem::take(props));
            }
        }

        json_schema!({
            "type": "object",
            "properties": properties,
            "additionalProperties": false
        })
    }

    fn inline_schema() -> bool {
        false
    }
}

impl<T, TA> WrapSchema<Vec<T>, KeyValueMap<TA>>
where
    TA: JsonSchemaAs<T>,
{
    /// Transform a schema from the entry type of a `KeyValueMap<T>` to the
    /// resulting field type.
    ///
    /// This usually means doing one of two things:
    /// 1. removing the `$key$` property from an object, or,
    /// 2. removing the first item from an array.
    ///
    /// We also need to adjust any fields that control the number of items or
    /// properties allowed such as `(max|min)_properties` or `(max|min)_items`.
    ///
    /// This is mostly straightforward. Where things get hairy is when dealing
    /// with subschemas. JSON schemas allow you to build the schema for an
    /// object by combining multiple subschemas:
    /// - You can match exactly one of a set of subschemas (`one_of`).
    /// - You can match any of a set of subschemas (`any_of`).
    /// - You can match all of a set of subschemas (`all_of`).
    ///
    /// Unfortunately for us, we need to handle all of these options by recursing
    /// into the subschemas and applying the same transformations as above.
    fn kvmap_transform_schema_1(g: &mut SchemaGenerator, schema: &mut Schema) {
        let mut parents = Vec::new();

        let mut value = if let Some(object) = schema.as_object_mut() {
            Value::Object(core::mem::take(object))
        } else if let Some(value) = schema.as_bool() {
            Value::Bool(value)
        } else {
            unreachable!()
        };

        Self::kvmap_transform_schema_impl_1(g, &mut value, &mut parents, 0);
        *schema = Schema::try_from(value).expect("modified value was not an object or boolean");
    }

    fn kvmap_transform_schema_impl_1(
        g: &mut SchemaGenerator,
        schema: &mut Value,
        parents: &mut Vec<String>,
        depth: u32,
    ) {
        if depth > 8 {
            return;
        }

        let mut done = false;
        let schema = match schema.as_object_mut() {
            Some(schema) => schema,
            _ => return,
        };

        // The schema is a reference to a schema defined elsewhere.
        //
        // If possible we replace it with its definition but if that is not
        // available then we give up and leave it as-is.
        let mut parents = if let Some(reference) = &schema.get("$ref") {
            let reference = match reference {
                Value::String(reference) => &**reference,
                // $ref is invalid, skip
                _ => return,
            };

            let name = match Self::resolve_reference_1(g, reference) {
                Some(name) => name,
                // Reference is defined elsewhere, nothing we can do.
                None => return,
            };

            // We are in a recursive reference loop. No point in continuing.
            if parents.iter().any(|parent| parent == name) {
                return;
            }

            let name = name.to_owned();
            *schema = match g.definitions().get(&name) {
                Some(Value::Object(schema)) => schema.clone(),
                _ => return,
            };

            parents.push(name);
            utils::DropGuard::new(parents, |parents| drop(parents.pop()))
        } else {
            utils::DropGuard::unguarded(parents)
        };

        // We do comparisons here to avoid lifetime conflicts below
        let ty = match schema.get("type") {
            Some(Value::String(ty)) if ty == "object" => Some("object"),
            Some(Value::String(ty)) if ty == "array" => Some("array"),
            _ => None,
        };

        if ty == Some("object") {
            // For objects KeyValueMap uses the $key$ property so we need to remove it from
            // the inner schema.

            if let Some(Value::Object(properties)) = schema.get_mut("properties") {
                done |= properties.remove("$key$").is_some();
            }

            if let Some(Value::Array(required)) = schema.get_mut("required") {
                required.retain(|req| match req {
                    Value::String(key) if key == "$key$" => {
                        done = true;
                        false
                    }
                    _ => true,
                });
            }

            if let Some(Value::Number(max)) = schema.get_mut("maxProperties") {
                *max = max.saturating_sub(1);
            }

            if let Some(Value::Number(min)) = schema.get_mut("minProperties") {
                *min = min.saturating_sub(1);
            }
        }

        if ty == Some("array") {
            // For arrays KeyValueMap uses the first array element so we need to remove it
            // from the inner schema.

            if let Some(Value::Array(items)) = schema.get_mut("prefixItems") {
                // If the array is empty then the leading element may be following the
                // additionalItem schema. In that case we do nothing.
                if !items.is_empty() {
                    items.remove(0);
                    done = true;
                }
            }

            if let Some(Value::Array(items)) = schema.get_mut("items") {
                // If the array is empty then the leading element may be following the
                // additionalItem schema. In that case we do nothing.
                if !items.is_empty() {
                    items.remove(0);
                    done = true;
                }
            }

            if let Some(Value::Number(max)) = schema.get_mut("maxItems") {
                *max = max.saturating_sub(1);
            }

            if let Some(Value::Number(min)) = schema.get_mut("minItems") {
                *min = min.saturating_sub(1);
            }
        }

        // We've already modified the schema so there's no need to do more work.
        if done {
            return;
        }

        if let Some(Value::Array(one_of)) = schema.get_mut("oneOf") {
            for subschema in one_of {
                Self::kvmap_transform_schema_impl_1(g, subschema, &mut parents, depth + 1);
            }
        }

        if let Some(Value::Array(any_of)) = schema.get_mut("anyOf") {
            for subschema in any_of {
                Self::kvmap_transform_schema_impl_1(g, subschema, &mut parents, depth + 1);
            }
        }

        if let Some(Value::Array(all_of)) = schema.get_mut("allOf") {
            for subschema in all_of {
                Self::kvmap_transform_schema_impl_1(g, subschema, &mut parents, depth + 1);
            }
        }
    }

    fn resolve_reference_1<'a>(g: &mut SchemaGenerator, reference: &'a str) -> Option<&'a str> {
        // We can only resolve references that are contained within the current
        // schema.
        let reference = reference.strip_prefix('#')?;

        let defpath: &str = &g.settings().definitions_path;
        let defpath = defpath.strip_prefix("#").unwrap_or(defpath);

        let mut reference = reference.strip_prefix(defpath)?;
        if !defpath.ends_with('/') {
            reference = reference.strip_prefix('/').unwrap_or(reference);
        }

        Some(reference)
    }
}

impl<T, TA> JsonSchemaAs<Vec<T>> for KeyValueMap<TA>
where
    TA: JsonSchemaAs<T>,
{
    fn schema_name() -> Cow<'static, str> {
        format!("KeyValueMap({})", <WrapSchema<T, TA>>::schema_name()).into()
    }

    fn schema_id() -> Cow<'static, str> {
        format!(
            "serde_with::KeyValueMap({})",
            <WrapSchema<T, TA>>::schema_id()
        )
        .into()
    }

    fn json_schema(g: &mut SchemaGenerator) -> Schema {
        let mut value = <WrapSchema<T, TA>>::json_schema(g);
        <WrapSchema<Vec<T>, KeyValueMap<TA>>>::kvmap_transform_schema_1(g, &mut value);

        json_schema!({
            "type": "object",
            "additionalProperties": value
        })
    }
}

impl<K, V, KA, VA, const N: usize> JsonSchemaAs<[(K, V); N]> for Map<KA, VA>
where
    KA: JsonSchemaAs<K>,
    VA: JsonSchemaAs<V>,
{
    forward_schema!(WrapSchema<BTreeMap<K, V>, BTreeMap<KA, VA>>);
}

macro_rules! map_first_last_wins_schema {
    ($(=> $extra:ident)? $type:ty) => {
        impl<K, V, $($extra,)? KA, VA> JsonSchemaAs<$type> for MapFirstKeyWins<KA, VA>
        where
            KA: JsonSchemaAs<K>,
            VA: JsonSchemaAs<V>,
        {
            forward_schema!(BTreeMap<WrapSchema<K, KA>, WrapSchema<V, VA>>);
        }

        impl<K, V, $($extra,)? KA, VA> JsonSchemaAs<$type> for MapPreventDuplicates<KA, VA>
        where
            KA: JsonSchemaAs<K>,
            VA: JsonSchemaAs<V>,
        {
            forward_schema!(BTreeMap<WrapSchema<K, KA>, WrapSchema<V, VA>>);
        }
    }
}

map_first_last_wins_schema!(BTreeMap<K, V>);
#[cfg(feature = "std")]
map_first_last_wins_schema!(=> S HashMap<K, V, S>);
#[cfg(feature = "hashbrown_0_14")]
map_first_last_wins_schema!(=> S hashbrown_0_14::HashMap<K, V, S>);
#[cfg(feature = "hashbrown_0_15")]
map_first_last_wins_schema!(=> S hashbrown_0_15::HashMap<K, V, S>);
#[cfg(feature = "indexmap_1")]
map_first_last_wins_schema!(=> S indexmap_1::IndexMap<K, V, S>);
#[cfg(feature = "indexmap_2")]
map_first_last_wins_schema!(=> S indexmap_2::IndexMap<K, V, S>);

impl<T, TA> JsonSchemaAs<Vec<T>> for OneOrMany<TA, PreferOne>
where
    TA: JsonSchemaAs<T>,
{
    fn schema_name() -> Cow<'static, str> {
        format!(
            "OneOrMany({},PreferOne)",
            <WrapSchema<T, TA>>::schema_name()
        )
        .into()
    }

    fn schema_id() -> Cow<'static, str> {
        format!(
            "serde_with::OneOrMany({},PreferOne)",
            <WrapSchema<T, TA>>::schema_id()
        )
        .into()
    }

    fn json_schema(g: &mut SchemaGenerator) -> Schema {
        let single = g.subschema_for::<WrapSchema<T, TA>>();

        json_schema!({
            "anyOf": [
                single,
                {
                    "type": "array",
                    "items": single
                }
            ]
        })
    }
}

impl<T, TA> JsonSchemaAs<Vec<T>> for OneOrMany<TA, PreferMany>
where
    TA: JsonSchemaAs<T>,
{
    fn schema_name() -> Cow<'static, str> {
        format!(
            "OneOrMany<{}, PreferMany>",
            <WrapSchema<T, TA>>::schema_name()
        )
        .into()
    }

    fn schema_id() -> Cow<'static, str> {
        format!(
            "serde_with::OneOrMany<{}, PreferMany>",
            <WrapSchema<T, TA>>::schema_id()
        )
        .into()
    }

    fn json_schema(g: &mut SchemaGenerator) -> Schema {
        let inner = g.subschema_for::<WrapSchema<T, TA>>();

        json_schema!({
            "anyOf": [
                {
                    "writeOnly": true,
                    "allOf": [
                        inner
                    ],
                },
                {
                    "type": "array",
                    "items": inner
                }
            ]
        })
    }
}

macro_rules! schema_for_pickfirst {
    ($( $param:ident )+) => {
        impl<T, $($param,)+> JsonSchemaAs<T> for PickFirst<($( $param, )+)>
        where
            $( $param: JsonSchemaAs<T>, )+
        {
            fn schema_name() -> Cow<'static, str> {
                format!(
                    concat!(
                        "PickFirst(",
                        $( "{", stringify!($param), "}", )+
                        ")"
                    ),
                    $( $param = <WrapSchema<T, $param>>::schema_name(), )+
                )
                .into()
            }

            fn schema_id() -> Cow<'static, str> {
                format!(
                    concat!(
                        "serde_with::PickFirst(",
                        $( "{", stringify!($param), "}", )+
                        ")"
                    ),
                    $( $param = <WrapSchema<T, $param>>::schema_id(), )+
                )
                .into()
            }

            fn json_schema(g: &mut SchemaGenerator) -> Schema {
                let mut first = true;
                let subschemas = alloc::vec![$(
                    {
                        let is_first = core::mem::replace(&mut first, false);
                        let schema = g.subschema_for::<WrapSchema<T, $param>>();

                        if !is_first {
                            json_schema!({
                                "writeOnly": true,
                                "allOf": [schema]
                            })
                        } else {
                            schema
                        }
                    }
                ),+];

                json_schema!({
                    "anyOf": subschemas
                })
            }
        }
    }
}

schema_for_pickfirst!(A);
schema_for_pickfirst!(A B);
schema_for_pickfirst!(A B C);
schema_for_pickfirst!(A B C D);

impl<T, TA> JsonSchemaAs<T> for SetLastValueWins<TA>
where
    TA: JsonSchemaAs<T>,
{
    fn schema_id() -> Cow<'static, str> {
        format!(
            "serde_with::SetLastValueWins<{}>",
            <WrapSchema<T, TA> as JsonSchema>::schema_id()
        )
        .into()
    }

    fn schema_name() -> Cow<'static, str> {
        format!(
            "SetLastValueWins<{}>",
            <WrapSchema<T, TA> as JsonSchema>::schema_name()
        )
        .into()
    }

    fn json_schema(g: &mut SchemaGenerator) -> Schema {
        let mut schema = <WrapSchema<T, TA> as JsonSchema>::json_schema(g);
        let object = schema.ensure_object();

        // We explicitly allow duplicate items since the whole point of
        // SetLastValueWins is to take the duplicate value.
        object.remove("uniqueItems");

        schema
    }

    fn inline_schema() -> bool {
        <WrapSchema<T, TA> as JsonSchema>::inline_schema()
    }
}

impl<T, TA> JsonSchemaAs<T> for SetPreventDuplicates<TA>
where
    TA: JsonSchemaAs<T>,
{
    forward_schema!(WrapSchema<T, TA>);
}

impl<SEP, T, TA> JsonSchemaAs<T> for StringWithSeparator<SEP, TA>
where
    SEP: Separator,
{
    forward_schema!(String);
}

impl<T, TA> JsonSchemaAs<Vec<T>> for VecSkipError<TA>
where
    TA: JsonSchemaAs<T>,
{
    forward_schema!(Vec<WrapSchema<T, TA>>);
}

mod timespan {
    use super::*;

    // #[non_exhaustive] is not actually necessary here but it should
    // help avoid warnings about semver breakage if this ever changes.
    #[non_exhaustive]
    pub enum TimespanTargetType {
        String,
        F64,
        U64,
        I64,
    }

    /// Internal helper trait used to constrain which types we implement
    /// `JsonSchemaAs<T>` for.
    pub trait TimespanSchemaTarget<F> {
        /// The underlying type.
        ///
        /// This is mainly used to decide which variant of the resulting schema
        /// should be marked as `write_only: true`.
        const TYPE: TimespanTargetType;

        /// Whether the target type is signed.
        ///
        /// This is only true for `std::time::Duration`.
        const SIGNED: bool = true;
    }

    #[cfg_attr(not(feature = "std"), allow(unused_macros))]
    macro_rules! timespan_type_of {
        (String) => {
            TimespanTargetType::String
        };
        (f64) => {
            TimespanTargetType::F64
        };
        (i64) => {
            TimespanTargetType::I64
        };
        (u64) => {
            TimespanTargetType::U64
        };
    }

    #[cfg_attr(not(feature = "std"), allow(unused_macros))]
    macro_rules! declare_timespan_target {
        ( $target:ty { $($format:ident),* $(,)? } ) => {
            $(
                impl TimespanSchemaTarget<$format> for $target {
                    const TYPE: TimespanTargetType = timespan_type_of!($format);
                }
            )*
        }
    }

    impl TimespanSchemaTarget<u64> for Duration {
        const TYPE: TimespanTargetType = TimespanTargetType::U64;
        const SIGNED: bool = false;
    }

    impl TimespanSchemaTarget<f64> for Duration {
        const TYPE: TimespanTargetType = TimespanTargetType::F64;
        const SIGNED: bool = false;
    }

    impl TimespanSchemaTarget<String> for Duration {
        const TYPE: TimespanTargetType = TimespanTargetType::String;
        const SIGNED: bool = false;
    }

    #[cfg(feature = "std")]
    declare_timespan_target!(SystemTime { i64, f64, String });

    #[cfg(feature = "chrono_0_4")]
    declare_timespan_target!(::chrono_0_4::Duration { i64, f64, String });
    #[cfg(feature = "chrono_0_4")]
    declare_timespan_target!(::chrono_0_4::DateTime<::chrono_0_4::Utc> { i64, f64, String });
    #[cfg(all(feature = "chrono_0_4", feature = "std"))]
    declare_timespan_target!(::chrono_0_4::DateTime<::chrono_0_4::Local> { i64, f64, String });
    #[cfg(feature = "chrono_0_4")]
    declare_timespan_target!(::chrono_0_4::NaiveDateTime { i64, f64, String });

    #[cfg(feature = "time_0_3")]
    declare_timespan_target!(::time_0_3::Duration { i64, f64, String });
    #[cfg(feature = "time_0_3")]
    declare_timespan_target!(::time_0_3::OffsetDateTime { i64, f64, String });
    #[cfg(feature = "time_0_3")]
    declare_timespan_target!(::time_0_3::PrimitiveDateTime { i64, f64, String });
}

use self::timespan::{TimespanSchemaTarget, TimespanTargetType};

/// Internal type used for the base impls on `DurationXXX` and `TimestampYYY` types.
///
/// This allows the `JsonSchema` impls that are Strict to be generic without
/// committing to it as part of the public API.
struct Timespan<Format, Strictness>(PhantomData<(Format, Strictness)>);

impl<T, F> JsonSchemaAs<T> for Timespan<F, Strict>
where
    T: TimespanSchemaTarget<F>,
    F: Format + JsonSchema,
{
    forward_schema!(F);
}

impl TimespanTargetType {
    pub(crate) fn into_flexible_schema(self, signed: bool) -> Schema {
        let mut number = json_schema!({
            "type": "number"
        });

        if !signed {
            number
                .ensure_object()
                .insert("minimum".into(), serde_json::json!(0.0));
        }

        // This is a more lenient version of the regex used to determine
        // whether JSON numbers are valid. Specifically, it allows multiple
        // leading zeroes whereas that is illegal in JSON.
        let regex = r#"[0-9]+(\.[0-9]+)?([eE][+-]?[0-9]+)?"#;
        let mut string = json_schema!({
            "type": "string",
            "pattern": match signed {
                true => format!("^-?{regex}$"),
                false => format!("^{regex}$"),
            }
        });

        if matches!(self, Self::String) {
            number
                .ensure_object()
                .insert("writeOnly".into(), true.into());
        } else {
            string
                .ensure_object()
                .insert("writeOnly".into(), true.into());
        }

        json_schema!({
            "oneOf": [number, string]
        })
    }

    pub(crate) fn schema_id(self) -> &'static str {
        match self {
            Self::String => "serde_with::FlexibleStringTimespan",
            Self::F64 => "serde_with::FlexibleF64Timespan",
            Self::U64 => "serde_with::FlexibleU64Timespan",
            Self::I64 => "serde_with::FlexibleI64Timespan",
        }
    }
}

impl<T, F> JsonSchemaAs<T> for Timespan<F, Flexible>
where
    T: TimespanSchemaTarget<F>,
    F: Format + JsonSchema,
{
    fn schema_name() -> Cow<'static, str> {
        <T as TimespanSchemaTarget<F>>::TYPE
            .schema_id()
            .strip_prefix("serde_with::")
            .expect("schema id did not start with `serde_with::` - this is a bug")
            .into()
    }

    fn schema_id() -> Cow<'static, str> {
        <T as TimespanSchemaTarget<F>>::TYPE.schema_id().into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        <T as TimespanSchemaTarget<F>>::TYPE
            .into_flexible_schema(<T as TimespanSchemaTarget<F>>::SIGNED)
    }

    fn inline_schema() -> bool {
        true
    }
}

macro_rules! forward_duration_schema {
    ($ty:ident) => {
        impl<T, F> JsonSchemaAs<T> for $ty<F, Strict>
        where
            T: TimespanSchemaTarget<F>,
            F: Format + JsonSchema
        {
            forward_schema!(WrapSchema<T, Timespan<F, Strict>>);
        }

        impl<T, F> JsonSchemaAs<T> for $ty<F, Flexible>
        where
            T: TimespanSchemaTarget<F>,
            F: Format + JsonSchema
        {
            forward_schema!(WrapSchema<T, Timespan<F, Flexible>>);
        }
    };
}

forward_duration_schema!(DurationSeconds);
forward_duration_schema!(DurationMilliSeconds);
forward_duration_schema!(DurationMicroSeconds);
forward_duration_schema!(DurationNanoSeconds);

forward_duration_schema!(DurationSecondsWithFrac);
forward_duration_schema!(DurationMilliSecondsWithFrac);
forward_duration_schema!(DurationMicroSecondsWithFrac);
forward_duration_schema!(DurationNanoSecondsWithFrac);

forward_duration_schema!(TimestampSeconds);
forward_duration_schema!(TimestampMilliSeconds);
forward_duration_schema!(TimestampMicroSeconds);
forward_duration_schema!(TimestampNanoSeconds);

forward_duration_schema!(TimestampSecondsWithFrac);
forward_duration_schema!(TimestampMilliSecondsWithFrac);
forward_duration_schema!(TimestampMicroSecondsWithFrac);
forward_duration_schema!(TimestampNanoSecondsWithFrac);
