#[cfg(feature = "use_core")]
extern crate core;

#[macro_use]
extern crate derivative;

use std::marker::PhantomData;

struct NoClone;

#[derive(Derivative)]
#[derivative(Clone, PartialEq)]
struct PhantomField<T> {
    foo: PhantomData<T>,
}

#[derive(Derivative)]
#[derivative(Clone, PartialEq)]
struct PhantomTuple<T> {
    foo: PhantomData<(T,)>,
}

#[test]
fn main() {
    let phantom_field = PhantomField::<NoClone> { foo: Default::default() };
    let phantom_tuple = PhantomTuple::<NoClone> { foo: Default::default() };
    assert!(phantom_field == phantom_field.clone());
    assert!(phantom_tuple == phantom_tuple.clone());
}
