use schemars::{generate::SchemaSettings, JsonSchema};
use serde::{Deserialize, Serialize};

#[derive(JsonSchema, Deserialize, Serialize)]
// The schema effectively ignores this `rename_all`, since it doesn't apply to serialization
#[serde(rename_all(deserialize = "PascalCase"))]
pub struct MyStruct {
    pub my_int: i32,
    #[serde(skip_deserializing)]
    pub my_read_only_bool: bool,
    // This property is excluded from the schema
    #[serde(skip_serializing)]
    pub my_write_only_bool: bool,
    // This property is excluded from the "required" properties of the schema, because it may be
    // be skipped during serialization
    #[serde(skip_serializing_if = "str::is_empty")]
    pub maybe_string: String,
    pub definitely_string: String,
}

fn main() {
    // By default, generated schemas describe how types are deserialized.
    // So we modify the settings here to instead generate schemas describing how it's serialized:
    let settings = SchemaSettings::default().for_serialize();

    let generator = settings.into_generator();
    let schema = generator.into_root_schema_for::<MyStruct>();
    println!("{}", serde_json::to_string_pretty(&schema).unwrap());
}
