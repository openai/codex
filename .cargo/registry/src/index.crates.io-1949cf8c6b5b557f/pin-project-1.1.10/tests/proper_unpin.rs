// SPDX-License-Identifier: Apache-2.0 OR MIT

#![allow(dead_code)]

#[macro_use]
mod auxiliary;

pub mod default {
    use std::marker::PhantomPinned;

    use pin_project::pin_project;

    struct Inner<T> {
        f: T,
    }

    assert_unpin!(Inner<()>);
    assert_not_unpin!(Inner<PhantomPinned>);

    #[pin_project]
    struct Struct<T, U> {
        #[pin]
        f1: Inner<T>,
        f2: U,
    }

    assert_unpin!(Struct<(), ()>);
    assert_unpin!(Struct<(), PhantomPinned>);
    assert_not_unpin!(Struct<PhantomPinned, ()>);
    assert_not_unpin!(Struct<PhantomPinned, PhantomPinned>);

    #[pin_project(project = EnumProj, project_ref = EnumProjRef)]
    enum Enum<T, U> {
        V1 {
            #[pin]
            f1: Inner<T>,
            f2: U,
        },
    }

    assert_unpin!(Enum<(), ()>);
    assert_unpin!(Enum<(), PhantomPinned>);
    assert_not_unpin!(Enum<PhantomPinned, ()>);
    assert_not_unpin!(Enum<PhantomPinned, PhantomPinned>);

    #[pin_project]
    struct TrivialBounds {
        #[pin]
        f: PhantomPinned,
    }

    assert_not_unpin!(TrivialBounds);

    #[pin_project]
    struct PinRef<'a, T, U> {
        #[pin]
        f1: &'a mut Inner<T>,
        f2: U,
    }

    assert_unpin!(PinRef<'_, PhantomPinned, PhantomPinned>);
}

pub mod cfg {
    use std::marker::PhantomPinned;

    use pin_project::pin_project;

    #[pin_project]
    struct Foo<T> {
        #[cfg(any())]
        #[pin]
        f: T,
        #[cfg(not(any()))]
        f: T,
    }

    assert_unpin!(Foo<PhantomPinned>);

    #[pin_project]
    struct Bar<T> {
        #[cfg(any())]
        f: T,
        #[cfg(not(any()))]
        #[pin]
        f: T,
    }

    assert_unpin!(Bar<()>);
    assert_not_unpin!(Bar<PhantomPinned>);
}

pub mod cfg_attr {
    use std::marker::PhantomPinned;

    use pin_project::pin_project;

    #[cfg_attr(any(), pin_project)]
    struct Foo<T> {
        f: T,
    }

    assert_unpin!(Foo<()>);
    assert_not_unpin!(Foo<PhantomPinned>);

    #[cfg_attr(not(any()), pin_project)]
    struct Bar<T> {
        #[cfg_attr(not(any()), pin)]
        f: T,
    }

    assert_unpin!(Bar<()>);
    assert_not_unpin!(Bar<PhantomPinned>);
}

// pin_project(!Unpin)
pub mod not_unpin {
    use std::marker::PhantomPinned;

    use pin_project::pin_project;

    struct Inner<T> {
        f: T,
    }

    #[pin_project(!Unpin)]
    struct Struct<T, U> {
        #[pin]
        inner: Inner<T>,
        other: U,
    }

    assert_not_unpin!(Struct<(), ()>);
    assert_not_unpin!(Struct<(), PhantomPinned>);
    assert_not_unpin!(Struct<PhantomPinned, ()>);
    assert_not_unpin!(Struct<PhantomPinned, PhantomPinned>);

    #[pin_project(!Unpin)]
    struct TrivialBounds {
        #[pin]
        f: PhantomPinned,
    }

    assert_not_unpin!(TrivialBounds);

    #[pin_project(!Unpin)]
    struct PinRef<'a, T, U> {
        #[pin]
        inner: &'a mut Inner<T>,
        other: U,
    }

    assert_not_unpin!(PinRef<'_, (), ()>);
}
