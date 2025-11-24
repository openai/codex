// SPDX-License-Identifier: Apache-2.0 OR MIT

// Original code (./pinned_drop.rs):
//
// ```
// #![allow(dead_code)]
//
// use std::pin::Pin;
//
// use pin_project::{pin_project, pinned_drop};
//
// #[pin_project(PinnedDrop)]
// pub struct Struct<'a, T> {
//     was_dropped: &'a mut bool,
//     #[pin]
//     field: T,
// }
//
// #[pinned_drop]
// fn drop_Struct<T>(mut this: Pin<&mut Struct<'_, T>>) {
//     **this.project().was_dropped = true;
// }
//
// fn main() {}
// ```

#![allow(
    dead_code,
    single_use_lifetimes,
    unused_imports,
    unused_parens,
    unknown_lints,
    renamed_and_removed_lints,
    unnameable_types,
    clippy::elidable_lifetime_names,
    clippy::mut_mut,
    clippy::needless_lifetimes,
    clippy::undocumented_unsafe_blocks
)]

use std::pin::Pin;

use pin_project::{pin_project, pinned_drop};

// #[pin_project(PinnedDrop)]
pub struct Struct<'a, T> {
    was_dropped: &'a mut bool,
    // #[pin]
    field: T,
}

const _: () = {
    pub(crate) struct __StructProjection<'pin, 'a, T>
    where
        Struct<'a, T>: 'pin,
    {
        was_dropped: &'pin mut (&'a mut bool),
        field: ::pin_project::__private::Pin<&'pin mut (T)>,
    }
    pub(crate) struct __StructProjectionRef<'pin, 'a, T>
    where
        Struct<'a, T>: 'pin,
    {
        was_dropped: &'pin (&'a mut bool),
        field: ::pin_project::__private::Pin<&'pin (T)>,
    }

    impl<'a, T> Struct<'a, T> {
        pub(crate) fn project<'pin>(
            self: ::pin_project::__private::Pin<&'pin mut Self>,
        ) -> __StructProjection<'pin, 'a, T> {
            unsafe {
                let Self { was_dropped, field } = self.get_unchecked_mut();
                __StructProjection {
                    was_dropped,
                    field: ::pin_project::__private::Pin::new_unchecked(field),
                }
            }
        }
        pub(crate) fn project_ref<'pin>(
            self: ::pin_project::__private::Pin<&'pin Self>,
        ) -> __StructProjectionRef<'pin, 'a, T> {
            unsafe {
                let Self { was_dropped, field } = self.get_ref();
                __StructProjectionRef {
                    was_dropped,
                    field: ::pin_project::__private::Pin::new_unchecked(field),
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
    fn __assert_not_repr_packed<'a, T>(this: &Struct<'a, T>) {
        let _ = &this.was_dropped;
        let _ = &this.field;
    }

    impl<'a, T> ::pin_project::__private::Drop for Struct<'a, T> {
        fn drop(&mut self) {
            // Safety - we're in 'drop', so we know that 'self' will
            // never move again.
            let pinned_self = unsafe { ::pin_project::__private::Pin::new_unchecked(self) };
            // We call `pinned_drop` only once. Since `PinnedDrop::drop`
            // is an unsafe method and a private API, it is never called again in safe
            // code *unless the user uses a maliciously crafted macro*.
            unsafe {
                ::pin_project::__private::PinnedDrop::drop(pinned_self);
            }
        }
    }

    // Automatically create the appropriate conditional `Unpin` implementation.
    //
    // See ./struct-default-expanded.rs and https://github.com/taiki-e/pin-project/pull/53.
    // for details.
    pub struct __Struct<'pin, 'a, T> {
        __pin_project_use_generics: ::pin_project::__private::AlwaysUnpin<'pin, (T)>,
        __field0: T,
        __lifetime0: &'a (),
    }
    impl<'pin, 'a, T> ::pin_project::__private::Unpin for Struct<'a, T> where
        ::pin_project::__private::PinnedFieldsOf<__Struct<'pin, 'a, T>>:
            ::pin_project::__private::Unpin
    {
    }
    // A dummy impl of `UnsafeUnpin`, to ensure that the user cannot implement it.
    #[doc(hidden)]
    unsafe impl<'pin, 'a, T> ::pin_project::UnsafeUnpin for Struct<'a, T> where
        ::pin_project::__private::PinnedFieldsOf<__Struct<'pin, 'a, T>>:
            ::pin_project::__private::Unpin
    {
    }
};

// Implementing `PinnedDrop::drop` is safe, but calling it is not safe.
// This is because destructors can be called multiple times in safe code and
// [double dropping is unsound](https://github.com/rust-lang/rust/pull/62360).
//
// Ideally, it would be desirable to be able to forbid manual calls in
// the same way as `Drop::drop`, but the library cannot do it. So, by using
// macros and replacing them with private traits, we prevent users from
// calling `PinnedDrop::drop`.
//
// Users can implement [`Drop`] safely using `#[pinned_drop]` and can drop a
// type that implements `PinnedDrop` using the [`drop`] function safely.
// **Do not call or implement this trait directly.**
#[doc(hidden)]
impl<T> ::pin_project::__private::PinnedDrop for Struct<'_, T> {
    // Since calling it twice on the same object would be UB,
    // this method is unsafe.
    unsafe fn drop(self: Pin<&mut Self>) {
        #[allow(clippy::needless_pass_by_value)]
        fn __drop_inner<T>(__self: Pin<&mut Struct<'_, T>>) {
            // A dummy `__drop_inner` function to prevent users call outer `__drop_inner`.
            fn __drop_inner() {}

            **__self.project().was_dropped = true;
        }
        __drop_inner(self);
    }
}

fn main() {}
