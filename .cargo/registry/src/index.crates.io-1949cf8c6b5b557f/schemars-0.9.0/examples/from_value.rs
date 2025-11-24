use schemars::schema_for_value;
use serde::Serialize;

#[derive(Serialize)]
pub struct MyStruct {
    pub my_int: i32,
    pub my_bool: bool,
    pub my_nullable_enum: Option<MyEnum>,
}

#[derive(Serialize)]
pub enum MyEnum {
    StringNewType(String),
    StructVariant { floats: Vec<f32> },
}

fn main() {
    let schema = schema_for_value!(MyStruct {
        my_int: 123,
        my_bool: true,
        my_nullable_enum: Some(MyEnum::StringNewType("foo".to_string()))
    });
    println!("{}", serde_json::to_string_pretty(&schema).unwrap());
}
