// SPDX-License-Identifier: Apache-2.0 OR MIT

// Original code (./not_unpin.rs):
//
// ```
// #![allow(dead_code)]
//
// use pin_project::pin_project;
//
// #[pin_project(!Unpin)]
// struct Struct<T, U> {
//     #[pin]
//     pinned: T,
//     unpinned: U,
// }
//
// fn main() {
//     fn _is_unpin<T: Unpin>() {}
//     // _is_unpin::<Struct<(), ()>>(); //~ ERROR `std::marker::PhantomPinned` cannot be unpinned
// }
// ```

#![allow(
    dead_code,
    unused_imports,
    unused_parens,
    unknown_lints,
    renamed_and_removed_lints,
    clippy::needless_lifetimes,
    clippy::undocumented_unsafe_blocks
)]

use pin_project::pin_project;

// #[pin_project(!Unpin)]
struct Struct<T, U> {
    // #[pin]
    pinned: T,
    unpinned: U,
}

const _: () = {
    struct __StructProjection<'pin, T, U>
    where
        Struct<T, U>: 'pin,
    {
        pinned: ::pin_project::__private::Pin<&'pin mut (T)>,
        unpinned: &'pin mut (U),
    }
    struct __StructProjectionRef<'pin, T, U>
    where
        Struct<T, U>: 'pin,
    {
        pinned: ::pin_project::__private::Pin<&'pin (T)>,
        unpinned: &'pin (U),
    }

    impl<T, U> Struct<T, U> {
        fn project<'pin>(
            self: ::pin_project::__private::Pin<&'pin mut Self>,
        ) -> __StructProjection<'pin, T, U> {
            unsafe {
                let Self { pinned, unpinned } = self.get_unchecked_mut();
                __StructProjection {
                    pinned: ::pin_project::__private::Pin::new_unchecked(pinned),
                    unpinned,
                }
            }
        }
        fn project_ref<'pin>(
            self: ::pin_project::__private::Pin<&'pin Self>,
        ) -> __StructProjectionRef<'pin, T, U> {
            unsafe {
                let Self { pinned, unpinned } = self.get_ref();
                __StructProjectionRef {
                    pinned: ::pin_project::__private::Pin::new_unchecked(pinned),
                    unpinned,
                }
            }
        }
    }

    // Ensure that it's impossible to use pin projections on a #[repr(packed)]
    // struct.
    //
    // See ./struct-default-expanded.rs and https://github.com/taiki-e/pin-project/pull/34
    // for details.
    #[forbid(unaligned_references, safe_packed_borrows)]
    fn __assert_not_repr_packed<T, U>(this: &Struct<T, U>) {
        let _ = &this.pinned;
        let _ = &this.unpinned;
    }

    // Create `Unpin` impl that has trivial `Unpin` bounds.
    //
    // See https://github.com/taiki-e/pin-project/issues/102#issuecomment-540472282
    // for details.
    #[doc(hidden)]
    impl<'pin, T, U> ::pin_project::__private::Unpin for Struct<T, U> where
        ::pin_project::__private::PinnedFieldsOf<
            ::pin_project::__private::Wrapper<'pin, ::pin_project::__private::PhantomPinned>,
        >: ::pin_project::__private::Unpin
    {
    }
    // A dummy impl of `UnsafeUnpin`, to ensure that the user cannot implement it.
    //
    // To ensure that users don't accidentally write a non-functional `UnsafeUnpin`
    // impls, we emit one ourselves. If the user ends up writing an `UnsafeUnpin`
    // impl, they'll get a "conflicting implementations of trait" error when
    // coherence checks are run.
    #[doc(hidden)]
    unsafe impl<'pin, T, U> ::pin_project::UnsafeUnpin for Struct<T, U> where
        ::pin_project::__private::PinnedFieldsOf<
            ::pin_project::__private::Wrapper<'pin, ::pin_project::__private::PhantomPinned>,
        >: ::pin_project::__private::Unpin
    {
    }

    // Ensure that struct does not implement `Drop`.
    //
    // See ./struct-default-expanded.rs for details.
    trait StructMustNotImplDrop {}
    #[allow(clippy::drop_bounds, drop_bounds)]
    impl<T: ::pin_project::__private::Drop> StructMustNotImplDrop for T {}
    impl<T, U> StructMustNotImplDrop for Struct<T, U> {}
    // A dummy impl of `PinnedDrop`, to ensure that users don't accidentally
    // write a non-functional `PinnedDrop` impls.
    #[doc(hidden)]
    impl<T, U> ::pin_project::__private::PinnedDrop for Struct<T, U> {
        unsafe fn drop(self: ::pin_project::__private::Pin<&mut Self>) {}
    }
};

fn main() {
    fn _is_unpin<T: Unpin>() {}
    // _is_unpin::<Struct<(), ()>>(); //~ ERROR `std::marker::PhantomPinned` cannot be unpinned
}
