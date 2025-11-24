#[derive(derive_more::Display)]
#[display("Stuff({})", bar)]
#[display(unknown = "unknown")]
pub struct Foo {
    bar: String,
}

fn main() {}
