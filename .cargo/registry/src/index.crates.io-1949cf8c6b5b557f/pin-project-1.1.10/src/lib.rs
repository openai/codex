// SPDX-License-Identifier: Apache-2.0 OR MIT

/*!
<!-- Note: Document from sync-markdown-to-rustdoc:start through sync-markdown-to-rustdoc:end
     is synchronized from README.md. Any changes to that range are not preserved. -->
<!-- tidy:sync-markdown-to-rustdoc:start -->

A crate for safe and ergonomic [pin-projection].

## Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
pin-project = "1"
```

## Examples

[`#[pin_project]`][`pin_project`] attribute creates projection types
covering all the fields of struct or enum.

```
use std::pin::Pin;

use pin_project::pin_project;

#[pin_project]
struct Struct<T, U> {
    #[pin]
    pinned: T,
    unpinned: U,
}

impl<T, U> Struct<T, U> {
    fn method(self: Pin<&mut Self>) {
        let this = self.project();
        let _: Pin<&mut T> = this.pinned; // Pinned reference to the field
        let _: &mut U = this.unpinned; // Normal reference to the field
    }
}
```

[*code like this will be generated*][struct-default-expanded]

To use `#[pin_project]` on enums, you need to name the projection type
returned from the method.

```
use std::pin::Pin;

use pin_project::pin_project;

#[pin_project(project = EnumProj)]
enum Enum<T, U> {
    Pinned(#[pin] T),
    Unpinned(U),
}

impl<T, U> Enum<T, U> {
    fn method(self: Pin<&mut Self>) {
        match self.project() {
            EnumProj::Pinned(x) => {
                let _: Pin<&mut T> = x;
            }
            EnumProj::Unpinned(y) => {
                let _: &mut U = y;
            }
        }
    }
}
```

[*code like this will be generated*][enum-default-expanded]

See [`#[pin_project]`][`pin_project`] attribute for more details, and
see [examples] directory for more examples and generated code.

## Related Projects

- [pin-project-lite]: A lightweight version of pin-project written with declarative macros.

[enum-default-expanded]: https://github.com/taiki-e/pin-project/blob/HEAD/examples/enum-default-expanded.rs
[examples]: https://github.com/taiki-e/pin-project/blob/HEAD/examples/README.md
[pin-project-lite]: https://github.com/taiki-e/pin-project-lite
[pin-projection]: https://doc.rust-lang.org/std/pin/index.html#projections-and-structural-pinning
[struct-default-expanded]: https://github.com/taiki-e/pin-project/blob/HEAD/examples/struct-default-expanded.rs

<!-- tidy:sync-markdown-to-rustdoc:end -->
*/

#![no_std]
#![doc(test(
    no_crate_inject,
    attr(
        deny(warnings, rust_2018_idioms, single_use_lifetimes),
        allow(dead_code, unused_variables)
    )
))]
#![warn(unsafe_op_in_unsafe_fn)]
#![warn(
    // Lints that may help when writing public library.
    missing_debug_implementations,
    missing_docs,
    clippy::alloc_instead_of_core,
    clippy::exhaustive_enums,
    clippy::exhaustive_structs,
    clippy::impl_trait_in_params,
    // clippy::missing_inline_in_public_items,
    clippy::std_instead_of_alloc,
    clippy::std_instead_of_core,
)]
#![allow(clippy::needless_doctest_main)]

#[doc(inline)]
pub use pin_project_internal::pin_project;
#[doc(inline)]
pub use pin_project_internal::pinned_drop;

