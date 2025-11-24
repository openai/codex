#[derive(derive_more::Display)]
#[display("Stuff({})")]
enum Foo {
    A,
}

#[derive(derive_more::Display)]
#[display("Stuff({0})")]
enum Foo2 {
    A,
}

#[derive(derive_more::Display)]
#[display("Stuff()", _0, _2)]
enum Foo3 {
    A,
}

fn main() {}
