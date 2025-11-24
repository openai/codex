#[derive(derive_more::Display)]
#[display(bound = "String: std::fmt::Display")]
pub struct Foo {
    bar: String,
}

fn main() {}
