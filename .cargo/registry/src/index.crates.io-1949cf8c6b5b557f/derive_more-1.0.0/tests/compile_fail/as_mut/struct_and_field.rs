#[derive(derive_more::AsMut)]
#[as_mut(forward)]
struct Foo {
    #[as_mut]
    bar: i32,
}

fn main() {}
