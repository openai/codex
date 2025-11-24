// SPDX-License-Identifier: Apache-2.0 OR MIT

#![allow(
    dead_code,
    unreachable_pub,
    clippy::items_after_statements,
    clippy::no_effect_underscore_binding,
    clippy::undocumented_unsafe_blocks
)]

#[macro_use]
mod auxiliary;

use std::{
    marker::{PhantomData, PhantomPinned},
    panic,
    pin::Pin,
};

use pin_project::{UnsafeUnpin, pin_project, pinned_drop};

#[test]
fn projection() {
    #[pin_project(
        project = StructProj,
        project_ref = StructProjRef,
        project_replace = StructProjOwn,
    )]
    struct Struct<T, U> {
        #[pin]
        f1: T,
        f2: U,
    }

    let mut s = Struct { f1: 1, f2: 2 };
    let mut s_orig = Pin::new(&mut s);
    let s = s_orig.as_mut().project();

    let _: Pin<&mut i32> = s.f1;
    assert_eq!(*s.f1, 1);
    let _: &mut i32 = s.f2;
    assert_eq!(*s.f2, 2);

    assert_eq!(s_orig.as_ref().f1, 1);
    assert_eq!(s_orig.as_ref().f2, 2);

    let mut s = Struct { f1: 1, f2: 2 };
    let mut s = Pin::new(&mut s);
    {
        let StructProj { f1, f2 } = s.as_mut().project();
        let _: Pin<&mut i32> = f1;
        let _: &mut i32 = f2;
    }
    {
        let StructProjRef { f1, f2 } = s.as_ref().project_ref();
        let _: Pin<&i32> = f1;
        let _: &i32 = f2;
    }
    {
        let StructProjOwn { f1, f2 } = s.as_mut().project_replace(Struct { f1: 3, f2: 4 });
        let _: PhantomData<i32> = f1;
        let _: i32 = f2;
        assert_eq!(f2, 2);
        assert_eq!(s.f1, 3);
        assert_eq!(s.f2, 4);
    }

    #[pin_project(project_replace)]
    struct TupleStruct<T, U>(#[pin] T, U);

    let mut s = TupleStruct(1, 2);
    let s = Pin::new(&mut s).project();

    let _: Pin<&mut i32> = s.0;
    assert_eq!(*s.0, 1);
    let _: &mut i32 = s.1;
    assert_eq!(*s.1, 2);

    #[pin_project(project = EnumProj, project_ref = EnumProjRef, project_replace = EnumProjOwn)]
    #[derive(Eq, PartialEq, Debug)]
    enum Enum<A, B, C, D> {
        Tuple(#[pin] A, B),
        Struct {
            #[pin]
            f1: C,
            f2: D,
        },
        Unit,
    }

    let mut e = Enum::Tuple(1, 2);
    let mut e = Pin::new(&mut e);

    match e.as_mut().project() {
        EnumProj::Tuple(x, y) => {
            let x: Pin<&mut i32> = x;
            assert_eq!(*x, 1);
            let y: &mut i32 = y;
            assert_eq!(*y, 2);
        }
        EnumProj::Struct { f1, f2 } => {
            let _: Pin<&mut i32> = f1;
            let _: &mut i32 = f2;
            unreachable!();
        }
        EnumProj::Unit => unreachable!(),
    }

    assert_eq!(&*e, &Enum::Tuple(1, 2));

    let mut e = Enum::Struct { f1: 3, f2: 4 };
    let mut e = Pin::new(&mut e);

    match e.as_mut().project() {
        EnumProj::Tuple(x, y) => {
            let _: Pin<&mut i32> = x;
            let _: &mut i32 = y;
            unreachable!();
        }
        EnumProj::Struct { f1, f2 } => {
            let _: Pin<&mut i32> = f1;
            assert_eq!(*f1, 3);
            let _: &mut i32 = f2;
            assert_eq!(*f2, 4);
        }
        EnumProj::Unit => unreachable!(),
    }

    if let EnumProj::Struct { f1, f2 } = e.as_mut().project() {
        let _: Pin<&mut i32> = f1;
        assert_eq!(*f1, 3);
        let _: &mut i32 = f2;
        assert_eq!(*f2, 4);
    }
}

#[test]
fn enum_project_set() {
    #[pin_project(project = EnumProj, project_ref = EnumProjRef, project_replace = EnumProjOwn)]
    #[derive(Eq, PartialEq, Debug)]
    enum Enum {
        V1(#[pin] u8),
        V2(bool),
    }

    let mut e = Enum::V1(25);
    let mut e_orig = Pin::new(&mut e);
    let e_proj = e_orig.as_mut().project();

    match e_proj {
        EnumProj::V1(val) => {
            let new_e = Enum::V2(val.as_ref().get_ref() == &25);
            e_orig.set(new_e);
        }
        EnumProj::V2(_) => unreachable!(),
    }

    assert_eq!(e, Enum::V2(true));
}

#[test]
fn where_clause() {
    #[pin_project]
    struct Struct<T>
    where
        T: Copy,
    {
        f: T,
    }

    #[pin_project]
    struct TupleStruct<T>(T)
    where
        T: Copy;

    #[pin_project(project = EnumProj, project_ref = EnumProjRef, project_replace = EnumProjOwn)]
    enum Enum<T>
    where
        T: Copy,
    {
        V(T),
    }
}

#[test]
fn where_clause_and_associated_type_field() {
    #[pin_project(project_replace)]
    struct Struct1<I>
    where
        I: Iterator,
    {
        #[pin]
        f1: I,
        f2: I::Item,
    }

    #[pin_project(project_replace)]
    struct Struct2<I, J>
    where
        I: Iterator<Item = J>,
    {
        #[pin]
        f1: I,
        f2: J,
    }

    #[pin_project(project_replace)]
    struct Struct3<T>
    where
        T: 'static,
    {
        f: T,
    }

    trait Static: 'static {}

    impl<T> Static for Struct3<T> {}

    #[pin_project(project_replace)]
    struct TupleStruct<I>(#[pin] I, I::Item)
    where
        I: Iterator;

    #[pin_project(project = EnumProj, project_ref = EnumProjRef, project_replace = EnumProjOwn)]
    enum Enum<I>
    where
        I: Iterator,
    {
        V1(#[pin] I),
        V2(I::Item),
    }
}

#[test]
fn derive_copy() {
    #[pin_project(project_replace)]
    #[derive(Clone, Copy)]
    struct Struct<T> {
        f: T,
    }

    fn is_copy<T: Copy>() {}

    is_copy::<Struct<u8>>();
}

#[test]
fn move_out() {
    struct NotCopy;

    #[pin_project(project_replace)]
    struct Struct {
        f: NotCopy,
    }

    let x = Struct { f: NotCopy };
    let _val: NotCopy = x.f;

    #[pin_project(project = EnumProj, project_ref = EnumProjRef, project_replace = EnumProjOwn)]
    enum Enum {
        V(NotCopy),
    }

    let x = Enum::V(NotCopy);
    #[allow(clippy::infallible_destructuring_match)]
    let _val: NotCopy = match x {
        Enum::V(val) => val,
    };
}

#[test]
fn trait_bounds_on_type_generics() {
    #[pin_project(project_replace)]
    pub struct Struct1<'a, T: ?Sized> {
        f: &'a mut T,
    }

    #[pin_project(project_replace)]
    pub struct Struct2<'a, T: ::core::fmt::Debug> {
        f: &'a mut T,
    }

    #[pin_project(project_replace)]
    pub struct Struct3<'a, T: core::fmt::Debug> {
        f: &'a mut T,
    }

    #[pin_project(project_replace)]
    pub struct Struct4<'a, T: core::fmt::Debug + core::fmt::Display> {
        f: &'a mut T,
    }

    #[pin_project(project_replace)]
    pub struct Struct5<'a, T: core::fmt::Debug + ?Sized> {
        f: &'a mut T,
    }

    #[pin_project(project_replace)]
    pub struct Struct6<'a, T: core::fmt::Debug = [u8; 16]> {
        f: &'a mut T,
    }

    let _: Struct6<'_> = Struct6 { f: &mut [0_u8; 16] };

    #[pin_project(project_replace)]
    pub struct Struct7<T: 'static> {
        f: T,
    }

    trait Static: 'static {}

    impl<T> Static for Struct7<T> {}

    #[pin_project(project_replace)]
    pub struct Struct8<'a, 'b: 'a> {
        f1: &'a u8,
        f2: &'b u8,
    }

    #[pin_project(project_replace)]
    pub struct TupleStruct<'a, T: ?Sized>(&'a mut T);

    #[pin_project(project = EnumProj, project_ref = EnumProjRef, project_replace = EnumProjOwn)]
    enum Enum<'a, T: ?Sized> {
        V(&'a mut T),
    }
}

