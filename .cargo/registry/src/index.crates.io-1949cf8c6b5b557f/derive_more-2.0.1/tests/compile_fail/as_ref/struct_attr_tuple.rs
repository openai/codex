#[derive(derive_more::AsRef)]
#[as_ref((i32, f32))]
struct Foo {
    bar: i32,
    baz: f32,
}

fn main() {}
