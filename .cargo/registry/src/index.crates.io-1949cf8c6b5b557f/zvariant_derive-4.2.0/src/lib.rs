#![deny(rust_2018_idioms)]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/dbus2/zbus/9f7a90d2b594ddc48b7a5f39fda5e00cd56a7dfb/logo.png"
)]
#![doc = include_str!("../README.md")]
#![doc(test(attr(
    warn(unused),
    deny(warnings),
    allow(dead_code),
    // W/o this, we seem to get some bogus warning about `extern crate zbus`.
    allow(unused_extern_crates),
)))]

use proc_macro::TokenStream;
use syn::DeriveInput;

mod dict;
mod r#type;
mod utils;
mod value;

/// Derive macro to add [`Type`] implementation to structs and enums.
///
/// # Examples
///
/// For structs it works just like serde's [`Serialize`] and [`Deserialize`] macros:
///
/// ```
/// use zvariant::{serialized::Context, to_bytes, Type, LE};
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Deserialize, Serialize, Type, PartialEq, Debug)]
/// struct Struct<'s> {
///     field1: u16,
///     field2: i64,
///     field3: &'s str,
/// }
///
/// assert_eq!(Struct::signature(), "(qxs)");
/// let s = Struct {
///     field1: 42,
///     field2: i64::max_value(),
///     field3: "hello",
/// };
/// let ctxt = Context::new_dbus(LE, 0);
/// let encoded = to_bytes(ctxt, &s).unwrap();
/// let decoded: Struct = encoded.deserialize().unwrap().0;
/// assert_eq!(decoded, s);
/// ```
///
/// Same with enum, except that all variants of the enum must have the same number and types of
/// fields (if any). If you want the encoding size of the (unit-type) enum to be dictated by
/// `repr` attribute (like in the example below), you'll also need [serde_repr] crate.
///
/// ```
/// use zvariant::{serialized::Context, to_bytes, Type, LE};
/// use serde::{Deserialize, Serialize};
/// use serde_repr::{Deserialize_repr, Serialize_repr};
///
/// #[repr(u8)]
/// #[derive(Deserialize_repr, Serialize_repr, Type, Debug, PartialEq)]
/// enum Enum {
///     Variant1,
///     Variant2,
/// }
/// assert_eq!(Enum::signature(), u8::signature());
/// let ctxt = Context::new_dbus(LE, 0);
/// let encoded = to_bytes(ctxt, &Enum::Variant2).unwrap();
/// let decoded: Enum = encoded.deserialize().unwrap().0;
/// assert_eq!(decoded, Enum::Variant2);
///
/// #[repr(i64)]
/// #[derive(Deserialize_repr, Serialize_repr, Type)]
/// enum Enum2 {
///     Variant1,
///     Variant2,
/// }
/// assert_eq!(Enum2::signature(), i64::signature());
///
/// // w/o repr attribute, u32 representation is chosen
/// #[derive(Deserialize, Serialize, Type)]
/// enum NoReprEnum {
///     Variant1,
///     Variant2,
/// }
/// assert_eq!(NoReprEnum::signature(), u32::signature());
///
/// // Not-unit enums are represented as a structure, with the first field being a u32 denoting the
/// // variant and the second as the actual value.
/// #[derive(Deserialize, Serialize, Type)]
/// enum NewType {
///     Variant1(f64),
///     Variant2(f64),
/// }
/// assert_eq!(NewType::signature(), "(ud)");
///
/// #[derive(Deserialize, Serialize, Type)]
/// enum StructFields {
///     Variant1(u16, i64, &'static str),
///     Variant2 { field1: u16, field2: i64, field3: &'static str },
/// }
/// assert_eq!(StructFields::signature(), "(u(qxs))");
/// ```
///
/// # Custom signatures
///
/// There are times when you'd find yourself wanting to specify a hardcoded signature yourself for
/// the type. The `signature` attribute exists for this purpose. A typical use case is when you'd
/// need to encode your type as a dictionary (signature `a{sv}`) type. For convenience, `dict` is
/// an alias for `a{sv}`. Here is an example:
///
/// ```
/// use zvariant::{SerializeDict, DeserializeDict, serialized::Context, to_bytes, Type, LE};
///
/// #[derive(DeserializeDict, SerializeDict, Type, PartialEq, Debug)]
/// // `#[zvariant(signature = "a{sv}")]` would be the same.
/// #[zvariant(signature = "dict")]
/// struct Struct {
///     field1: u16,
///     field2: i64,
///     field3: String,
/// }
///
/// assert_eq!(Struct::signature(), "a{sv}");
/// let s = Struct {
///     field1: 42,
///     field2: i64::max_value(),
///     field3: "hello".to_string(),
/// };
/// let ctxt = Context::new_dbus(LE, 0);
/// let encoded = to_bytes(ctxt, &s).unwrap();
/// let decoded: Struct = encoded.deserialize().unwrap().0;
/// assert_eq!(decoded, s);
/// ```
///
/// Another common use for custom signatures is (de)serialization of unit enums as strings:
///
/// ```
/// use zvariant::{serialized::Context, to_bytes, Type, LE};
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Deserialize, Serialize, Type, PartialEq, Debug)]
/// #[zvariant(signature = "s")]
/// enum StrEnum {
///     Variant1,
///     Variant2,
///     Variant3,
/// }
///
/// assert_eq!(StrEnum::signature(), "s");
/// let ctxt = Context::new_dbus(LE, 0);
/// let encoded = to_bytes(ctxt, &StrEnum::Variant2).unwrap();
/// assert_eq!(encoded.len(), 13);
/// let decoded: StrEnum = encoded.deserialize().unwrap().0;
/// assert_eq!(decoded, StrEnum::Variant2);
/// ```
///
/// [`Type`]: https://docs.rs/zvariant/latest/zvariant/trait.Type.html
/// [`Serialize`]: https://docs.serde.rs/serde/trait.Serialize.html
/// [`Deserialize`]: https://docs.serde.rs/serde/de/trait.Deserialize.html
/// [serde_repr]: https://crates.io/crates/serde_repr
#[proc_macro_derive(Type, attributes(zvariant))]
pub fn type_macro_derive(input: TokenStream) -> TokenStream {
    let ast: DeriveInput = syn::parse(input).unwrap();
    r#type::expand_derive(ast)
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}

