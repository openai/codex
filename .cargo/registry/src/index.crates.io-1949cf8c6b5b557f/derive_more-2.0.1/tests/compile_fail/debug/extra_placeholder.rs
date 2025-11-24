#[derive(derive_more::Debug)]
pub struct Foo {
    #[debug("Stuff({}): {}", bar)]
    bar: String,
}

fn main() {}
