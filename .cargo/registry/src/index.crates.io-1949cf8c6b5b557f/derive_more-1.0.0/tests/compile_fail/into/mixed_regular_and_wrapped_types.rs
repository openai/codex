#[derive(derive_more::Into)]
#[into(owned, ref(i32), i32)]
struct Foo {
    bar: i32,
}

fn main() {}
