//! A framework for Rust wrappers over C APIs.
//!
//! Ownership is as important in C as it is in Rust, but the semantics are often implicit. In
//! particular, pointer-to-value is commonly used to pass C values both when transferring ownership
//! or a borrow.
//!
//! This crate provides a framework to define a Rust wrapper over these kinds of raw C APIs in a way
//! that allows ownership semantics to be expressed in an ergonomic manner. The framework takes a
//! dual-type approach similar to APIs in the standard library such as `PathBuf`/`Path` or `String`/
//! `str`. One type represents an owned value and references to the other represent borrowed
//! values.
//!
//! # Examples
//!
//! ```
//! use foreign_types::{ForeignType, ForeignTypeRef, Opaque};
//! use std::ops::{Deref, DerefMut};
//!
//! mod foo_sys {
//!     pub enum FOO {}
//!
//!     extern {
//!         pub fn FOO_free(foo: *mut FOO);
//!     }
//! }
//!
//! // The borrowed type is a newtype wrapper around an `Opaque` value.
//! //
//! // `FooRef` values never exist; we instead create references to `FooRef`s
//! // from raw C pointers.
//! pub struct FooRef(Opaque);
//!
//! impl ForeignTypeRef for FooRef {
//!     type CType = foo_sys::FOO;
//! }
//!
//! // The owned type is simply a newtype wrapper around the raw C type.
//! //
//! // It dereferences to `FooRef`, so methods that do not require ownership
//! // should be defined there.
//! pub struct Foo(*mut foo_sys::FOO);
//!
//! impl Drop for Foo {
//!     fn drop(&mut self) {
//!         unsafe { foo_sys::FOO_free(self.0) }
//!     }
//! }
//!
//! impl ForeignType for Foo {
//!     type CType = foo_sys::FOO;
//!     type Ref = FooRef;
//!
//!     unsafe fn from_ptr(ptr: *mut foo_sys::FOO) -> Foo {
//!         Foo(ptr)
//!     }
//!
//!     fn as_ptr(&self) -> *mut foo_sys::FOO {
//!         self.0
//!     }
//! }
//!
//! impl Deref for Foo {
//!     type Target = FooRef;
//!
//!     fn deref(&self) -> &FooRef {
//!         unsafe { FooRef::from_ptr(self.0) }
//!     }
//! }
//!
//! impl DerefMut for Foo {
//!     fn deref_mut(&mut self) -> &mut FooRef {
//!         unsafe { FooRef::from_ptr_mut(self.0) }
//!     }
//! }
//! ```
//!
//! The `foreign_type!` macro can generate this boilerplate for you:
//!
//! ```
//! #[macro_use]
//! extern crate foreign_types;
//!
//! mod foo_sys {
//!     pub enum FOO {}
//!
//!     extern {
//!         pub fn FOO_free(foo: *mut FOO);
//!         pub fn FOO_duplicate(foo: *mut FOO) -> *mut FOO; // Optional
//!     }
//! }
//!
//! foreign_type! {
//!     type CType = foo_sys::FOO;
//!     fn drop = foo_sys::FOO_free;
//!     fn clone = foo_sys::FOO_duplicate; // Optional
//!     /// A Foo.
//!     pub struct Foo;
//!     /// A borrowed Foo.
//!     pub struct FooRef;
//! }
//!
//! # fn main() {}
//! ```
//!
//! If `fn clone` is specified, then it must take `CType` as an argument and return a copy of it as `CType`.
//! It will be used to implement `ToOwned` and `Clone`.
//!
//! `#[derive(…)] is permitted before the lines with `pub struct`.
//! `#[doc(hidden)]` before the `type CType` line will hide the `foreign_type!` implementations from documentation.
//!
//! Say we then have a separate type in our C API that contains a `FOO`:
//!
//! ```
//! mod foo_sys {
//!     pub enum FOO {}
//!     pub enum BAR {}
//!
//!     extern {
//!         pub fn FOO_free(foo: *mut FOO);
//!         pub fn BAR_free(bar: *mut BAR);
//!         pub fn BAR_get_foo(bar: *mut BAR) -> *mut FOO;
//!     }
//! }
//! ```
//!
//! The documentation for the C library states that `BAR_get_foo` returns a reference into the `BAR`
//! passed to it, which translates into a reference in Rust. It also says that we're allowed to
//! modify the `FOO`, so we'll define a pair of accessor methods, one immutable and one mutable:
//!
//! ```
//! #[macro_use]
//! extern crate foreign_types;
//!
//! use foreign_types::ForeignTypeRef;
//!
//! mod foo_sys {
//!     pub enum FOO {}
//!     pub enum BAR {}
//!
//!     extern {
//!         pub fn FOO_free(foo: *mut FOO);
//!         pub fn BAR_free(bar: *mut BAR);
//!         pub fn BAR_get_foo(bar: *mut BAR) -> *mut FOO;
//!     }
//! }
//!
//! foreign_type! {
//!     #[doc(hidden)]
//!     type CType = foo_sys::FOO;
//!     fn drop = foo_sys::FOO_free;
//!     /// A Foo.
//!     pub struct Foo;
//!     /// A borrowed Foo.
//!     pub struct FooRef;
//! }
//!
//! foreign_type! {
//!     type CType = foo_sys::BAR;
//!     fn drop = foo_sys::BAR_free;
//!     /// A Foo.
//!     pub struct Bar;
//!     /// A borrowed Bar.
//!     pub struct BarRef;
//! }
//!
//! impl BarRef {
//!     fn foo(&self) -> &FooRef {
//!         unsafe { FooRef::from_ptr(foo_sys::BAR_get_foo(self.as_ptr())) }
//!     }
//!
//!     fn foo_mut(&mut self) -> &mut FooRef {
//!         unsafe { FooRef::from_ptr_mut(foo_sys::BAR_get_foo(self.as_ptr())) }
//!     }
//! }
//!
//! # fn main() {}
//! ```
#![no_std]
#![warn(missing_docs)]
#![doc(html_root_url="https://docs.rs/foreign-types/0.3")]
extern crate foreign_types_shared;