/// A trait used for custom implementations of [`Unpin`].
///
/// This trait is used in conjunction with the `UnsafeUnpin` argument to
/// the [`#[pin_project]`][macro@pin_project] attribute.
///
/// # Safety
///
/// The Rust [`Unpin`] trait is safe to implement - by itself,
/// implementing it cannot lead to [undefined behavior][undefined-behavior].
/// Undefined behavior can only occur when other unsafe code is used.
///
/// It turns out that using pin projections, which requires unsafe code,
/// imposes additional requirements on an [`Unpin`] impl. Normally, all of this
/// unsafety is contained within this crate, ensuring that it's impossible for
/// you to violate any of the guarantees required by pin projection.
///
/// However, things change if you want to provide a custom [`Unpin`] impl
/// for your `#[pin_project]` type. As stated in [the Rust
/// documentation][pin-projection], you must be sure to only implement [`Unpin`]
/// when all of your `#[pin]` fields (i.e. structurally pinned fields) are also
/// [`Unpin`].
///
/// To help highlight this unsafety, the `UnsafeUnpin` trait is provided.
/// Implementing this trait is logically equivalent to implementing [`Unpin`] -
/// this crate will generate an [`Unpin`] impl for your type that 'forwards' to
/// your `UnsafeUnpin` impl. However, this trait is `unsafe` - since your type
/// uses structural pinning (otherwise, you wouldn't be using this crate!),
/// you must be sure that your `UnsafeUnpin` impls follows all of
/// the requirements for an [`Unpin`] impl of a structurally-pinned type.
///
/// Note that if you specify `#[pin_project(UnsafeUnpin)]`, but do *not*
/// provide an impl of `UnsafeUnpin`, your type will never implement [`Unpin`].
/// This is effectively the same thing as adding a [`PhantomPinned`] to your
/// type.
///
/// Since this trait is `unsafe`, impls of it will be detected by the
/// `unsafe_code` lint, and by tools like [`cargo geiger`][cargo-geiger].
///
/// # Examples
///
/// An `UnsafeUnpin` impl which, in addition to requiring that structurally
/// pinned fields be [`Unpin`], imposes an additional requirement:
///
/// ```
/// use pin_project::{UnsafeUnpin, pin_project};
///
/// #[pin_project(UnsafeUnpin)]
/// struct Struct<K, V> {
///     #[pin]
///     field_1: K,
///     field_2: V,
/// }
///
/// unsafe impl<K, V> UnsafeUnpin for Struct<K, V> where K: Unpin + Clone {}
/// ```
///
/// [`PhantomPinned`]: core::marker::PhantomPinned
/// [cargo-geiger]: https://github.com/rust-secure-code/cargo-geiger
/// [pin-projection]: core::pin#projections-and-structural-pinning
/// [undefined-behavior]: https://doc.rust-lang.org/reference/behavior-considered-undefined.html
pub unsafe trait UnsafeUnpin {}

// Not public API.
#[doc(hidden)]
#[allow(missing_debug_implementations)]
pub mod __private {
    use core::mem::ManuallyDrop;
    #[doc(hidden)]
    pub use core::{
        marker::{PhantomData, PhantomPinned, Unpin},
        ops::Drop,
        pin::Pin,
        ptr,
    };

    #[doc(hidden)]
    pub use pin_project_internal::__PinProjectInternalDerive;

    use super::UnsafeUnpin;

    // An internal trait used for custom implementations of [`Drop`].
    //
    // **Do not call or implement this trait directly.**
    //
    // # Why this trait is private and `#[pinned_drop]` attribute is needed?
    //
    // Implementing `PinnedDrop::drop` is safe, but calling it is not safe.
    // This is because destructors can be called multiple times in safe code and
    // [double dropping is unsound][rust-lang/rust#62360].
    //
    // Ideally, it would be desirable to be able to forbid manual calls in
    // the same way as [`Drop::drop`], but the library cannot do it. So, by using
    // macros and replacing them with private traits,
    // this crate prevent users from calling `PinnedDrop::drop` in safe code.
    //
    // This allows implementing [`Drop`] safely using `#[pinned_drop]`.
    // Also by using the [`drop`] function just like dropping a type that directly
    // implements [`Drop`], can drop safely a type that implements `PinnedDrop`.
    //
    // [rust-lang/rust#62360]: https://github.com/rust-lang/rust/pull/62360
    #[doc(hidden)]
    pub trait PinnedDrop {
        #[doc(hidden)]
        unsafe fn drop(self: Pin<&mut Self>);
    }

