use schemars::JsonSchema;

#[derive(JsonSchema)]
#[schemars(extend(x))]
#[schemars(extend("x"))]
#[schemars(extend("x" = ))]
#[schemars(extend("y" = "ok!", "y" = "duplicated!"), extend("y" = "duplicated!"))]
#[schemars(extend("y" = "duplicated!"))]
pub struct Struct;

fn main() {}
