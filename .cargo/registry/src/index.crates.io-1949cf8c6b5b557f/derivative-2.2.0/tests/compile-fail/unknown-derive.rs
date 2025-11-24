#[cfg(feature = "use_core")]
extern crate core;

#[macro_use]
extern crate derivative;

#[derive(Derivative)]
#[derivative(DoesNotExist1)]
struct Foo;

#[derive(Derivative)]
#[derivative(DoesNotExist2(with_some="argument"))]
struct Bar;

#[derive(Derivative)]
#[derivative(Debug)]
struct Baz {
    #[derivative(DoesNotExist3)]
    _baz: (),
}

fn main() {}