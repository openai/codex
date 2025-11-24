#[derive(derive_more::AsMut)]
struct Foo {
    #[as_mut(baz)]
    bar: i32,
}

fn main() {}
