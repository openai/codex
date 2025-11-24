#[derive(derive_more::Display)]
#[display("Stuff({})", .0)]
pub struct Foo(String);

fn main() {}
