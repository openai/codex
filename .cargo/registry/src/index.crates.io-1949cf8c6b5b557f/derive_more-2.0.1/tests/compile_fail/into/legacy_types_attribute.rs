#[derive(derive_more::Into)]
#[into(types(i32, "&str"))]
struct Foo(String);

fn main() {}
