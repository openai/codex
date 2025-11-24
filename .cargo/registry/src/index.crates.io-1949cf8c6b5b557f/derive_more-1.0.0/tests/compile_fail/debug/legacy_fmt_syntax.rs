#[derive(derive_more::Debug)]
pub struct Foo {
    #[debug(fmt = "Stuff({}): {}", "bar")]
    bar: String,
}

#[derive(derive_more::Debug)]
#[debug(fmt = "Stuff({}): {}", _0)]
pub struct Bar(String);

fn main() {}