#[test]
fn overlapping_lifetime_names() {
    #[pin_project(project_replace)]
    pub struct Struct1<'pin, T> {
        #[pin]
        f: &'pin mut T,
    }

    #[pin_project(project_replace)]
    pub struct Struct2<'pin, 'pin_, 'pin__> {
        #[pin]
        f: &'pin &'pin_ &'pin__ (),
    }

    pub trait Trait<'a> {}

    #[pin_project(project_replace)]
    pub struct Hrtb<'pin___, T>
    where
        for<'pin> &'pin T: Unpin,
        T: for<'pin> Trait<'pin>,
        for<'pin, 'pin_, 'pin__> &'pin &'pin_ &'pin__ T: Unpin,
    {
        #[pin]
        f: &'pin___ mut T,
    }

    #[pin_project(PinnedDrop)]
    pub struct PinnedDropStruct<'pin> {
        #[pin]
        f: &'pin (),
    }

    #[pinned_drop]
    impl PinnedDrop for PinnedDropStruct<'_> {
        fn drop(self: Pin<&mut Self>) {}
    }

    #[pin_project(UnsafeUnpin)]
    pub struct UnsafeUnpinStruct<'pin> {
        #[pin]
        f: &'pin (),
    }

    unsafe impl UnsafeUnpin for UnsafeUnpinStruct<'_> {}

    #[pin_project(!Unpin)]
    pub struct NotUnpinStruct<'pin> {
        #[pin]
        f: &'pin (),
    }
}

