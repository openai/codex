//! Integration with [schemars v0.8](schemars_0_8).
//!
//! This module is only available if using the `schemars_0_8` feature of the crate.
//!
//! If you would like to add support for schemars to your own `serde_with` helpers
//! see [`JsonSchemaAs`].

use crate::{
    formats::{Flexible, Format, PreferMany, PreferOne, Separator, Strict},
    prelude::{Schema as WrapSchema, *},
};
use ::schemars_0_8::{
    gen::SchemaGenerator,
    schema::{
        ArrayValidation, InstanceType, Metadata, NumberValidation, ObjectValidation, Schema,
        SchemaObject, SingleOrVec, SubschemaValidation,
    },
    JsonSchema,
};

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
/// # extern crate schemars_0_8 as schemars;
/// # use serde::{Serialize, Serializer, Deserialize, Deserializer};
/// # use serde_with::{SerializeAs, DeserializeAs};
/// use serde_with::schemars_0_8::JsonSchemaAs;
/// use schemars::gen::SchemaGenerator;
/// use schemars::schema::Schema;
/// use schemars::JsonSchema;
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
///     fn schema_name() -> String {
///         "PositiveInt".into()
///     }
///
///     fn json_schema(gen: &mut SchemaGenerator) -> Schema {
///         let mut schema = <i32 as JsonSchema>::json_schema(gen).into_object();
///         schema.number().minimum = Some(0.0);
///         schema.into()
///     }
/// }
/// ```
///
/// [0]: crate::serde_as
/// [1]: crate::Schema
pub trait JsonSchemaAs<T: ?Sized> {
    /// Whether JSON Schemas generated for this type should be re-used where possible using the `$ref` keyword.
    ///
    /// For trivial types (such as primitives), this should return `false`. For more complex types, it should return `true`.
    /// For recursive types, this **must** return `true` to prevent infinite cycles when generating schemas.
    ///
    /// By default, this returns `true`.
    fn is_referenceable() -> bool {
        true
    }

    /// The name of the generated JSON Schema.
    ///
    /// This is used as the title for root schemas, and the key within the root's `definitions` property for sub-schemas.
    ///
    /// As the schema name is used as as part of `$ref` it has to be a valid URI path segment according to
    /// [RFC 3986 Section-3](https://datatracker.ietf.org/doc/html/rfc3986#section-3).
    fn schema_name() -> String;

    /// Returns a string that uniquely identifies the schema produced by this type.
    ///
    /// This does not have to be a human-readable string, and the value will not itself be included in generated schemas.
    /// If two types produce different schemas, then they **must** have different `schema_id()`s,
    /// but two types that produce identical schemas should *ideally* have the same `schema_id()`.
    ///
    /// The default implementation returns the same value as `schema_name()`.
    fn schema_id() -> Cow<'static, str> {
        Cow::Owned(Self::schema_name())
    }

    /// Generates a JSON Schema for this type.
    ///
    /// If the returned schema depends on any [referenceable](JsonSchema::is_referenceable) schemas, then this method will
    /// add them to the [`SchemaGenerator`]'s schema definitions.
    ///
    /// This should not return a `$ref` schema.
    fn json_schema(gen: &mut SchemaGenerator) -> Schema;
}

impl<T, TA> JsonSchema for WrapSchema<T, TA>
where
    T: ?Sized,
    TA: JsonSchemaAs<T>,
{
    fn schema_name() -> String {
        TA::schema_name()
    }

    fn schema_id() -> Cow<'static, str> {
        TA::schema_id()
    }

    fn json_schema(gen: &mut SchemaGenerator) -> Schema {
        TA::json_schema(gen)
    }

    fn is_referenceable() -> bool {
        TA::is_referenceable()
    }
}

//===================================================================
// Macro helpers

