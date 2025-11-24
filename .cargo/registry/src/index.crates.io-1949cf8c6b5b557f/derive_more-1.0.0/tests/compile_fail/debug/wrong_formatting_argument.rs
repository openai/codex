#[derive(derive_more::Debug)]
pub struct Foo {
    #[debug("Stuff({bar:M})")]
    bar: String,
}

fn main() {}