/// Adds [`Serialize`] implementation to structs to be serialized as `a{sv}` type.
///
/// This macro serializes the deriving struct as a D-Bus dictionary type, where keys are strings and
/// values are generic values. Such dictionary types are very commonly used with
/// [D-Bus](https://dbus.freedesktop.org/doc/dbus-specification.html#standard-interfaces-properties)
/// and GVariant.
///
/// # Examples
///
/// For structs it works just like serde's [`Serialize`] macros:
///
/// ```
/// use zvariant::{SerializeDict, Type};
///
/// #[derive(SerializeDict, Type)]
/// #[zvariant(signature = "a{sv}")]
/// struct Struct {
///     field1: u16,
///     #[zvariant(rename = "another-name")]
///     field2: i64,
///     optional_field: Option<String>,
/// }
/// ```
///
/// The serialized D-Bus version of `Struct {42, 77, None}`
/// will be `{"field1": Value::U16(42), "another-name": Value::I64(77)}`.
///
/// # Auto renaming fields
///
/// The macro supports specifying a Serde-like `#[zvariant(rename_all = "case")]` attribute on
/// structures. The attribute allows to rename all the fields from snake case to another case
/// automatically:
///
/// ```
/// use zvariant::{SerializeDict, Type};
///
/// #[derive(SerializeDict, Type)]
/// #[zvariant(signature = "a{sv}", rename_all = "PascalCase")]
/// struct Struct {
///     field1: u16,
///     #[zvariant(rename = "another-name")]
///     field2: i64,
///     optional_field: Option<String>,
/// }
/// ```
///
/// It's still possible to specify custom names for individual fields using the
/// `#[zvariant(rename = "another-name")]` attribute even when the `rename_all` attribute is
/// present.
///
/// Currently the macro supports the following values for `case`:
///
/// * `"lowercase"`
/// * `"UPPERCASE"`
/// * `"PascalCase"`
/// * `"camelCase"`
/// * `"snake_case"`
/// * `"kebab-case"`
///
/// [`Serialize`]: https://docs.serde.rs/serde/trait.Serialize.html
#[proc_macro_derive(SerializeDict, attributes(zvariant))]
pub fn serialize_dict_macro_derive(input: TokenStream) -> TokenStream {
    let input: DeriveInput = syn::parse(input).unwrap();
    dict::expand_serialize_derive(input)
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}

