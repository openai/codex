#[derive(derive_more::Debug)]
#[debug(bound = "String: std::fmt::Display")]
pub struct Foo {
    bar: String,
}

fn main() {}