macro_rules! forward_schema {
    ($fwd:ty) => {
        fn schema_name() -> String {
            <$fwd as JsonSchema>::schema_name()
        }

        fn schema_id() -> Cow<'static, str> {
            <$fwd as JsonSchema>::schema_id()
        }

        fn json_schema(gen: &mut SchemaGenerator) -> Schema {
            <$fwd as JsonSchema>::json_schema(gen)
        }

        fn is_referenceable() -> bool {
            <$fwd as JsonSchema>::is_referenceable()
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
    VA: JsonSchemaAs<V>,
{
    forward_schema!(BTreeMap<WrapSchema<K, KA>, WrapSchema<V, VA>>);
}

// schemars only requires that V implement JsonSchema for HashMap<K, V>
impl<K, V, S, KA, VA> JsonSchemaAs<HashMap<K, V, S>> for HashMap<KA, VA, S>
where
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
    fn schema_name() -> String {
        std::format!("[{}; {}]", <WrapSchema<T, TA>>::schema_name(), N)
    }

    fn schema_id() -> Cow<'static, str> {
        std::format!("[{}; {}]", <WrapSchema<T, TA>>::schema_id(), N).into()
    }

    fn json_schema(gen: &mut SchemaGenerator) -> Schema {
        let (max, min) = match N.try_into() {
            Ok(len) => (Some(len), Some(len)),
            Err(_) => (None, Some(u32::MAX)),
        };

        SchemaObject {
            instance_type: Some(InstanceType::Array.into()),
            array: Some(Box::new(ArrayValidation {
                items: Some(gen.subschema_for::<WrapSchema<T, TA>>().into()),
                max_items: max,
                min_items: min,
                ..Default::default()
            })),
            ..Default::default()
        }
        .into()
    }

    fn is_referenceable() -> bool {
        false
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
    fn schema_name() -> String {
        "BoolFromInt<Strict>".into()
    }

    fn schema_id() -> Cow<'static, str> {
        "serde_with::BoolFromInt<Strict>".into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        SchemaObject {
            instance_type: Some(InstanceType::Integer.into()),
            number: Some(Box::new(NumberValidation {
                minimum: Some(0.0),
                maximum: Some(1.0),
                ..Default::default()
            })),
            ..Default::default()
        }
        .into()
    }

    fn is_referenceable() -> bool {
        false
    }
}

impl JsonSchemaAs<bool> for BoolFromInt<Flexible> {
    fn schema_name() -> String {
        "BoolFromInt<Flexible>".into()
    }

