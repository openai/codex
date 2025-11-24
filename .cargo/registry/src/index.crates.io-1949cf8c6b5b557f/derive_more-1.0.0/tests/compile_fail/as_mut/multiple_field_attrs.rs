#[derive(derive_more::AsMut)]
struct Foo {
    #[as_mut]
    #[as_mut(forward)]
    bar: i32,
}

fn main() {}
