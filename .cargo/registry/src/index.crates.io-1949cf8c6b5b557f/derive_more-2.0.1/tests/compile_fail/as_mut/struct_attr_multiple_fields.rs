#[derive(derive_more::AsMut)]
#[as_mut(forward)]
struct Foo {
    bar: i32,
    baz: f32,
}

fn main() {}
