#![deny(clippy::all)]

#[cfg(feature = "use_core")]
extern crate core;

#[macro_use]
extern crate derivative;

#[derive(Derivative)]
#[derivative(Clone(clone_from = "true"))]
pub struct Foo {}

fn main() {}
