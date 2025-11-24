use schemars::JsonSchema;

#[derive(JsonSchema)]
pub struct Struct1(#[validate(regex = 0, foo, length(min = 1, equal = 2, bar))] String);

#[derive(JsonSchema)]
pub struct Struct2(#[schemars(regex = 0, foo, length(min = 1, equal = 2, bar))] String);

#[derive(JsonSchema)]
pub struct Struct3(
    #[validate(
        regex = "foo",
        contains = "bar",
        regex(path = "baz"),
        phone,
        email,
        url
    )]
    String,
);

#[derive(JsonSchema)]
pub struct Struct4(
    #[schemars(
        regex = "foo",
        contains = "bar",
        regex(path = "baz"),
        phone,
        email,
        url
    )]
    String,
);

fn main() {}
