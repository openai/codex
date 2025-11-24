#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

/// The map type used by schemars types.
///
/// Currently a `BTreeMap` or `IndexMap` can be used, but this may change to a different implementation
/// with a similar interface in a future version of schemars.
/// The `IndexMap` will be used when the `preserve_order` feature flag is set.
#[cfg(not(feature = "preserve_order"))]
pub type Map<K, V> = std::collections::BTreeMap<K, V>;
#[cfg(feature = "preserve_order")]
pub type Map<K, V> = indexmap::IndexMap<K, V>;
/// The set type used by schemars types.
///
/// Currently a `BTreeSet`, but this may change to a different implementation
/// with a similar interface in a future version of schemars.
pub type Set<T> = std::collections::BTreeSet<T>;

/// A view into a single entry in a map, which may either be vacant or occupied.
//
/// This is constructed from the `entry` method on `BTreeMap` or `IndexMap`,
/// depending on whether the `preserve_order` feature flag is set.
#[cfg(not(feature = "preserve_order"))]
pub type MapEntry<'a, K, V> = std::collections::btree_map::Entry<'a, K, V>;
#[cfg(feature = "preserve_order")]
pub type MapEntry<'a, K, V> = indexmap::map::Entry<'a, K, V>;

mod flatten;
mod json_schema_impls;
mod ser;
#[macro_use]
mod macros;

/// This module is only public for use by `schemars_derive`. It should not need to be used by code
/// outside of `schemars`, and should not be considered part of the public API.
#[doc(hidden)]
pub mod _private;

pub mod r#gen;
pub mod schema;
pub mod visit;

pub use r#gen::SchemaGenerator;

#[cfg(feature = "schemars_derive")]
extern crate schemars_derive;
use std::borrow::Cow;

#[cfg(feature = "schemars_derive")]
pub use schemars_derive::*;

// Export serde_json so schemars_derive can use it
#[doc(hidden)]
pub use serde_json as _serde_json;

use schema::Schema;

/// A type which can be described as a JSON Schema document.
///
/// This is implemented for many Rust primitive and standard library types.
///
/// This can also be automatically derived on most custom types with `#[derive(JsonSchema)]`.
///
/// # Examples
/// Deriving an implementation:
/// ```
/// use schemars::{schema_for, JsonSchema};
///
/// #[derive(JsonSchema)]
/// struct MyStruct {
///     foo: i32,
/// }
///
/// let my_schema = schema_for!(MyStruct);
/// ```
///
/// When manually implementing `JsonSchema`, as well as determining an appropriate schema,
/// you will need to determine an appropriate name and ID for the type.
/// For non-generic types, the type name/path are suitable for this:
/// ```
/// use schemars::{r#gen::SchemaGenerator, schema::Schema, JsonSchema};
/// use std::borrow::Cow;
///
/// struct NonGenericType;
///
/// impl JsonSchema for NonGenericType {
///     fn schema_name() -> String {
///         // Exclude the module path to make the name in generated schemas clearer.
///         "NonGenericType".to_owned()
///     }
///
///     fn schema_id() -> Cow<'static, str> {
///         // Include the module, in case a type with the same name is in another module/crate
///         Cow::Borrowed(concat!(module_path!(), "::NonGenericType"))
///     }
///
///     fn json_schema(_generator: &mut SchemaGenerator) -> Schema {
///         todo!()
///     }
/// }
///
/// assert_eq!(NonGenericType::schema_id(), <&mut NonGenericType>::schema_id());
/// ```
///
/// But generic type parameters which may affect the generated schema should typically be included in the name/ID:
/// ```
/// use schemars::{r#gen::SchemaGenerator, schema::Schema, JsonSchema};
/// use std::{borrow::Cow, marker::PhantomData};
///
/// struct GenericType<T>(PhantomData<T>);
///
/// impl<T: JsonSchema> JsonSchema for GenericType<T> {
///     fn schema_name() -> String {
///         format!("GenericType_{}", T::schema_name())
///     }
///
///     fn schema_id() -> Cow<'static, str> {
///         Cow::Owned(format!(
///             "{}::GenericType<{}>",
///             module_path!(),
///             T::schema_id()
///         ))
///     }
///
///     fn json_schema(_generator: &mut SchemaGenerator) -> Schema {
///         todo!()
///     }
/// }
///
/// assert_eq!(<GenericType<i32>>::schema_id(), <&mut GenericType<&i32>>::schema_id());
/// ```
///
pub trait JsonSchema {
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
    /// This is used as the title for root schemas, and the key within the root's `definitions` property for subschemas.
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
    /// add them to the [`SchemaGenerator`](r#gen::SchemaGenerator)'s schema definitions.
    ///
    /// This should not return a `$ref` schema.
    fn json_schema(generator: &mut r#gen::SchemaGenerator) -> Schema;

    // TODO document and bring into public API?
    #[doc(hidden)]
    fn _schemars_private_non_optional_json_schema(generator: &mut r#gen::SchemaGenerator) -> Schema {
        Self::json_schema(generator)
    }

    // TODO document and bring into public API?
    #[doc(hidden)]
    fn _schemars_private_is_option() -> bool {
        false
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    pub fn schema_object_for<T: JsonSchema>() -> schema::SchemaObject {
        schema_object(schema_for::<T>())
    }

    pub fn schema_for<T: JsonSchema>() -> schema::Schema {
        let mut generator = r#gen::SchemaGenerator::default();
        T::json_schema(&mut generator)
    }

    pub fn schema_object(schema: schema::Schema) -> schema::SchemaObject {
        match schema {
            schema::Schema::Object(o) => o,
            s => panic!("Schema was not an object: {:?}", s),
        }
    }
}
