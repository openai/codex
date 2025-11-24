#[derive(derive_more::Display)]
#[display(fmt = "Stuff({}): {}", "bar")]
pub struct Foo {
    bar: String,
}

#[derive(derive_more::Display)]
#[display(fmt = "Stuff({}): {}", _0)]
pub struct Bar(String);

fn main() {}
