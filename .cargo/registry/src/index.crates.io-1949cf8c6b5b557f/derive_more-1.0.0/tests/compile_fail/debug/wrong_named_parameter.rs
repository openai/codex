#[derive(derive_more::Debug)]
pub struct Foo {
    #[debug("Stuff({bars})")]
    bar: String,
}

fn main() {}
