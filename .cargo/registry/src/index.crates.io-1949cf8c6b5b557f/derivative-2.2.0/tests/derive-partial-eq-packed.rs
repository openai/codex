#![allow(clippy::eq_op, clippy::trivially_copy_pass_by_ref)]

#[cfg(feature = "use_core")]
extern crate core;

#[macro_use]
extern crate derivative;

#[derive(Derivative)]
#[derivative(PartialEq)]
#[repr(C, packed)]
struct Foo {
    foo: u8,
}

#[derive(Derivative)]
#[derivative(PartialEq)]
#[repr(C, packed)]
struct WithPtr<T: ?Sized> {
    #[derivative(PartialEq(bound = ""))]
    foo: *const T,
}

#[derive(Derivative)]
#[derivative(PartialEq)]
#[repr(C, packed)]
struct Empty;

#[derive(Derivative)]
#[derivative(PartialEq)]
#[repr(C, packed)]
struct AllIgnored {
    #[derivative(PartialEq = "ignore")]
    foo: u8,
}

#[derive(Derivative)]
#[derivative(PartialEq)]
#[repr(C, packed)]
struct OneIgnored {
    #[derivative(PartialEq = "ignore")]
    foo: u8,
    bar: u8,
}

#[derive(Derivative)]
#[derivative(PartialEq)]
#[repr(C, packed)]
struct Parity(#[derivative(PartialEq(compare_with = "same_parity"))] u8);

fn same_parity(lhs: &u8, rhs: &u8) -> bool {
    lhs % 2 == rhs % 2
}

#[derive(Derivative)]
#[derivative(PartialEq)]
#[repr(C, packed)]
struct Generic<T>(#[derivative(PartialEq(compare_with = "dummy_cmp", bound = ""))] T);

fn dummy_cmp<T>(_: &T, _: &T) -> bool {
    true
}

struct NonPartialEq;

#[derive(Derivative)]
#[derivative(PartialEq, Eq)]
#[repr(C, packed)]
struct GenericIgnore<T> {
    f: u32,
    #[derivative(PartialEq = "ignore")]
    t: T,
}

trait SomeTrait {}

#[derive(Copy, Clone)]
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

    assert!(Parity(3) == Parity(7));
    assert!(Parity(2) == Parity(42));
    assert!(Parity(3) != Parity(42));
    assert!(Parity(2) != Parity(7));

    assert!(Generic(SomeType { foo: 0 }) == Generic(SomeType { foo: 0 }));
    assert!(
        GenericIgnore {
            f: 123,
            t: NonPartialEq
        } == GenericIgnore {
            f: 123,
            t: NonPartialEq
        }
    );
}