/// Adds [`Deserialize`] implementation to structs to be deserialized from `a{sv}` type.
///
/// This macro deserializes a D-Bus dictionary type as a struct, where keys are strings and values
/// are generic values. Such dictionary types are very commonly used with
/// [D-Bus](https://dbus.freedesktop.org/doc/dbus-specification.html#standard-interfaces-properties)
/// and GVariant.
///
/// # Examples
///
/// For structs it works just like serde's [`Deserialize`] macros:
///
/// ```
/// use zvariant::{DeserializeDict, Type};
///
/// #[derive(DeserializeDict, Type)]
/// #[zvariant(signature = "a{sv}")]
/// ##[allow(unused)]
/// struct Struct {
///     field1: u16,
///     #[zvariant(rename = "another-name")]
///     field2: i64,
///     optional_field: Option<String>,
/// }
/// ```
///
/// The deserialized D-Bus dictionary `{"field1": Value::U16(42), "another-name": Value::I64(77)}`
/// will be `Struct {42, 77, None}`.
///
/// # Auto renaming fields
///
/// The macro supports specifying a Serde-like `#[zvariant(rename_all = "case")]` attribute on
/// structures. The attribute allows to rename all the fields from snake case to another case
/// automatically:
///
/// ```
/// use zvariant::{SerializeDict, Type};
///
/// #[derive(SerializeDict, Type)]
/// #[zvariant(signature = "a{sv}", rename_all = "PascalCase")]
/// struct Struct {
///     field1: u16,
///     #[zvariant(rename = "another-name")]
///     field2: i64,
///     optional_field: Option<String>,
/// }
/// ```
///
/// It's still possible to specify custom names for individual fields using the
/// `#[zvariant(rename = "another-name")]` attribute even when the `rename_all` attribute is
/// present.
///
/// Currently the macro supports the following values for `case`:
///
/// * `"lowercase"`
/// * `"UPPERCASE"`
/// * `"PascalCase"`
/// * `"camelCase"`
/// * `"snake_case"`
/// * `"kebab-case"`
///
/// [`Deserialize`]: https://docs.serde.rs/serde/de/trait.Deserialize.html
#[proc_macro_derive(DeserializeDict, attributes(zvariant))]
pub fn deserialize_dict_macro_derive(input: TokenStream) -> TokenStream {
    let input: DeriveInput = syn::parse(input).unwrap();
    dict::expand_deserialize_derive(input)
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}

