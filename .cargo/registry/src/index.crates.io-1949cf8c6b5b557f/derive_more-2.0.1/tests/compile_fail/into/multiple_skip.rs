#[derive(derive_more::Into)]
struct Foo {
    #[into(skip)]
    #[into(skip)]
    a: i32,
}

fn main() {}
