use schemars::{generate::SchemaSettings, JsonSchema};

#[derive(JsonSchema)]
pub struct MyStruct {
    pub my_int: i32,
    pub my_bool: bool,
    pub my_nullable_enum: Option<MyEnum>,
}

#[derive(JsonSchema)]
pub enum MyEnum {
    StringNewType(String),
    StructVariant { floats: Vec<f32> },
}

fn main() {
    let settings = SchemaSettings::draft07().with(|s| {
        s.meta_schema = None;
        s.inline_subschemas = true;
    });
    let generator = settings.into_generator();
    let schema = generator.into_root_schema_for::<MyStruct>();
    println!("{}", serde_json::to_string_pretty(&schema).unwrap());
}
