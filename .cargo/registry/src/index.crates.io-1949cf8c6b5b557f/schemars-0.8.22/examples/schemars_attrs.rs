use schemars::{schema_for, JsonSchema};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, JsonSchema)]
#[schemars(rename_all = "camelCase", deny_unknown_fields)]
pub struct MyStruct {
    #[serde(rename = "thisIsOverridden")]
    #[schemars(rename = "myNumber", range(min = 1, max = 10))]
    pub my_int: i32,
    pub my_bool: bool,
    #[schemars(default)]
    pub my_nullable_enum: Option<MyEnum>,
    #[schemars(inner(regex(pattern = "^x$")))]
    pub my_vec_str: Vec<String>,
}

#[derive(Deserialize, Serialize, JsonSchema)]
#[schemars(untagged)]
pub enum MyEnum {
    StringNewType(#[schemars(phone)] String),
    StructVariant {
        #[schemars(length(min = 1, max = 100))]
        floats: Vec<f32>,
    },
}

fn main() {
    let schema = schema_for!(MyStruct);
    println!("{}", serde_json::to_string_pretty(&schema).unwrap());
}
