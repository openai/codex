#![deny(clippy::all)]

#[cfg(feature = "use_core")]
extern crate core;

#[macro_use]
extern crate derivative;

#[derive(Default, Derivative)]
#[derivative(Debug)]
pub struct Foo {
    foo: u8,
}

fn main() {
    let foo1 = Foo::default();
    println!("foo = {:?}", foo1);
}
