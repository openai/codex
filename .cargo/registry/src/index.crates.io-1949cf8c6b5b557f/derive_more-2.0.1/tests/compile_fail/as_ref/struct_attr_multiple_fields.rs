#[derive(derive_more::AsRef)]
#[as_ref(forward)]
struct Foo {
    bar: i32,
    baz: f32,
}

fn main() {}
