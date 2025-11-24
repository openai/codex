/// Generates a [`Schema`](crate::Schema) for the given type using default settings.
/// The default settings currently conform to [JSON Schema 2020-12](https://json-schema.org/specification-links#2020-12), but this is liable to change in a future version of Schemars if support for other JSON Schema versions is added.
///
/// The type must implement [`JsonSchema`](crate::JsonSchema).
///
/// # Example
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
#[cfg(doc)]
#[macro_export]
macro_rules! schema_for {
    ($type:ty) => {
        $crate::SchemaGenerator::default().into_root_schema_for::<$type>()
    };
}

/// Generates a [`Schema`](crate::Schema) for the given type using default settings.
/// The default settings currently conform to [JSON Schema 2020-12](https://json-schema.org/specification-links#2020-12), but this is liable to change in a future version of Schemars if support for other JSON Schema versions is added.
///
/// The type must implement [`JsonSchema`](crate::JsonSchema).
///
/// # Example
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
#[cfg(not(doc))]
#[macro_export]
macro_rules! schema_for {
    ($type:ty) => {
        $crate::SchemaGenerator::default().into_root_schema_for::<$type>()
    };
    ($_:expr) => {
        compile_error!("This argument to `schema_for!` is not a type - did you mean to use `schema_for_value!` instead?")
    };
}

/// Generates a [`Schema`](crate::Schema) for the given example value using default settings.
/// The default settings currently conform to [JSON Schema 2020-12](https://json-schema.org/specification-links#2020-12), but this is liable to change in a future version of Schemars if support for other JSON Schema versions is added.
///
/// The value must implement [`Serialize`](serde::Serialize). If the value also implements
/// [`JsonSchema`](crate::JsonSchema), then prefer using the [`schema_for!(Type)`](schema_for) macro
/// which will generally produce a more precise schema, particularly when the value contains any
/// enums.
///
/// If the `Serialize` implementation of the value decides to fail, this macro will panic.
/// For a non-panicking alternative, create a [`SchemaGenerator`](crate::SchemaGenerator) and use
/// its [`into_root_schema_for_value`](crate::SchemaGenerator::into_root_schema_for_value) method.
///
/// # Example
/// ```
/// use schemars::schema_for_value;
///
/// #[derive(serde::Serialize)]
/// struct MyStruct {
///     foo: i32,
/// }
///
/// let my_schema = schema_for_value!(MyStruct { foo: 123 });
/// ```
#[macro_export]
macro_rules! schema_for_value {
    ($value:expr) => {
        $crate::SchemaGenerator::default()
            .into_root_schema_for_value(&$value)
            .unwrap()
    };
}

/// Construct a [`Schema`](crate::Schema) from a JSON literal. This can either be a JSON object, or
/// a boolean (`true` or `false`).
///
/// You can interpolate variables or expressions into a JSON object using the same rules as the
/// [`serde_json::json`] macro.
///
/// # Example
/// ```
/// use schemars::{Schema, json_schema};
///
/// let desc = "A helpful description.";
/// let my_schema: Schema = json_schema!({
///     "description": desc,
///     "type": ["object", "null"]
/// });
/// ```
#[macro_export]
macro_rules! json_schema {
    (
        {$($json_object:tt)*}
    ) => {
        <$crate::Schema as ::core::convert::TryFrom<_>>::try_from($crate::_private::serde_json::json!({$($json_object)*})).unwrap()
    };
    (true) => {
        $crate::Schema::from(true)
    };
    (false) => {
        $crate::Schema::from(false)
    };
}
