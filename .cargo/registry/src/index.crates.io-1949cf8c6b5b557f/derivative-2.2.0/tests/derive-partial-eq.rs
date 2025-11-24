#![allow(clippy::eq_op, clippy::trivially_copy_pass_by_ref, dead_code)]

#[cfg(feature = "use_core")]
extern crate core;

use std::marker::PhantomData;

#[macro_use]
extern crate derivative;

#[derive(Derivative)]
#[derivative(PartialEq)]
struct Foo {
    foo: u8,
}

/// Test for backward compatibility.
#[derive(Derivative)]
#[derivative(PartialEq = "feature_allow_slow_enum")]
#[allow(unused)]
enum AllowsFeature<T> {
    Some(T),
    None,
}

#[derive(Derivative)]
#[derivative(PartialEq)]
enum Option<T> {
    Some(T),
    None,
}

#[derive(Derivative)]
#[derivative(PartialEq)]
enum SimpleEnum {
    Some,
    None,
}

#[derive(Derivative)]
#[derivative(PartialEq)]
enum UnitEnum {
    Single,
}

#[derive(Derivative)]
#[derivative(PartialEq)]
struct WithPtr<T: ?Sized> {
    #[derivative(PartialEq(bound = ""))]
    foo: *const T,
}

#[derive(Derivative)]
#[derivative(PartialEq)]
struct Empty;

#[derive(Derivative)]
#[derivative(PartialEq)]
struct AllIgnored {
    #[derivative(PartialEq = "ignore")]
    foo: u8,
}

#[derive(Derivative)]
#[derivative(PartialEq)]
struct OneIgnored {
    #[derivative(PartialEq = "ignore")]
    foo: u8,
    bar: u8,
}

#[derive(Derivative)]
#[derivative(PartialEq)]
struct Parity(#[derivative(PartialEq(compare_with = "same_parity"))] u8);

fn same_parity(lhs: &u8, rhs: &u8) -> bool {
    lhs % 2 == rhs % 2
}

#[derive(Derivative)]
#[derivative(PartialEq)]
struct Generic<T>(#[derivative(PartialEq(compare_with = "dummy_cmp", bound = ""))] T);

fn dummy_cmp<T>(_: &T, _: &T) -> bool {
    true
}

struct NonPartialEq;

#[derive(Derivative)]
#[derivative(PartialEq, Eq)]
struct GenericIgnore<T> {
    f: u32,
    #[derivative(PartialEq = "ignore")]
    t: PhantomData<T>,
}

trait SomeTrait {}
struct SomeType {
    #[allow(dead_code)]
    foo: u8,
}
impl SomeTrait for SomeType {}

#[test]
fn main() {
    assert!(Foo { foo: 7 } == Foo { foo: 7 });
    assert!(Foo { foo: 7 } != Foo { foo: 42 });

    let ptr1: *const dyn SomeTrait = &SomeType { foo: 0 };
    let ptr2: *const dyn SomeTrait = &SomeType { foo: 1 };
    assert!(WithPtr { foo: ptr1 } == WithPtr { foo: ptr1 });
    assert!(WithPtr { foo: ptr1 } != WithPtr { foo: ptr2 });

    assert!(Empty == Empty);
    assert!(AllIgnored { foo: 0 } == AllIgnored { foo: 42 });
    assert!(OneIgnored { foo: 0, bar: 6 } == OneIgnored { foo: 42, bar: 6 });
    assert!(OneIgnored { foo: 0, bar: 6 } != OneIgnored { foo: 42, bar: 7 });

    assert!(Option::Some(42) == Option::Some(42));
    assert!(Option::Some(0) != Option::Some(42));
    assert!(Option::Some(42) != Option::None);
    assert!(Option::None != Option::Some(42));
    assert!(Option::None::<u8> == Option::None::<u8>);

    assert!(SimpleEnum::Some == SimpleEnum::Some);
    assert!(SimpleEnum::None == SimpleEnum::None);
    assert!(SimpleEnum::Some != SimpleEnum::None);
    assert!(SimpleEnum::None != SimpleEnum::Some);

    assert!(UnitEnum::Single == UnitEnum::Single);

    assert!(Parity(3) == Parity(7));
    assert!(Parity(2) == Parity(42));
    assert!(Parity(3) != Parity(42));
    assert!(Parity(2) != Parity(7));

    assert!(Generic(SomeType { foo: 0 }) == Generic(SomeType { foo: 0 }));
    assert!(
        GenericIgnore {
            f: 123,
            t: PhantomData::<NonPartialEq>::default()
        } == GenericIgnore {
            f: 123,
            t: PhantomData::<NonPartialEq>::default()
        }
    );
}