    // This is an internal helper struct used by `pin-project-internal`.
    // This allows us to force an error if the user tries to provide
    // a regular `Unpin` impl when they specify the `UnsafeUnpin` argument.
    // This is why we need Wrapper:
    //
    // Supposed we have the following code:
    //
    // ```
    // #[pin_project(UnsafeUnpin)]
    // struct MyStruct<T> {
    //     #[pin] field: T
    // }
    //
    // impl<T> Unpin for MyStruct<T> where MyStruct<T>: UnsafeUnpin {} // generated by pin-project-internal
    // impl<T> Unpin for MyStruct<T> where T: Copy // written by the user
    // ```
    //
    // We want this code to be rejected - the user is completely bypassing
    // `UnsafeUnpin`, and providing an unsound Unpin impl in safe code!
    //
    // Unfortunately, the Rust compiler will accept the above code.
    // Because MyStruct is declared in the same crate as the user-provided impl,
    // the compiler will notice that `MyStruct<T>: UnsafeUnpin` never holds.
    //
    // The solution is to introduce the `Wrapper` struct, which is defined
    // in the `pin-project` crate.
    //
    // We now have code that looks like this:
    //
    // ```
    // impl<T> Unpin for MyStruct<T> where Wrapper<MyStruct<T>>: UnsafeUnpin {} // generated by pin-project-internal
    // impl<T> Unpin for MyStruct<T> where T: Copy // written by the user
    // ```
    //
    // We also have `unsafe impl<T> UnsafeUnpin for Wrapper<T> where T: UnsafeUnpin {}`
    // in the `pin-project` crate.
    //
    // Now, our generated impl has a bound involving a type defined in another
    // crate - Wrapper. This will cause rust to conservatively assume that
    // `Wrapper<MyStruct<T>>: UnsafeUnpin` holds, in the interest of preserving
    // forwards compatibility (in case such an impl is added for Wrapper<T> in
    // a new version of the crate).
    //
    // This will cause rust to reject any other `Unpin` impls for MyStruct<T>,
    // since it will assume that our generated impl could potentially apply in
    // any situation.
    //
    // This achieves the desired effect - when the user writes
    // `#[pin_project(UnsafeUnpin)]`, the user must either provide no impl of
    // `UnsafeUnpin` (which is equivalent to making the type never implement
    // Unpin), or provide an impl of `UnsafeUnpin`. It is impossible for them to
    // provide an impl of `Unpin`
    #[doc(hidden)]
    #[allow(dead_code)]
    pub struct Wrapper<'a, T: ?Sized>(PhantomData<&'a ()>, T);
    // SAFETY: `T` implements UnsafeUnpin.
    unsafe impl<T: ?Sized + UnsafeUnpin> UnsafeUnpin for Wrapper<'_, T> {}

    // Workaround for issue on unstable negative_impls feature that allows unsound overlapping Unpin
    // implementations and rustc bug that leaks unstable negative_impls into stable.
    // See https://github.com/taiki-e/pin-project/issues/340#issuecomment-2432146009 for details.
    #[doc(hidden)]
    pub type PinnedFieldsOf<T> =
        <PinnedFieldsOfHelperStruct<T> as PinnedFieldsOfHelperTrait>::Actual;
    // We cannot use <Option<T> as IntoIterator>::Item or similar since we should allow ?Sized in T.
    #[doc(hidden)]
    pub trait PinnedFieldsOfHelperTrait {
        type Actual: ?Sized;
    }
    #[doc(hidden)]
    pub struct PinnedFieldsOfHelperStruct<T: ?Sized>(T);
    impl<T: ?Sized> PinnedFieldsOfHelperTrait for PinnedFieldsOfHelperStruct<T> {
        type Actual = T;
    }

    // This is an internal helper struct used by `pin-project-internal`.
    //
    // See https://github.com/taiki-e/pin-project/pull/53 for more details.
    #[doc(hidden)]
    pub struct AlwaysUnpin<'a, T>(PhantomData<&'a ()>, PhantomData<T>);
    impl<T> Unpin for AlwaysUnpin<'_, T> {}

    // This is an internal helper used to ensure a value is dropped.
    #[doc(hidden)]
    pub struct UnsafeDropInPlaceGuard<T: ?Sized>(*mut T);
    impl<T: ?Sized> UnsafeDropInPlaceGuard<T> {
        #[doc(hidden)]
        pub unsafe fn new(ptr: *mut T) -> Self {
            Self(ptr)
        }
    }
    impl<T: ?Sized> Drop for UnsafeDropInPlaceGuard<T> {
        fn drop(&mut self) {
            // SAFETY: the caller of `UnsafeDropInPlaceGuard::new` must guarantee
            // that `ptr` is valid for drop when this guard is destructed.
            unsafe {
                ptr::drop_in_place(self.0);
            }
        }
    }

    // This is an internal helper used to ensure a value is overwritten without
    // its destructor being called.
    #[doc(hidden)]
    pub struct UnsafeOverwriteGuard<T> {
        target: *mut T,
        value: ManuallyDrop<T>,
    }
    impl<T> UnsafeOverwriteGuard<T> {
        #[doc(hidden)]
        pub unsafe fn new(target: *mut T, value: T) -> Self {
            Self { target, value: ManuallyDrop::new(value) }
        }
    }
    impl<T> Drop for UnsafeOverwriteGuard<T> {
        fn drop(&mut self) {
            // SAFETY: the caller of `UnsafeOverwriteGuard::new` must guarantee
            // that `target` is valid for writes when this guard is destructed.
            unsafe {
                ptr::write(self.target, ptr::read(&*self.value));
            }
        }
    }
}
