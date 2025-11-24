#[cfg(feature = "use_core")]
extern crate core;

#[macro_use]
extern crate derivative;

#[derive(Derivative)]
#[derivative(Debug)]
struct Foo<T, U> {
    foo: T,
    #[derivative(Debug(format_with="std::fmt::Debug::fmt"))]
    bar: U,
}

fn main() {}