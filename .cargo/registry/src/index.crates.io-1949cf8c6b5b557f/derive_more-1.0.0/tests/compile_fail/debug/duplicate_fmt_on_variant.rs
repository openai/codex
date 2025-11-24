#[derive(derive_more::Debug)]
pub enum Foo {
    #[debug("Test")]
    #[debug("Second")]
    Unit,
}

fn main() {}
