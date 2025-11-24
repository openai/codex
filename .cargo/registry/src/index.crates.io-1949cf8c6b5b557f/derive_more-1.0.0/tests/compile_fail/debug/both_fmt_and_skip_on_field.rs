#[derive(derive_more::Debug)]
pub struct Foo {
    #[debug("Stuff({}): {}", bar)]
    #[debug(skip)]
    bar: String,
}

fn main() {}
