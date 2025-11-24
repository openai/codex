#[derive(derive_more::AsMut)]
#[as_mut(forward)]
#[as_mut(forward)]
struct Foo {
    bar: i32,
}

fn main() {}
