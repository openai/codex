#[cfg(feature = "use_core")]
extern crate core;

#[macro_use]
extern crate derivative;

trait Foo {}

fn fmt<T>(_: &T, _: &mut std::fmt::Formatter) -> std::fmt::Result {
    unimplemented!()
}

#[derive(Debug)]
struct Qux<'a, T: Foo>(&'a T);

#[derive(Derivative)]
#[derivative(Debug)]
struct _Bar<'a, T: Foo>(#[derivative(Debug(format_with="fmt"))] Qux<'a, T>);

fn main() {
}