use schemars::schema::{Schema, SchemaObject};
use schemars::{r#gen::SchemaGenerator, schema_for, JsonSchema};
use serde::{Deserialize, Serialize};

// `int_as_string` and `bool_as_string` use the schema for `String`.
#[derive(Default, Deserialize, Serialize, JsonSchema)]
pub struct MyStruct {
    #[serde(default = "eight", with = "as_string")]
    #[schemars(with = "String")]
    pub int_as_string: i32,

    #[serde(default = "eight")]
    pub int_normal: i32,

    #[serde(default, with = "as_string")]
    #[schemars(schema_with = "make_custom_schema")]
    pub bool_as_string: bool,

    #[serde(default)]
    pub bool_normal: bool,
}

fn make_custom_schema(generator: &mut SchemaGenerator) -> Schema {
    let mut schema: SchemaObject = <String>::json_schema(generator).into();
    schema.format = Some("boolean".to_owned());
    schema.into()
}

fn eight() -> i32 {
    8
}

// This module serializes values as strings
mod as_string {
    use serde::{de::Error, Deserialize, Deserializer, Serializer};

    pub fn serialize<T, S>(value: &T, serializer: S) -> Result<S::Ok, S::Error>
    where
        T: std::fmt::Display,
        S: Serializer,
    {
        serializer.collect_str(value)
    }

    pub fn deserialize<'de, T, D>(deserializer: D) -> Result<T, D::Error>
    where
        T: std::str::FromStr,
        D: Deserializer<'de>,
    {
        let string = String::deserialize(deserializer)?;
        string
            .parse()
            .map_err(|_| D::Error::custom("Input was not valid"))
    }
}

fn main() {
    let schema = schema_for!(MyStruct);
    println!("{}", serde_json::to_string_pretty(&schema).unwrap());
}