    fn schema_id() -> Cow<'static, str> {
        "serde_with::BoolFromInt<Flexible>".into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        SchemaObject {
            instance_type: Some(InstanceType::Integer.into()),
            ..Default::default()
        }
        .into()
    }

    fn is_referenceable() -> bool {
        false
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
    fn schema_name() -> String {
        "BytesOrString".into()
    }

    fn schema_id() -> Cow<'static, str> {
        "serde_with::BytesOrString".into()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        SchemaObject {
            subschemas: Some(Box::new(SubschemaValidation {
                any_of: Some(std::vec![
                    generator.subschema_for::<Vec<u8>>(),
                    SchemaObject {
                        instance_type: Some(InstanceType::String.into()),
                        metadata: Some(Box::new(Metadata {
                            write_only: true,
                            ..Default::default()
                        })),
                        ..Default::default()
                    }
                    .into()
                ]),
                ..Default::default()
            })),
            ..Default::default()
        }
        .into()
    }

    fn is_referenceable() -> bool {
        false
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

impl<K, V, S, KA, VA> JsonSchemaAs<HashSet<(K, V), S>> for Map<KA, VA>
where
    VA: JsonSchemaAs<V>,
{
    forward_schema!(WrapSchema<BTreeMap<K, V>, BTreeMap<KA, VA>>);
}

impl<T> JsonSchemaAs<Vec<T>> for EnumMap
where
    T: JsonSchema,
{
    fn schema_name() -> String {
        std::format!("EnumMap({})", T::schema_name())
    }

    fn schema_id() -> Cow<'static, str> {
        std::format!("serde_with::EnumMap({})", T::schema_id()).into()
    }

    // We generate the schema here by going through all the variants of the
    // enum (the oneOf property) and sticking all their properties onto an
    // object.
    //
    // This will be wrong if the object is not an externally tagged enum but in
    // that case serialization and deserialization will fail so it is probably
    // OK.
    fn json_schema(gen: &mut SchemaGenerator) -> Schema {
        let mut object = SchemaObject {
            instance_type: Some(InstanceType::Object.into()),
            ..Default::default()
        };
        let inner = T::json_schema(gen).into_object();

        let one_of = match inner.subschemas {
            Some(subschemas) => match subschemas.one_of {
                Some(one_of) => one_of,
                None => return object.into(),
            },
            None => return object.into(),
        };

        let properties = &mut object.object().properties;
        for schema in one_of {
            if let Some(object) = schema.into_object().object {
                properties.extend(object.properties.into_iter());
            }
        }

        object.object().additional_properties = Some(Box::new(Schema::Bool(false)));
        object.into()
    }

    fn is_referenceable() -> bool {
        true
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
    fn kvmap_transform_schema_0_8(gen: &mut SchemaGenerator, schema: &mut Schema) {
        let mut parents = Vec::new();

        Self::kvmap_transform_schema_impl_0_8(gen, schema, &mut parents, 0);
    }

    fn kvmap_transform_schema_impl_0_8(
        gen: &mut SchemaGenerator,
        schema: &mut Schema,
        parents: &mut Vec<String>,
        depth: u32,
    ) {
        if depth > 8 {
            return;
        }

        let mut done = false;
        let schema = match schema {
            Schema::Object(schema) => schema,
            _ => return,
        };

        // The schema is a reference to a schema defined elsewhere.
        //
        // If possible we replace it with its definition but if that is not
        // available then we give up and leave it as-is.
        let mut parents = if let Some(reference) = &schema.reference {
            let name = match reference.strip_prefix(&gen.settings().definitions_path) {
                Some(name) => name,
                // Reference is defined elsewhere, nothing we can do.
                None => return,
            };

            // We are in a recursive reference loop. No point in continuing.
            if parents.iter().any(|parent| parent == name) {
                return;
            }

            let name = name.to_owned();
            *schema = match gen.definitions().get(&name) {
                Some(Schema::Object(schema)) => schema.clone(),
                _ => return,
            };

            parents.push(name);
            utils::DropGuard::new(parents, |parents| drop(parents.pop()))
        } else {
            utils::DropGuard::unguarded(parents)
        };

        if let Some(object) = &mut schema.object {
            // For objects KeyValueMap uses the $key$ property so we need to remove it from
            // the inner schema.

            done |= object.properties.remove("$key$").is_some();
            done |= object.required.remove("$key$");

            if let Some(max) = &mut object.max_properties {
                *max = max.saturating_sub(1);
            }

            if let Some(min) = &mut object.min_properties {
                *min = min.saturating_sub(1);
            }
        }

        if let Some(array) = &mut schema.array {
            // For arrays KeyValueMap uses the first array element so we need to remove it
            // from the inner schema.

            if let Some(SingleOrVec::Vec(items)) = &mut array.items {
                // If the array is empty then the leading element may be following the
                // additionalItem schema. In that case we do nothing.
                if !items.is_empty() {
                    items.remove(0);
                    done = true;
                }
            }

            if let Some(max) = &mut array.max_items {
                *max = max.saturating_sub(1);
            }

            if let Some(min) = &mut array.min_items {
                *min = min.saturating_sub(1);
            }
        }

        // We've already modified the schema so there's no need to do more work.
        if done {
            return;
        }

        let subschemas = match &mut schema.subschemas {
            Some(subschemas) => subschemas,
            None => return,
        };

        if let Some(one_of) = &mut subschemas.one_of {
            for subschema in one_of {
                Self::kvmap_transform_schema_impl_0_8(gen, subschema, &mut parents, depth + 1);
            }
        }

        if let Some(any_of) = &mut subschemas.any_of {
            for subschema in any_of {
                Self::kvmap_transform_schema_impl_0_8(gen, subschema, &mut parents, depth + 1);
            }
        }

        if let Some(all_of) = &mut subschemas.all_of {
            for subschema in all_of {
                Self::kvmap_transform_schema_impl_0_8(gen, subschema, &mut parents, depth + 1);
            }
        }
    }
}

impl<T, TA> JsonSchemaAs<Vec<T>> for KeyValueMap<TA>
where
    TA: JsonSchemaAs<T>,
{
    fn schema_name() -> String {
        std::format!("KeyValueMap({})", <WrapSchema<T, TA>>::schema_name())
    }

    fn schema_id() -> Cow<'static, str> {
        std::format!(
            "serde_with::KeyValueMap({})",
            <WrapSchema<T, TA>>::schema_id()
        )
        .into()
    }

    fn json_schema(gen: &mut SchemaGenerator) -> Schema {
        let mut value = <WrapSchema<T, TA>>::json_schema(gen);
        <WrapSchema<Vec<T>, KeyValueMap<TA>>>::kvmap_transform_schema_0_8(gen, &mut value);

        SchemaObject {
            instance_type: Some(InstanceType::Object.into()),
            object: Some(Box::new(ObjectValidation {
                additional_properties: Some(Box::new(value)),
                ..Default::default()
            })),
            ..Default::default()
        }
        .into()
    }

    fn is_referenceable() -> bool {
        true
    }
}