#[doc(inline)]
pub use foreign_types_shared::*;

/// A macro to easily define wrappers for foreign types.
///
/// # Examples
///
/// ```
/// #[macro_use]
/// extern crate foreign_types;
///
/// # mod openssl_sys { pub type SSL = (); pub unsafe fn SSL_free(_: *mut SSL) {} pub unsafe fn SSL_dup(x: *mut SSL) -> *mut SSL {x} }
/// foreign_type! {
///     type CType = openssl_sys::SSL;
///     fn drop = openssl_sys::SSL_free;
///     fn clone = openssl_sys::SSL_dup;
///     /// Documentation for the owned type.
///     pub struct Ssl;
///     /// Documentation for the borrowed type.
///     pub struct SslRef;
/// }
///
/// # fn main() {}
/// ```
#[macro_export]
macro_rules! foreign_type {
    (
        $(#[$impl_attr:meta])*
        type CType = $ctype:ty;
        fn drop = $drop:expr;
        $(fn clone = $clone:expr;)*
        $(#[$owned_attr:meta])*
        pub struct $owned:ident;
        $(#[$borrowed_attr:meta])*
        pub struct $borrowed:ident;
    ) => {
        $(#[$owned_attr])*
        pub struct $owned(*mut $ctype);

        $(#[$impl_attr])*
        impl $crate::ForeignType for $owned {
            type CType = $ctype;
            type Ref = $borrowed;

            #[inline]
            unsafe fn from_ptr(ptr: *mut $ctype) -> $owned {
                $owned(ptr)
            }

            #[inline]
            fn as_ptr(&self) -> *mut $ctype {
                self.0
            }
        }

        impl Drop for $owned {
            #[inline]
            fn drop(&mut self) {
                unsafe { $drop(self.0) }
            }
        }

        $(
            impl Clone for $owned {
                #[inline]
                fn clone(&self) -> $owned {
                    unsafe {
                        let handle: *mut $ctype = $clone(self.0);
                        $crate::ForeignType::from_ptr(handle)
                    }
                }
            }

            impl ::std::borrow::ToOwned for $borrowed {
                type Owned = $owned;
                #[inline]
                fn to_owned(&self) -> $owned {
                    unsafe {
                        let handle: *mut $ctype = $clone($crate::ForeignTypeRef::as_ptr(self));
                        $crate::ForeignType::from_ptr(handle)
                    }
                }
            }
        )*

        impl ::std::ops::Deref for $owned {
            type Target = $borrowed;

            #[inline]
            fn deref(&self) -> &$borrowed {
                unsafe { $crate::ForeignTypeRef::from_ptr(self.0) }
            }
        }

        impl ::std::ops::DerefMut for $owned {
            #[inline]
            fn deref_mut(&mut self) -> &mut $borrowed {
                unsafe { $crate::ForeignTypeRef::from_ptr_mut(self.0) }
            }
        }

        impl ::std::borrow::Borrow<$borrowed> for $owned {
            #[inline]
            fn borrow(&self) -> &$borrowed {
                &**self
            }
        }

        impl ::std::convert::AsRef<$borrowed> for $owned {
            #[inline]
            fn as_ref(&self) -> &$borrowed {
                &**self
            }
        }

        $(#[$borrowed_attr])*
        pub struct $borrowed($crate::Opaque);

        $(#[$impl_attr])*
        impl $crate::ForeignTypeRef for $borrowed {
            type CType = $ctype;
        }
    }
}