#[test]
fn combine() {
    #[pin_project(PinnedDrop, UnsafeUnpin)]
    pub struct PinnedDropWithUnsafeUnpin<T> {
        #[pin]
        f: T,
    }

    #[pinned_drop]
    impl<T> PinnedDrop for PinnedDropWithUnsafeUnpin<T> {
        fn drop(self: Pin<&mut Self>) {}
    }

    unsafe impl<T: Unpin> UnsafeUnpin for PinnedDropWithUnsafeUnpin<T> {}

    #[pin_project(PinnedDrop, !Unpin)]
    pub struct PinnedDropWithNotUnpin<T> {
        #[pin]
        f: T,
    }

    #[pinned_drop]
    impl<T> PinnedDrop for PinnedDropWithNotUnpin<T> {
        fn drop(self: Pin<&mut Self>) {}
    }

    #[pin_project(UnsafeUnpin, project_replace)]
    pub struct UnsafeUnpinWithReplace<T> {
        #[pin]
        f: T,
    }

    unsafe impl<T: Unpin> UnsafeUnpin for UnsafeUnpinWithReplace<T> {}

    #[pin_project(!Unpin, project_replace)]
    pub struct NotUnpinWithReplace<T> {
        #[pin]
        f: T,
    }
}

#[test]
fn private_type_in_public_type() {
    #[pin_project(project_replace)]
    pub struct PublicStruct<T> {
        #[pin]
        inner: PrivateStruct<T>,
    }

    struct PrivateStruct<T>(T);
}

