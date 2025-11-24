#[derive(derive_more::From)]
enum Foo {
    #[from(i32)]
    #[from(forward)]
    Bar(i32),
}

fn main() {}
