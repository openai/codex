#[derive(derive_more::From)]
#[from(i32)]
#[from(forward)]
struct Foo(i32);

fn main() {}
