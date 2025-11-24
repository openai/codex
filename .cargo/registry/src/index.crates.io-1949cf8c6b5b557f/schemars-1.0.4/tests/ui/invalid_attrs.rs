use schemars::JsonSchema;

#[derive(JsonSchema)]
#[serde(
    default = 0,
    foo,
    deny_unknown_fields,
    deny_unknown_fields,
    inline = 1,
    inline,
    inline,
    with = "String",
    serialize_with = "String",
    a::path
)]
pub struct Struct1 {
    #[serde(serialize_with = "u64")]
    pub field: u32,
}

#[derive(JsonSchema)]
#[schemars(
    default = 0,
    foo,
    deny_unknown_fields,
    deny_unknown_fields,
    inline = 1,
    inline,
    inline,
    with = "String",
    serialize_with = "String",
    a::path
)]
pub struct Struct2 {
    #[schemars(serialize_with = "u64")]
    pub field: u32,
}

fn main() {}
