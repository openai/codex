#[cfg(feature = "use_core")]
extern crate core;

#[macro_use]
extern crate derivative;

#[derive(Derivative)]
#[derivative(Clone = "does_not_exist")]
struct Foo;

#[derive(Derivative)]
#[derivative(Clone(does_not_exist = "true"))]
struct Bar;

fn main() {}