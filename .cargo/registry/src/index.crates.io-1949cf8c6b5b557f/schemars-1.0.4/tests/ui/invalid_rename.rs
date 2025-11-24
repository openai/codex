use schemars::JsonSchema;

struct DoesNotImplJsonSchema;

#[derive(JsonSchema)]
#[schemars(rename = "} } {T} {U} {T::test} } {T")]
pub struct Struct1<T> {
    pub t: T,
}

fn main() {}
