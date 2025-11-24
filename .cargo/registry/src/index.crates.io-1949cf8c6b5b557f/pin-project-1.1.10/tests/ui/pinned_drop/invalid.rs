// SPDX-License-Identifier: Apache-2.0 OR MIT

mod argument {
    use std::pin::Pin;

    use pin_project::{pin_project, pinned_drop};

    #[pin_project(PinnedDrop)]
    struct UnexpectedArg1(());

    #[pinned_drop(foo)] //~ ERROR unexpected argument
    impl PinnedDrop for UnexpectedArg1 {
        fn drop(self: Pin<&mut Self>) {}
    }

    #[pin_project(PinnedDrop)]
    struct UnexpectedArg2(());

    #[pinned_drop()] // Ok
    impl PinnedDrop for UnexpectedArg2 {
        fn drop(self: Pin<&mut Self>) {}
    }
}

mod attribute {
    use pin_project::{pin_project, pinned_drop};

    #[pin_project(PinnedDrop)]
    struct Duplicate(());

    #[pinned_drop]
    #[pinned_drop] //~ ERROR duplicate #[pinned_drop] attribute
    impl PinnedDrop for Duplicate {
        fn drop(self: Pin<&mut Self>) {}
    }
}

mod item {
    use pin_project::{pin_project, pinned_drop};

    #[pin_project(PinnedDrop)]
    struct TraitImpl(());

    #[pinned_drop]
    impl Drop for TraitImpl {} //~ ERROR may only be used on implementation for the `PinnedDrop` trait

    #[pin_project(PinnedDrop)]
    struct InherentImpl(());

    #[pinned_drop]
    impl InherentImpl {} //~ ERROR may only be used on implementation for the `PinnedDrop` trait

    #[pinned_drop]
    fn func(_: Pin<&mut ()>) {} //~ ERROR expected `impl`
}

mod unsafety {
    use pin_project::{pin_project, pinned_drop};

    #[pin_project(PinnedDrop)]
    struct Impl(());

    #[pinned_drop]
    unsafe impl PinnedDrop for Impl {
        //~^ ERROR implementing the trait `PinnedDrop` is not unsafe
        fn drop(self: Pin<&mut Self>) {}
    }

    #[pin_project(PinnedDrop)]
    struct Method(());

    #[pinned_drop]
    impl PinnedDrop for Method {
        unsafe fn drop(self: Pin<&mut Self>) {} //~ ERROR implementing the method `drop` is not unsafe
    }
}

mod assoc_item {
    use pin_project::{pin_project, pinned_drop};

    #[pin_project(PinnedDrop)]
    struct Empty(());

    #[pinned_drop]
    impl PinnedDrop for Empty {} //~ ERROR not all trait items implemented, missing: `drop`

    #[pin_project(PinnedDrop)]
    struct Const1(());

    #[pinned_drop]
    impl PinnedDrop for Const1 {
        const A: u8 = 0; //~ ERROR const `A` is not a member of trait `PinnedDrop`
        fn drop(self: Pin<&mut Self>) {}
    }

    #[pin_project(PinnedDrop)]
    struct Const2(());

    #[pinned_drop]
    impl PinnedDrop for Const2 {
        fn drop(self: Pin<&mut Self>) {}
        const A: u8 = 0; //~ ERROR const `A` is not a member of trait `PinnedDrop`
    }

    #[pin_project(PinnedDrop)]
    struct Type1(());

    #[pinned_drop]
    impl PinnedDrop for Type1 {
        type A = u8; //~ ERROR type `A` is not a member of trait `PinnedDrop`
        fn drop(self: Pin<&mut Self>) {}
    }

    #[pin_project(PinnedDrop)]
    struct Type2(());

    #[pinned_drop]
    impl PinnedDrop for Type2 {
        fn drop(self: Pin<&mut Self>) {}
        type A = u8; //~ ERROR type `A` is not a member of trait `PinnedDrop`
    }

    #[pin_project(PinnedDrop)]
    struct Duplicate(());

    #[pinned_drop]
    impl PinnedDrop for Duplicate {
        fn drop(self: Pin<&mut Self>) {}
        fn drop(self: Pin<&mut Self>) {} //~ ERROR duplicate definitions with name `drop`
    }
}

mod method {
    use std::pin::Pin;

    use pin_project::{pin_project, pinned_drop};

    #[pin_project(PinnedDrop)]
    struct RetUnit(());

    #[pinned_drop]
    impl PinnedDrop for RetUnit {
        fn drop(self: Pin<&mut Self>) -> () {} // Ok
    }

    #[pin_project(PinnedDrop)]
    struct RetTy(());

    #[pinned_drop]
    impl PinnedDrop for RetTy {
        fn drop(self: Pin<&mut Self>) -> Self {} //~ ERROR method `drop` must return the unit type
    }

    #[pin_project(PinnedDrop)]
    struct NoArg(());

    #[pinned_drop]
    impl PinnedDrop for NoArg {
        fn drop() {} //~ ERROR method `drop` must take an argument `self: Pin<&mut Self>`
    }

    #[pin_project(PinnedDrop)]
    struct MultiArg(());

    #[pinned_drop]
    impl PinnedDrop for MultiArg {
        fn drop(self: Pin<&mut Self>, _: ()) {} //~ ERROR method `drop` must take an argument `self: Pin<&mut Self>`
    }

    #[pin_project(PinnedDrop)]
    struct InvalidArg1(());

    #[pinned_drop]
    impl PinnedDrop for InvalidArg1 {
        fn drop(&mut self) {} //~ ERROR method `drop` must take an argument `self: Pin<&mut Self>`
    }

    #[pin_project(PinnedDrop)]
    struct InvalidArg2(());

    #[pinned_drop]
    impl PinnedDrop for InvalidArg2 {
        fn drop(_: Pin<&mut Self>) {} //~ ERROR method `drop` must take an argument `self: Pin<&mut Self>`
    }

    #[pin_project(PinnedDrop)]
    struct InvalidArg3(());

    #[pinned_drop]
    impl PinnedDrop for InvalidArg3 {
        fn drop(self: Pin<&Self>) {} //~ ERROR method `drop` must take an argument `self: Pin<&mut Self>`
    }

    #[pin_project(PinnedDrop)]
    struct InvalidArg4(());

    #[pinned_drop]
    impl PinnedDrop for InvalidArg4 {
        fn drop(self: Pin<&mut ()>) {} //~ ERROR method `drop` must take an argument `self: Pin<&mut Self>`
    }

    #[pin_project(PinnedDrop)]
    struct InvalidName(());

    #[pinned_drop]
    impl PinnedDrop for InvalidName {
        fn pinned_drop(self: Pin<&mut Self>) {} //~ ERROR method `pinned_drop` is not a member of trait `PinnedDrop`
    }
}

mod self_ty {
    use pin_project::pinned_drop;

    #[pinned_drop]
    impl PinnedDrop for () {
        //~^ ERROR implementing the trait `PinnedDrop` on this type is unsupported
        fn drop(self: Pin<&mut Self>) {}
    }

    #[pinned_drop]
    impl PinnedDrop for &mut A {
        //~^ ERROR implementing the trait `PinnedDrop` on this type is unsupported
        fn drop(self: Pin<&mut Self>) {}
    }

    #[pinned_drop]
    impl PinnedDrop for [A] {
        //~^ ERROR implementing the trait `PinnedDrop` on this type is unsupported
        fn drop(self: Pin<&mut Self>) {}
    }
}

fn main() {}
