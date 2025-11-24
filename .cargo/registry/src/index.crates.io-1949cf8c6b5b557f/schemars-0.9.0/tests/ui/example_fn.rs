use schemars::JsonSchema;

#[derive(JsonSchema)]
#[schemars(example = "my_fn")]
pub struct Struct;

fn my_fn() {}

fn main() {}