/// Implements conversions for your type to/from [`Value`].
///
/// Implements `TryFrom<Value>` and `Into<Value>` for your type.
///
/// # Examples
///
/// Simple owned strutures:
///
/// ```
/// use zvariant::{OwnedObjectPath, OwnedValue, Value};
///
/// #[derive(Clone, Value, OwnedValue)]
/// struct OwnedStruct {
///     owned_str: String,
///     owned_path: OwnedObjectPath,
/// }
///
/// let s = OwnedStruct {
///     owned_str: String::from("hi"),
///     owned_path: OwnedObjectPath::try_from("/blah").unwrap(),
/// };
/// let value = Value::from(s.clone());
/// let _ = OwnedStruct::try_from(value).unwrap();
/// let value = OwnedValue::try_from(s).unwrap();
/// let s = OwnedStruct::try_from(value).unwrap();
/// assert_eq!(s.owned_str, "hi");
/// assert_eq!(s.owned_path.as_str(), "/blah");
/// ```
///
/// Now for the more exciting case of unowned structures:
///
/// ```
/// use zvariant::{ObjectPath, Str};
/// # use zvariant::{OwnedValue, Value};
/// #
/// #[derive(Clone, Value, OwnedValue)]
/// struct UnownedStruct<'a> {
///     s: Str<'a>,
///     path: ObjectPath<'a>,
/// }
///
/// let hi = String::from("hi");
/// let s = UnownedStruct {
///     s: Str::from(&hi),
///     path: ObjectPath::try_from("/blah").unwrap(),
/// };
/// let value = Value::from(s.clone());
/// let s = UnownedStruct::try_from(value).unwrap();
///
/// let value = OwnedValue::try_from(s).unwrap();
/// let s = UnownedStruct::try_from(value).unwrap();
/// assert_eq!(s.s, "hi");
/// assert_eq!(s.path, "/blah");
/// ```
///
/// Generic structures also supported:
///
/// ```
/// # use zvariant::{OwnedObjectPath, OwnedValue, Value};
/// #
/// #[derive(Clone, Value, OwnedValue)]
/// struct GenericStruct<S, O> {
///     field1: S,
///     field2: O,
/// }
///
/// let s = GenericStruct {
///     field1: String::from("hi"),
///     field2: OwnedObjectPath::try_from("/blah").unwrap(),
/// };
/// let value = Value::from(s.clone());
/// let _ = GenericStruct::<String, OwnedObjectPath>::try_from(value).unwrap();
/// let value = OwnedValue::try_from(s).unwrap();
/// let s = GenericStruct::<String, OwnedObjectPath>::try_from(value).unwrap();
/// assert_eq!(s.field1, "hi");
/// assert_eq!(s.field2.as_str(), "/blah");
/// ```
///
/// Enums also supported but currently only simple ones w/ an integer representation:
///
/// ```
/// # use zvariant::{OwnedValue, Value};
/// #
/// #[derive(Debug, PartialEq, Value, OwnedValue)]
/// #[repr(u8)]
/// enum Enum {
///     Variant1 = 1,
///     Variant2 = 2,
/// }
///
/// let value = Value::from(Enum::Variant1);
/// let e = Enum::try_from(value).unwrap();
/// assert_eq!(e, Enum::Variant1);
/// let value = OwnedValue::try_from(Enum::Variant2).unwrap();
/// let e = Enum::try_from(value).unwrap();
/// assert_eq!(e, Enum::Variant2);
/// ```
///
/// # Dictionary encoding
///
/// For treating your type as a dictionary, you can use the `signature = "dict"` attribute. See
/// [`Type`] for more details and an example use. Please note that this macro can only handle
/// `dict` or `a{sv}` values. All other values will be ignored.
///
/// [`Value`]: https://docs.rs/zvariant/latest/zvariant/enum.Value.html
/// [`Type`]: derive.Type.html#custom-types
#[proc_macro_derive(Value)]
pub fn value_macro_derive(input: TokenStream) -> TokenStream {
    let ast: DeriveInput = syn::parse(input).unwrap();
    value::expand_derive(ast, value::ValueType::Value)
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}

/// Implements conversions for your type to/from [`OwnedValue`].
///
/// Implements `TryFrom<OwnedValue>` and `TryInto<OwnedValue>` for your type.
///
/// See [`Value`] documentation for examples.
///
/// [`OwnedValue`]: https://docs.rs/zvariant/latest/zvariant/struct.OwnedValue.html
#[proc_macro_derive(OwnedValue)]
pub fn owned_value_macro_derive(input: TokenStream) -> TokenStream {
    let ast: DeriveInput = syn::parse(input).unwrap();
    value::expand_derive(ast, value::ValueType::OwnedValue)
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}
