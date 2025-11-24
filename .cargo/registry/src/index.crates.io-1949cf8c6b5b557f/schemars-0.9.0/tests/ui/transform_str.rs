use schemars::JsonSchema;

#[derive(JsonSchema)]
#[schemars(transform = "x")]
pub struct Struct;

fn main() {}
