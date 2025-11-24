#[derive(derive_more::Debug)]
pub struct Foo(#[debug("Stuff({_1})")] String);

fn main() {}
