#[derive(derive_more::From)]
#[from(types(i32, "&str"))]
struct Foo {
    foo: String,
    bar: String,
}

fn main() {}
