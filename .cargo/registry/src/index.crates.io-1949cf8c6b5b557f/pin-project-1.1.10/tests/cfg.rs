// SPDX-License-Identifier: Apache-2.0 OR MIT

#![allow(dead_code, clippy::items_after_statements)]

// Refs: https://doc.rust-lang.org/reference/attributes.html

#[macro_use]
mod auxiliary;

use std::{marker::PhantomPinned, pin::Pin};

use pin_project::pin_project;

struct Always;

// Use this type to check that `cfg(any())` is working properly.
struct Never(PhantomPinned);

#[test]
fn cfg() {
    // structs

    #[pin_project(project_replace)]
    struct SameName {
        #[cfg(not(any()))]
        #[pin]
        inner: Always,
        #[cfg(any())]
        #[pin]
        inner: Never,
    }

    assert_unpin!(SameName);

    let _ = SameName { inner: Always };

    #[pin_project(project_replace)]
    struct DifferentName {
        #[cfg(not(any()))]
        #[pin]
        a: Always,
        #[cfg(any())]
        #[pin]
        n: Never,
    }

    assert_unpin!(DifferentName);

    let _ = DifferentName { a: Always };

    #[pin_project(project_replace)]
    struct TupleStruct(
        #[cfg(not(any()))]
        #[pin]
        Always,
        #[cfg(any())]
        #[pin]
        Never,
    );

    assert_unpin!(TupleStruct);

    let _ = TupleStruct(Always);

    // enums

    #[pin_project(
        project = VariantProj,
        project_ref = VariantProjRef,
        project_replace = VariantProjOwn,
    )]
    enum Variant {
        #[cfg(not(any()))]
        Inner(#[pin] Always),
        #[cfg(any())]
        Inner(#[pin] Never),

        #[cfg(not(any()))]
        A(#[pin] Always),
        #[cfg(any())]
        N(#[pin] Never),
    }

    assert_unpin!(Variant);

    let _ = Variant::Inner(Always);
    let _ = Variant::A(Always);

    #[pin_project(
        project = FieldProj,
        project_ref = FieldProjRef,
        project_replace = FieldProjOwn,
    )]
    enum Field {
        SameName {
            #[cfg(not(any()))]
            #[pin]
            inner: Always,
            #[cfg(any())]
            #[pin]
            inner: Never,
        },
        DifferentName {
            #[cfg(not(any()))]
            #[pin]
            a: Always,
            #[cfg(any())]
            #[pin]
            n: Never,
        },
        TupleVariant(
            #[cfg(not(any()))]
            #[pin]
            Always,
            #[cfg(any())]
            #[pin]
            Never,
        ),
    }

    assert_unpin!(Field);

    let _ = Field::SameName { inner: Always };
    let _ = Field::DifferentName { a: Always };
    let _ = Field::TupleVariant(Always);
}

#[test]
fn cfg_attr() {
    #[pin_project(project_replace)]
    struct SameCfg {
        #[cfg(not(any()))]
        #[cfg_attr(not(any()), pin)]
        inner: Always,
        #[cfg(any())]
        #[cfg_attr(any(), pin)]
        inner: Never,
    }

    assert_unpin!(SameCfg);

    let mut x = SameCfg { inner: Always };
    let x = Pin::new(&mut x).project();
    let _: Pin<&mut Always> = x.inner;

    #[pin_project(project_replace)]
    struct DifferentCfg {
        #[cfg(not(any()))]
        #[cfg_attr(any(), pin)]
        inner: Always,
        #[cfg(any())]
        #[cfg_attr(not(any()), pin)]
        inner: Never,
    }

    assert_unpin!(DifferentCfg);

    let mut x = DifferentCfg { inner: Always };
    let x = Pin::new(&mut x).project();
    let _: &mut Always = x.inner;

    #[cfg_attr(not(any()), pin_project)]
    struct Foo<T> {
        #[cfg_attr(not(any()), pin)]
        inner: T,
    }

    assert_unpin!(Foo<()>);
    assert_not_unpin!(Foo<PhantomPinned>);

    let mut x = Foo { inner: 0_u8 };
    let x = Pin::new(&mut x).project();
    let _: Pin<&mut u8> = x.inner;
}

#[test]
fn cfg_attr_any_packed() {
    // Since `cfg(any())` can never be true, it is okay for this to pass.
    #[pin_project(project_replace)]
    #[cfg_attr(any(), repr(packed))]
    struct Struct {
        #[pin]
        f: u32,
    }
}