#[allow(clippy::needless_lifetimes)]
#[test]
fn lifetime_project() {
    #[pin_project(project_replace)]
    struct Struct1<T, U> {
        #[pin]
        pinned: T,
        unpinned: U,
    }

    #[pin_project(project_replace)]
    struct Struct2<'a, T, U> {
        #[pin]
        pinned: &'a T,
        unpinned: U,
    }

    #[pin_project(project = EnumProj, project_ref = EnumProjRef, project_replace = EnumProjOwn)]
    enum Enum<T, U> {
        V {
            #[pin]
            pinned: T,
            unpinned: U,
        },
    }

    impl<T, U> Struct1<T, U> {
        fn get_pin_ref<'a>(self: Pin<&'a Self>) -> Pin<&'a T> {
            self.project_ref().pinned
        }
        fn get_pin_mut<'a>(self: Pin<&'a mut Self>) -> Pin<&'a mut T> {
            self.project().pinned
        }
        fn get_pin_ref_elided(self: Pin<&Self>) -> Pin<&T> {
            self.project_ref().pinned
        }
        fn get_pin_mut_elided(self: Pin<&mut Self>) -> Pin<&mut T> {
            self.project().pinned
        }
    }

    impl<'b, T, U> Struct2<'b, T, U> {
        fn get_pin_ref<'a>(self: Pin<&'a Self>) -> Pin<&'a &'b T> {
            self.project_ref().pinned
        }
        fn get_pin_mut<'a>(self: Pin<&'a mut Self>) -> Pin<&'a mut &'b T> {
            self.project().pinned
        }
        fn get_pin_ref_elided(self: Pin<&Self>) -> Pin<&&'b T> {
            self.project_ref().pinned
        }
        fn get_pin_mut_elided(self: Pin<&mut Self>) -> Pin<&mut &'b T> {
            self.project().pinned
        }
    }

    impl<T, U> Enum<T, U> {
        fn get_pin_ref<'a>(self: Pin<&'a Self>) -> Pin<&'a T> {
            match self.project_ref() {
                EnumProjRef::V { pinned, .. } => pinned,
            }
        }
        fn get_pin_mut<'a>(self: Pin<&'a mut Self>) -> Pin<&'a mut T> {
            match self.project() {
                EnumProj::V { pinned, .. } => pinned,
            }
        }
        fn get_pin_ref_elided(self: Pin<&Self>) -> Pin<&T> {
            match self.project_ref() {
                EnumProjRef::V { pinned, .. } => pinned,
            }
        }
        fn get_pin_mut_elided(self: Pin<&mut Self>) -> Pin<&mut T> {
            match self.project() {
                EnumProj::V { pinned, .. } => pinned,
            }
        }
    }
}

mod visibility {
    use pin_project::pin_project;

    #[pin_project(project_replace)]
    pub(crate) struct S {
        pub f: u8,
    }
}

#[test]
fn visibility() {
    let mut x = visibility::S { f: 0 };
    let x = Pin::new(&mut x);
    let y = x.as_ref().project_ref();
    let _: &u8 = y.f;
    let y = x.project();
    let _: &mut u8 = y.f;
}

#[test]
fn trivial_bounds() {
    #[pin_project(project_replace)]
    pub struct NoGenerics {
        #[pin]
        f: PhantomPinned,
    }

    assert_not_unpin!(NoGenerics);
}

