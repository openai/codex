#[derive(derive_more::AsMut)]
#[as_mut((i32, f32))]
struct Foo {
    bar: i32,
    baz: f32,
}

fn main() {}