impl<K, V, KA, VA, const N: usize> JsonSchemaAs<[(K, V); N]> for Map<KA, VA>
where
    VA: JsonSchemaAs<V>,
{
    forward_schema!(WrapSchema<BTreeMap<K, V>, BTreeMap<KA, VA>>);
}

macro_rules! map_first_last_wins_schema {
    ($(=> $extra:ident)? $type:ty) => {
        impl<K, V, $($extra,)? KA, VA> JsonSchemaAs<$type> for MapFirstKeyWins<KA, VA>
        where
            VA: JsonSchemaAs<V>,
        {
            forward_schema!(BTreeMap<WrapSchema<K, KA>, WrapSchema<V, VA>>);
        }

        impl<K, V, $($extra,)? KA, VA> JsonSchemaAs<$type> for MapPreventDuplicates<KA, VA>
        where
            VA: JsonSchemaAs<V>,
        {
            forward_schema!(BTreeMap<WrapSchema<K, KA>, WrapSchema<V, VA>>);
        }
    }
}

map_first_last_wins_schema!(BTreeMap<K, V>);
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
    fn schema_name() -> String {
        std::format!(
            "OneOrMany({},PreferOne)",
            <WrapSchema<T, TA>>::schema_name()
        )
    }

    fn schema_id() -> Cow<'static, str> {
        std::format!(
            "serde_with::OneOrMany({},PreferOne)",
            <WrapSchema<T, TA>>::schema_id()
        )
        .into()
    }

    fn json_schema(gen: &mut SchemaGenerator) -> Schema {
        let single = gen.subschema_for::<WrapSchema<T, TA>>();
        let array = SchemaObject {
            instance_type: Some(InstanceType::Array.into()),
            array: Some(Box::new(ArrayValidation {
                items: Some(single.clone().into()),
                ..Default::default()
            })),
            ..Default::default()
        };

        SchemaObject {
            subschemas: Some(Box::new(SubschemaValidation {
                any_of: Some(std::vec![single, array.into()]),
                ..Default::default()
            })),
            ..Default::default()
        }
        .into()
    }
}

impl<T, TA> JsonSchemaAs<Vec<T>> for OneOrMany<TA, PreferMany>
where
    TA: JsonSchemaAs<T>,
{
    fn schema_name() -> String {
        std::format!(
            "OneOrMany<{}, PreferMany>",
            <WrapSchema<T, TA>>::schema_name()
        )
    }

    fn schema_id() -> Cow<'static, str> {
        std::format!(
            "serde_with::OneOrMany<{}, PreferMany>",
            <WrapSchema<T, TA>>::schema_id()
        )
        .into()
    }

    fn json_schema(gen: &mut SchemaGenerator) -> Schema {
        let inner = gen.subschema_for::<WrapSchema<T, TA>>();
        let single = SchemaObject {
            metadata: Some(Box::new(Metadata {
                write_only: true,
                ..Default::default()
            })),
            subschemas: Some(Box::new(SubschemaValidation {
                all_of: Some(std::vec![inner.clone()]),
                ..Default::default()
            })),
            ..Default::default()
        };
        let array = SchemaObject {
            instance_type: Some(InstanceType::Array.into()),
            array: Some(Box::new(ArrayValidation {
                items: Some(Schema::from(single.clone()).into()),
                ..Default::default()
            })),
            ..Default::default()
        };

        SchemaObject {
            subschemas: Some(Box::new(SubschemaValidation {
                any_of: Some(std::vec![single.into(), array.into()]),
                ..Default::default()
            })),
            ..Default::default()
        }
        .into()
    }
}

