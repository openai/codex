use schemars::JsonSchema;

#[derive(JsonSchema)]
#[serde(deny_unknown_fields, default)]
#[schemars(!unknown, deny_unknown_fields, !deny_unknown_fields, !default, !default, !from)]
pub struct Struct1;

fn main() {}
