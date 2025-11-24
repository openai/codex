#[derive(derive_more::AsRef)]
struct Foo {
    #[as_ref(baz)]
    bar: i32,
}

fn main() {}