macro_rules! schema_for_pickfirst {
    ($( $param:ident )+) => {
        impl<T, $($param,)+> JsonSchemaAs<T> for PickFirst<($( $param, )+)>
        where
            $( $param: JsonSchemaAs<T>, )+
        {
            fn schema_name() -> String {
                std::format!(
                    concat!(
                        "PickFirst(",
                        $( "{", stringify!($param), "}", )+
                        ")"
                    ),
                    $( $param = <WrapSchema<T, $param>>::schema_name(), )+
                )
            }

            fn schema_id() -> Cow<'static, str> {
                std::format!(
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
                let subschemas = std::vec![$(
                    {
                        let is_first = std::mem::replace(&mut first, false);
                        let schema = g.subschema_for::<WrapSchema<T, $param>>();

                        if !is_first {
                            SchemaObject {
                                metadata: Some(Box::new(Metadata {
                                    write_only: true,
                                    ..Default::default()
                                })),
                                subschemas: Some(Box::new(SubschemaValidation {
                                    all_of: Some(std::vec![schema]),
                                    ..Default::default()
                                })),
                                ..Default::default()
                            }
                            .into()
                        } else {
                            schema
                        }
                    }
                ),+];

                SchemaObject {
                    subschemas: Some(Box::new(SubschemaValidation {
                        any_of: Some(subschemas),
                        ..Default::default()
                    })),
                    ..Default::default()
                }
                .into()
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
        std::format!(
            "serde_with::SetLastValueWins<{}>",
            <WrapSchema<T, TA> as JsonSchema>::schema_id()
        )
        .into()
    }

    fn schema_name() -> String {
        std::format!(
            "SetLastValueWins<{}>",
            <WrapSchema<T, TA> as JsonSchema>::schema_name()
        )
    }

    fn json_schema(gen: &mut SchemaGenerator) -> Schema {
        let schema = <WrapSchema<T, TA> as JsonSchema>::json_schema(gen);
        let mut schema = schema.into_object();

        // We explicitly allow duplicate items since the whole point of
        // SetLastValueWins is to take the duplicate value.
        if let Some(array) = &mut schema.array {
            array.unique_items = None;
        }

        schema.into()
    }

    fn is_referenceable() -> bool {
        false
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

    declare_timespan_target!(SystemTime { i64, f64, String });

    #[cfg(feature = "chrono_0_4")]
    declare_timespan_target!(::chrono_0_4::Duration { i64, f64, String });
    #[cfg(feature = "chrono_0_4")]
    declare_timespan_target!(::chrono_0_4::DateTime<::chrono_0_4::Utc> { i64, f64, String });
    #[cfg(feature = "chrono_0_4")]
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
        use ::schemars_0_8::schema::StringValidation;

        let mut number = SchemaObject {
            instance_type: Some(InstanceType::Number.into()),
            number: (!signed).then(|| {
                Box::new(NumberValidation {
                    minimum: Some(0.0),
                    ..Default::default()
                })
            }),
            ..Default::default()
        };

        // This is a more lenient version of the regex used to determine
        // whether JSON numbers are valid. Specifically, it allows multiple
        // leading zeroes whereas that is illegal in JSON.
        let regex = r#"[0-9]+(\.[0-9]+)?([eE][+-]?[0-9]+)?"#;
        let mut string = SchemaObject {
            instance_type: Some(InstanceType::String.into()),
            string: Some(Box::new(StringValidation {
                pattern: Some(match signed {
                    true => std::format!("^-?{regex}$"),
                    false => std::format!("^{regex}$"),
                }),
                ..Default::default()
            })),
            ..Default::default()
        };

        if matches!(self, Self::String) {
            number.metadata().write_only = true;
        } else {
            string.metadata().write_only = true;
        }

        SchemaObject {
            subschemas: Some(Box::new(SubschemaValidation {
                one_of: Some(std::vec![number.into(), string.into()]),
                ..Default::default()
            })),
            ..Default::default()
        }
        .into()
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
    fn schema_name() -> String {
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

    fn is_referenceable() -> bool {
        false
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
