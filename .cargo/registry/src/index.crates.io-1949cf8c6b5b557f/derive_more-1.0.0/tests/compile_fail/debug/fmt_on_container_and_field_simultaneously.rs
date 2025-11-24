#[derive(derive_more::Debug)]
#[debug("{bar}")]
pub struct Foo {
    #[debug("{bar}")]
    bar: String,
}

fn main() {}
