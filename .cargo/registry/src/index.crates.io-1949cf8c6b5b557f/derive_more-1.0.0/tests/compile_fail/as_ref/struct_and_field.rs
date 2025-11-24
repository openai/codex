#[derive(derive_more::AsRef)]
#[as_ref(forward)]
struct Foo {
    #[as_ref]
    bar: i32,
}

fn main() {}
