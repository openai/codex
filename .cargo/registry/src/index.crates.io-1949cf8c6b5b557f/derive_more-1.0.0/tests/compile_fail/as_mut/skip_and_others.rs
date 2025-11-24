#[derive(derive_more::AsMut)]
struct Foo {
    #[as_mut]
    bar: i32,
    #[as_mut(ignore)]
    baz: f32,
}

fn main() {}
