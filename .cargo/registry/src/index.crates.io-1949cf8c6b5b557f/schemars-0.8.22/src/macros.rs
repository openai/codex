/// Generates a [`RootSchema`](crate::schema::RootSchema) for the given type using default settings.
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
        $crate::r#gen::SchemaGenerator::default().into_root_schema_for::<$type>()
    };
}

/// Generates a [`RootSchema`](crate::schema::RootSchema) for the given type using default settings.
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
        $crate::r#gen::SchemaGenerator::default().into_root_schema_for::<$type>()
    };
    ($_:expr) => {
        compile_error!("This argument to `schema_for!` is not a type - did you mean to use `schema_for_value!` instead?")
    };
}

/// Generates a [`RootSchema`](crate::schema::RootSchema) for the given example value using default settings.
///
/// The value must implement [`Serialize`](serde::Serialize). If the value also implements [`JsonSchema`](crate::JsonSchema),
/// then prefer using the [`schema_for!`](schema_for) macro which will generally produce a more precise schema,
/// particularly when the value contains any enums.
///
/// If the `Serialize` implementation of the value decides to fail, this macro will panic.
/// For a non-panicking alternative, create a [`SchemaGenerator`](crate::r#gen::SchemaGenerator) and use
/// its [`into_root_schema_for_value`](crate::r#gen::SchemaGenerator::into_root_schema_for_value) method.
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
        $crate::r#gen::SchemaGenerator::default()
            .into_root_schema_for_value(&$value)
            .unwrap()
    };
}
