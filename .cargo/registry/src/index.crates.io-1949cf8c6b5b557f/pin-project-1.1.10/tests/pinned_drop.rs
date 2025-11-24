// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::pin::Pin;

use pin_project::{pin_project, pinned_drop};

#[test]
fn safe_project() {
    #[pin_project(PinnedDrop)]
    struct Struct<'a> {
        was_dropped: &'a mut bool,
        #[pin]
        field: u8,
    }

    #[pinned_drop]
    impl PinnedDrop for Struct<'_> {
        fn drop(self: Pin<&mut Self>) {
            **self.project().was_dropped = true;
        }
    }

    let mut was_dropped = false;
    drop(Struct { was_dropped: &mut was_dropped, field: 42 });
    assert!(was_dropped);
}

#[test]
fn self_call() {
    #[pin_project(PinnedDrop)]
    struct S<T>(T);

    trait Trait {
        fn self_ref(&self) {}
        fn self_pin_ref(self: Pin<&Self>) {}
        fn self_mut(&mut self) {}
        fn self_pin_mut(self: Pin<&mut Self>) {}
        fn assoc_fn(_this: Pin<&mut Self>) {}
    }

    impl<T> Trait for S<T> {}

    #[pinned_drop]
    impl<T> PinnedDrop for S<T> {
        fn drop(mut self: Pin<&mut Self>) {
            self.self_ref();
            self.as_ref().self_pin_ref();
            self.self_mut();
            self.as_mut().self_pin_mut();
            Self::assoc_fn(self.as_mut());
            <Self>::assoc_fn(self.as_mut());
        }
    }
}

#[test]
fn self_ty() {
    #[pin_project(PinnedDrop)]
    struct Struct {
        f: (),
    }

    #[pinned_drop]
    impl PinnedDrop for Struct {
        #[allow(irrefutable_let_patterns)]
        #[allow(clippy::match_single_binding)]
        fn drop(mut self: Pin<&mut Self>) {
            // expr
            let _: Self = Self { f: () };

            // pat
            match *self {
                Self { f: () } => {}
            }
            if let Self { f: () } = *self {}
            let Self { f: () } = *self;
        }
    }

    #[pin_project(PinnedDrop)]
    struct TupleStruct(());

    #[pinned_drop]
    impl PinnedDrop for TupleStruct {
        #[allow(irrefutable_let_patterns)]
        fn drop(mut self: Pin<&mut Self>) {
            // expr
            let _: Self = Self(());

            // pat
            match *self {
                Self(()) => {}
            }
            if let Self(()) = *self {}
            let Self(()) = *self;
        }
    }

    #[pin_project(PinnedDrop, project = EnumProj, project_ref = EnumProjRef)]
    enum Enum {
        Struct { f: () },
        Tuple(()),
        Unit,
    }

    #[pinned_drop]
    impl PinnedDrop for Enum {
        fn drop(mut self: Pin<&mut Self>) {
            // expr
            let _: Self = Self::Struct { f: () };
            let _: Self = Self::Tuple(());
            let _: Self = Self::Unit;

            // pat
            match *self {
                Self::Struct { f: () } | Self::Tuple(()) | Self::Unit => {}
            }
            if let Self::Struct { f: () } = *self {}
            if let Self::Tuple(()) = *self {}
            if let Self::Unit = *self {}
        }
    }
}

#[test]
fn self_inside_macro_containing_fn() {
    macro_rules! mac {
        ($($tt:tt)*) => {
            $($tt)*
        };
    }

    #[pin_project(PinnedDrop)]
    struct S(());

    #[pinned_drop]
    impl PinnedDrop for S {
        fn drop(self: Pin<&mut Self>) {
            mac!({
                #[allow(dead_code)]
                struct S(());
                impl S {
                    fn _f(self) -> Self {
                        self
                    }
                }
            });
        }
    }
}

// See also `ui/pinned_drop/self.rs`.
#[test]
fn self_inside_macro_def() {
    #[pin_project(PinnedDrop)]
    struct S(());

    #[pinned_drop]
    impl PinnedDrop for S {
        fn drop(self: Pin<&mut Self>) {
            macro_rules! mac {
                () => {{
                    let _ = self;
                    let _ = Self(());
                }};
            }
            mac!();
        }
    }
}

#[test]
fn self_arg_inside_macro_call() {
    #[pin_project(PinnedDrop)]
    struct Struct {
        f: (),
    }

    #[pinned_drop]
    impl PinnedDrop for Struct {
        fn drop(self: Pin<&mut Self>) {
            let _: Vec<_> = vec![self.f];
        }
    }
}

#[test]
fn self_ty_inside_macro_call() {
    macro_rules! mac {
        ($($tt:tt)*) => {
            $($tt)*
        };
    }

    #[pin_project(PinnedDrop)]
    struct Struct<T: Send>
    where
        mac!(Self): Send,
    {
        _f: T,
    }

    impl<T: Send> Struct<T> {
        const ASSOC1: usize = 1;
        fn assoc1() {}
    }

    trait Trait {
        type Assoc2;
        const ASSOC2: usize;
        fn assoc2();
    }

    impl<T: Send> Trait for Struct<T> {
        type Assoc2 = u8;
        const ASSOC2: usize = 2;
        fn assoc2() {}
    }

    #[pinned_drop]
    impl<T: Send> PinnedDrop for Struct<T>
    where
        mac!(Self): Send,
    {
        #[allow(path_statements)]
        #[allow(clippy::no_effect)]
        fn drop(self: Pin<&mut Self>) {
            // inherent items
            mac!(Self::ASSOC1;);
            mac!(<Self>::ASSOC1;);
            mac!(Self::assoc1(););
            mac!(<Self>::assoc1(););

            // trait items
            mac!(let _: <Self as Trait>::Assoc2;);
            mac!(Self::ASSOC2;);
            mac!(<Self>::ASSOC2;);
            mac!(<Self as Trait>::ASSOC2;);
            mac!(Self::assoc2(););
            mac!(<Self>::assoc2(););
            mac!(<Self as Trait>::assoc2(););
        }
    }
}

#[test]
fn inside_macro() {
    #[pin_project(PinnedDrop)]
    struct S(());

    macro_rules! mac {
        ($expr:expr) => {
            #[pinned_drop]
            impl PinnedDrop for S {
                fn drop(self: Pin<&mut Self>) {
                    let _ = $expr;
                }
            }
        };
    }

    mac!(1);
}

mod self_path {
    use super::*;

    #[pin_project(PinnedDrop)]
    struct S<T: Unpin>(T);

    fn f() {}

    #[pinned_drop]
    impl<T: Unpin> PinnedDrop for self::S<T> {
        fn drop(mut self: Pin<&mut Self>) {
            self::f();
            let _: self::S<()> = self::S(());
            let _: self::S<Pin<&mut Self>> = self::S(self.as_mut());
            let self::S(()) = self::S(());
            let self::S(&mut Self(_)) = self::S(&mut *self);
        }
    }
}