#[test]
fn dst() {
    #[pin_project]
    struct Struct1<T: ?Sized> {
        f: T,
    }

    let mut x = Struct1 { f: 0_u8 };
    let x: Pin<&mut Struct1<dyn core::fmt::Debug>> = Pin::new(&mut x);
    let _: &mut (dyn core::fmt::Debug) = x.project().f;

    #[pin_project]
    struct Struct2<T: ?Sized> {
        #[pin]
        f: T,
    }

    let mut x = Struct2 { f: 0_u8 };
    let x: Pin<&mut Struct2<dyn core::fmt::Debug + Unpin>> = Pin::new(&mut x);
    let _: Pin<&mut (dyn core::fmt::Debug + Unpin)> = x.project().f;

    #[allow(explicit_outlives_requirements)] // https://github.com/rust-lang/rust/issues/60993
    #[pin_project]
    struct Struct3<T>
    where
        T: ?Sized,
    {
        f: T,
    }

    #[allow(explicit_outlives_requirements)] // https://github.com/rust-lang/rust/issues/60993
    #[pin_project]
    struct Struct4<T>
    where
        T: ?Sized,
    {
        #[pin]
        f: T,
    }

    #[pin_project(UnsafeUnpin)]
    struct Struct5<T: ?Sized> {
        f: T,
    }

    #[pin_project(UnsafeUnpin)]
    struct Struct6<T: ?Sized> {
        #[pin]
        f: T,
    }

    #[pin_project(PinnedDrop)]
    struct Struct7<T: ?Sized> {
        f: T,
    }

    #[pinned_drop]
    impl<T: ?Sized> PinnedDrop for Struct7<T> {
        fn drop(self: Pin<&mut Self>) {}
    }

    #[pin_project(PinnedDrop)]
    struct Struct8<T: ?Sized> {
        #[pin]
        f: T,
    }

    #[pinned_drop]
    impl<T: ?Sized> PinnedDrop for Struct8<T> {
        fn drop(self: Pin<&mut Self>) {}
    }

    #[pin_project(!Unpin)]
    struct Struct9<T: ?Sized> {
        f: T,
    }

    #[pin_project(!Unpin)]
    struct Struct10<T: ?Sized> {
        #[pin]
        f: T,
    }

    #[pin_project]
    struct Struct11<'a, T: ?Sized, U: ?Sized> {
        f1: &'a mut T,
        f2: U,
    }

    #[pin_project]
    struct TupleStruct1<T: ?Sized>(T);

    #[pin_project]
    struct TupleStruct2<T: ?Sized>(#[pin] T);

    #[allow(explicit_outlives_requirements)] // https://github.com/rust-lang/rust/issues/60993
    #[pin_project]
    struct TupleStruct3<T>(T)
    where
        T: ?Sized;

    #[allow(explicit_outlives_requirements)] // https://github.com/rust-lang/rust/issues/60993
    #[pin_project]
    struct TupleStruct4<T>(#[pin] T)
    where
        T: ?Sized;

    #[pin_project(UnsafeUnpin)]
    struct TupleStruct5<T: ?Sized>(T);

    #[pin_project(UnsafeUnpin)]
    struct TupleStruct6<T: ?Sized>(#[pin] T);

    #[pin_project(PinnedDrop)]
    struct TupleStruct7<T: ?Sized>(T);

    #[pinned_drop]
    impl<T: ?Sized> PinnedDrop for TupleStruct7<T> {
        fn drop(self: Pin<&mut Self>) {}
    }

    #[pin_project(PinnedDrop)]
    struct TupleStruct8<T: ?Sized>(#[pin] T);

    #[pinned_drop]
    impl<T: ?Sized> PinnedDrop for TupleStruct8<T> {
        fn drop(self: Pin<&mut Self>) {}
    }

    #[pin_project(!Unpin)]
    struct TupleStruct9<T: ?Sized>(T);

    #[pin_project(!Unpin)]
    struct TupleStruct10<T: ?Sized>(#[pin] T);

    #[pin_project]
    struct TupleStruct11<'a, T: ?Sized, U: ?Sized>(&'a mut T, U);
}

#[test]
fn dyn_type() {
    #[pin_project]
    struct Struct1 {
        f: dyn core::fmt::Debug,
    }

    #[pin_project]
    struct Struct2 {
        #[pin]
        f: dyn core::fmt::Debug,
    }

    #[pin_project]
    struct Struct3 {
        f: dyn core::fmt::Debug + Send,
    }

    #[pin_project]
    struct Struct4 {
        #[pin]
        f: dyn core::fmt::Debug + Send,
    }

    #[pin_project]
    struct TupleStruct1(dyn core::fmt::Debug);

    #[pin_project]
    struct TupleStruct2(#[pin] dyn core::fmt::Debug);

    #[pin_project]
    struct TupleStruct3(dyn core::fmt::Debug + Send);

    #[pin_project]
    struct TupleStruct4(#[pin] dyn core::fmt::Debug + Send);
}

// TODO: how do we handle this? Should propagate #[repr(...)] to ProjectionOwned? The layout will be different from the original anyway due to PhantomData.
#[allow(clippy::trailing_empty_array)]
#[test]
fn parse_self() {
    macro_rules! mac {
        ($($tt:tt)*) => {
            $($tt)*
        };
    }

    pub trait Trait {
        type Assoc;
    }

    #[allow(clippy::type_repetition_in_bounds)]
    #[pin_project(project_replace)]
    pub struct Generics<T: Trait<Assoc = Self>>
    where
        Self: Trait<Assoc = Self>,
        <Self as Trait>::Assoc: Sized,
        mac!(Self): Trait<Assoc = mac!(Self)>,
    {
        _f: T,
    }

    impl<T: Trait<Assoc = Self>> Trait for Generics<T> {
        type Assoc = Self;
    }

    #[pin_project(project_replace)]
    pub struct Struct {
        _f1: Box<Self>,
        _f2: Box<<Self as Trait>::Assoc>,
        _f3: Box<mac!(Self)>,
        _f4: [(); Self::ASSOC],
        _f5: [(); Self::assoc()],
        _f6: [(); mac!(Self::assoc())],
    }

    impl Struct {
        const ASSOC: usize = 1;
        const fn assoc() -> usize {
            0
        }
    }

    impl Trait for Struct {
        type Assoc = Self;
    }

    #[pin_project(project_replace)]
    struct Tuple(
        Box<Self>,
        Box<<Self as Trait>::Assoc>,
        Box<mac!(Self)>,
        [(); Self::ASSOC],
        [(); Self::assoc()],
        [(); mac!(Self::assoc())],
    );

    impl Tuple {
        const ASSOC: usize = 1;
        const fn assoc() -> usize {
            0
        }
    }

    impl Trait for Tuple {
        type Assoc = Self;
    }

    #[pin_project(project = EnumProj, project_ref = EnumProjRef, project_replace = EnumProjOwn)]
    enum Enum {
        Struct {
            _f1: Box<Self>,
            _f2: Box<<Self as Trait>::Assoc>,
            _f3: Box<mac!(Self)>,
            _f4: [(); Self::ASSOC],
            _f5: [(); Self::assoc()],
            _f6: [(); mac!(Self::assoc())],
        },
        Tuple(
            Box<Self>,
            Box<<Self as Trait>::Assoc>,
            Box<mac!(Self)>,
            [(); Self::ASSOC],
            [(); Self::assoc()],
            [(); mac!(Self::assoc())],
        ),
    }

    impl Enum {
        const ASSOC: usize = 1;
        const fn assoc() -> usize {
            0
        }
    }

    impl Trait for Enum {
        type Assoc = Self;
    }
}

#[test]
fn no_infer_outlives() {
    trait Trait<X> {
        type Y;
    }

    struct Struct1<A>(A);

    impl<X, T> Trait<X> for Struct1<T> {
        type Y = Option<T>;
    }

    #[pin_project(project_replace)]
    struct Struct2<A, B> {
        _f: <Struct1<A> as Trait<B>>::Y,
    }
}

// https://github.com/rust-lang/rust/issues/47949
// https://github.com/taiki-e/pin-project/pull/194#discussion_r419098111
#[allow(clippy::many_single_char_names)]
#[test]
fn project_replace_panic() {
    #[pin_project(project_replace)]
    struct S<T, U> {
        #[pin]
        pinned: T,
        unpinned: U,
    }

    struct D<'a>(&'a mut bool, bool);
    impl Drop for D<'_> {
        fn drop(&mut self) {
            *self.0 = true;
            if self.1 {
                panic!();
            }
        }
    }

    let (mut a, mut b, mut c, mut d) = (false, false, false, false);
    let res = panic::catch_unwind(panic::AssertUnwindSafe(|| {
        let mut x = S { pinned: D(&mut a, true), unpinned: D(&mut b, false) };
        let _y = Pin::new(&mut x)
            .project_replace(S { pinned: D(&mut c, false), unpinned: D(&mut d, false) });
        // Previous `x.pinned` was dropped and panicked when `project_replace` is
        // called, so this is unreachable.
        unreachable!();
    }));
    assert!(res.is_err());
    assert!(a);
    assert!(b);
    assert!(c);
    assert!(d);

    let (mut a, mut b, mut c, mut d) = (false, false, false, false);
    let res = panic::catch_unwind(panic::AssertUnwindSafe(|| {
        let mut x = S { pinned: D(&mut a, false), unpinned: D(&mut b, true) };
        {
            let _y = Pin::new(&mut x)
                .project_replace(S { pinned: D(&mut c, false), unpinned: D(&mut d, false) });
            // `_y` (previous `x.unpinned`) live to the end of this scope, so
            // this is not unreachable.
            // unreachable!();
        }
        unreachable!();
    }));
    assert!(res.is_err());
    assert!(a);
    assert!(b);
    assert!(c);
    assert!(d);
}
