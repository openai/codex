use schemars::{schema_for, JsonSchema};

/// # My Amazing Struct
/// This struct shows off generating a schema with
/// a custom title and description.
#[derive(JsonSchema)]
pub struct MyStruct {
    /// # My Amazing Integer
    pub my_int: i32,
    /// This bool has a description, but no title.
    pub my_bool: bool,
    /// # A Nullable Enum
    /// This enum might be set, or it might not.
    pub my_nullable_enum: Option<MyEnum>,
}

/// # My Amazing Enum
#[derive(JsonSchema)]
pub enum MyEnum {
    /// A wrapper around a `String`
    StringNewType(String),
    /// A struct-like enum variant which contains
    /// some floats
    StructVariant {
        /// The floats themselves
        floats: Vec<f32>,
    },
}

fn main() {
    let schema = schema_for!(MyStruct);
    println!("{}", serde_json::to_string_pretty(&schema).unwrap());
}
