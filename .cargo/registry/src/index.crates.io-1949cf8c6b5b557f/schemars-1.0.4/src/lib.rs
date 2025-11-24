#![forbid(unsafe_code)]
#![deny(
    missing_docs,
    unused_imports,
    clippy::cargo,
    clippy::pedantic,
    clippy::exhaustive_structs,
    clippy::exhaustive_enums
)]
#![allow(
    clippy::must_use_candidate,
    clippy::return_self_not_must_use,
    clippy::wildcard_imports,
    clippy::missing_errors_doc
)]
#![doc = include_str!("../README.md")]
#![no_std]

extern crate alloc;
#[cfg(feature = "std")]
extern crate std;

mod encoding;
mod json_schema_impls;
mod schema;
mod ser;
#[macro_use]
mod macros;

/// This module is only public for use by `schemars_derive`. It should not need to be used by code
/// outside of `schemars`, and should not be considered part of the public API.
#[doc(hidden)]
#[allow(clippy::exhaustive_structs)]
pub mod _private;
pub mod consts;
pub mod generate;
pub mod transform;

#[cfg(feature = "schemars_derive")]
extern crate schemars_derive;
use alloc::borrow::Cow;

#[cfg(feature = "schemars_derive")]
pub use schemars_derive::*;

#[doc(inline)]
pub use generate::SchemaGenerator;
pub use schema::Schema;

mod _alloc_prelude {
    pub use alloc::borrow::ToOwned;
    pub use alloc::boxed::Box;
    pub use alloc::format;
    pub use alloc::string::{String, ToString};
    pub use alloc::vec;
    pub use alloc::vec::Vec;
}

/// A type which can be described as a JSON Schema document.
///
/// This is implemented for many Rust primitive and standard library types.
///
/// This can also be automatically derived on most custom types with `#[derive(JsonSchema)]` by
/// enabling the `derive` feature flag (which is enabled by default).
/// For more info on deriving `JsonSchema`, see [the derive macro documentation](derive@JsonSchema).
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
/// use schemars::{SchemaGenerator, Schema, JsonSchema, json_schema};
/// use std::borrow::Cow;
///
/// struct NonGenericType;
///
/// impl JsonSchema for NonGenericType {
///     fn schema_name() -> Cow<'static, str> {
///         // Exclude the module path to make the name in generated schemas clearer.
///         "NonGenericType".into()
///     }
///
///     fn schema_id() -> Cow<'static, str> {
///         // Include the module, in case a type with the same name is in another module/crate
///         concat!(module_path!(), "::NonGenericType").into()
///     }
///
///     fn json_schema(_gen: &mut SchemaGenerator) -> Schema {
///         json_schema!({
///             "foo": "bar"
///         })
///     }
/// }
///
/// assert_eq!(NonGenericType::schema_id(), <&mut NonGenericType>::schema_id());
/// ```
///
/// But generic type parameters which may affect the generated schema should typically be included
/// in the name/ID:
/// ```
/// use schemars::{SchemaGenerator, Schema, JsonSchema, json_schema};
/// use std::{borrow::Cow, marker::PhantomData};
///
/// struct GenericType<T>(PhantomData<T>);
///
/// impl<T: JsonSchema> JsonSchema for GenericType<T> {
///     fn schema_name() -> Cow<'static, str> {
///         format!("GenericType_{}", T::schema_name()).into()
///     }
///
///     fn schema_id() -> Cow<'static, str> {
///         format!(
///             "{}::GenericType<{}>",
///             module_path!(),
///             T::schema_id()
///         ).into()
///     }
///
///     fn json_schema(_gen: &mut SchemaGenerator) -> Schema {
///         json_schema!({
///             "foo": "bar"
///         })
///     }
/// }
///
/// assert_eq!(<GenericType<i32>>::schema_id(), <&mut GenericType<&i32>>::schema_id());
/// ```
pub trait JsonSchema {
    /// Whether JSON Schemas generated for this type should be included directly in parent schemas,
    /// rather than being re-used where possible using the `$ref` keyword.
    ///
    /// For trivial types (such as primitives), this should return `true`. For more complex types,
    /// it should return `false`. For recursive types, this **must** return `false` to prevent
    /// infinite cycles when generating schemas.
    ///
    /// By default, this returns `false`.
    fn inline_schema() -> bool {
        false
    }

    /// The name of the generated JSON Schema.
    ///
    /// This is used as the title for root schemas, and the key within the root's `definitions`
    /// property for subschemas.
    fn schema_name() -> Cow<'static, str>;

    /// Returns a string that uniquely identifies the schema produced by this type.
    ///
    /// This does not have to be a human-readable string, and the value will not itself be included
    /// in generated schemas. If two types produce different schemas, then they **must** have
    /// different `schema_id()`s, but two types that produce identical schemas should *ideally*
    /// have the same `schema_id()`.
    ///
    /// The default implementation returns the same value as
    /// [`schema_name()`](JsonSchema::schema_name).
    fn schema_id() -> Cow<'static, str> {
        Self::schema_name()
    }

    /// Generates a JSON Schema for this type.
    ///
    /// If the returned schema depends on any [non-inlined](JsonSchema::inline_schema)
    /// schemas, then this method will add them to the [`SchemaGenerator`]'s schema definitions.
    ///
    /// This should not return a `$ref` schema.
    fn json_schema(generator: &mut SchemaGenerator) -> Schema;

    // TODO document and bring into public API?
    #[doc(hidden)]
    fn _schemars_private_non_optional_json_schema(generator: &mut SchemaGenerator) -> Schema {
        Self::json_schema(generator)
    }

    // TODO document and bring into public API?
    #[doc(hidden)]
    fn _schemars_private_is_option() -> bool {
        false
    }
}
