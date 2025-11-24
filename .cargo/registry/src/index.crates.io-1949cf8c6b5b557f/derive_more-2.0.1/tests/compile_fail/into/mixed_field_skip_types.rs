#[derive(derive_more::Into)]
struct Foo {
    #[into(skip, i32)]
    a: i32,
}

fn main() {}
