#[derive(derive_more::From)]
enum Foo {
    #[from(types(i32, "&str"))]
    Bar(String),
}

fn main() {}
