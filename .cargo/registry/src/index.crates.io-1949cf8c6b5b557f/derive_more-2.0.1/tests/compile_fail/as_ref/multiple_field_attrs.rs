#[derive(derive_more::AsRef)]
struct Foo {
    #[as_ref]
    #[as_ref(forward)]
    bar: i32,
}

fn main() {}
