#[derive(derive_more::Debug)]
pub struct Foo(#[debug("Stuff({})", .0)] String);

fn main() {}
