use schemars::JsonSchema;

#[derive(JsonSchema)]
#[validate(email)]
pub struct Struct1(#[validate(regex, foo, length(min = 1, equal = 2, bar))] String);

#[derive(JsonSchema)]
#[schemars(email)]
pub struct Struct2(#[schemars(regex, foo, length(min = 1, equal = 2, bar))] String);

#[derive(JsonSchema)]
pub struct Struct3(
    #[validate(
        regex = "foo",
        contains = "bar",
        regex(pattern = "baz"),
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
        regex(pattern = "baz"),
        phone,
        email(code = "code_str", message = "message"),
        email = "foo",
        email,
        email,
        url
    )]
    String,
);

fn main() {}
