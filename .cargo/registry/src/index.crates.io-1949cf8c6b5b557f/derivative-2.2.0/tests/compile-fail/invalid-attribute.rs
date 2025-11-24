#[cfg(feature = "use_core")]
extern crate core;

#[macro_use]
extern crate derivative;

#[derive(Derivative)]
#[derivative(Clone = not_a_string)]
struct Foo1;

#[derive(Derivative)]
#[derivative(Clone = 1+2)]
struct Foo2;

#[derive(Derivative)]
#[derivative(Default(new = "True"))]
struct Foo3;

#[derive(Derivative)]
#[derivative(Debug(bound))]
struct Foo4;

fn main() {}