use schemars::JsonSchema;

#[derive(JsonSchema)]
#[serde(
    default = 0,
    foo,
    deny_unknown_fields,
    deny_unknown_fields,
    inline = 1,
    inline,
    inline
)]
pub struct Struct1;

#[derive(JsonSchema)]
#[schemars(
    default = 0,
    foo,
    deny_unknown_fields,
    deny_unknown_fields,
    inline = 1,
    inline,
    inline
)]
pub struct Struct2;

fn main() {}
