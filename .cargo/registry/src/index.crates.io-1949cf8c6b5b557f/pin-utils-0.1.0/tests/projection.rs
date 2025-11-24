use pin_utils::{unsafe_pinned, unsafe_unpinned, pin_mut};
use std::pin::Pin;
use std::marker::Unpin;

struct Foo<T1, T2> {
    field1: T1,
    field2: T2,
}

impl<T1, T2> Foo<T1, T2> {
    unsafe_pinned!(field1: T1);
    unsafe_unpinned!(field2: T2);
}

impl<T1: Unpin, T2> Unpin for Foo<T1, T2> {} // Conditional Unpin impl

#[test]
fn projection() {
    let foo = Foo { field1: 1, field2: 2 };
    pin_mut!(foo);

    let x1: Pin<&mut i32> = foo.as_mut().field1();
    assert_eq!(*x1, 1);

    let x2: &mut i32 = foo.as_mut().field2();
    assert_eq!(*x2, 2);
}
