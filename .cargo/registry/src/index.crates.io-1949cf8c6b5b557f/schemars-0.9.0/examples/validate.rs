use schemars::{schema_for, JsonSchema};

#[derive(JsonSchema)]
pub struct MyStruct {
    #[validate(range(min = 1, max = 10))]
    pub my_int: i32,
    pub my_bool: bool,
    #[validate(required)]
    pub my_nullable_enum: Option<MyEnum>,
}

#[derive(JsonSchema)]
pub enum MyEnum {
    StringNewType(#[validate(email)] String),
    StructVariant {
        #[validate(length(min = 1, max = 100))]
        floats: Vec<f32>,
    },
}

fn main() {
    let schema = schema_for!(MyStruct);
    println!("{}", serde_json::to_string_pretty(&schema).unwrap());
}
