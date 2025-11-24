#[derive(derive_more::Display)]
#[display("Stuff({_variant:?})")]
enum Foo {
    A,
}

fn main() {}
