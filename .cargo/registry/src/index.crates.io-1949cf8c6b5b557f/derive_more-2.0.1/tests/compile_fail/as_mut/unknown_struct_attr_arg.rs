#[derive(derive_more::AsMut)]
#[as_mut(baz)]
struct Foo {
    bar: i32,
}

fn main() {}
