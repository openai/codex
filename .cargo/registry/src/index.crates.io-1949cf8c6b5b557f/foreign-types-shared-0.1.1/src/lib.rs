//! Internal crate used by foreign-types

#![no_std]
#![warn(missing_docs)]
#![doc(html_root_url="https://docs.rs/foreign-types-shared/0.1")]

use core::cell::UnsafeCell;

/// An opaque type used to define `ForeignTypeRef` types.
///
/// A type implementing `ForeignTypeRef` should simply be a newtype wrapper around this type.
pub struct Opaque(UnsafeCell<()>);

/// A type implemented by wrappers over foreign types.
pub trait ForeignType: Sized {
    /// The raw C type.
    type CType;

    /// The type representing a reference to this type.
    type Ref: ForeignTypeRef<CType = Self::CType>;

    /// Constructs an instance of this type from its raw type.
    unsafe fn from_ptr(ptr: *mut Self::CType) -> Self;

    /// Returns a raw pointer to the wrapped value.
    fn as_ptr(&self) -> *mut Self::CType;
}

/// A trait implemented by types which reference borrowed foreign types.
pub trait ForeignTypeRef: Sized {
    /// The raw C type.
    type CType;

    /// Constructs a shared instance of this type from its raw type.
    #[inline]
    unsafe fn from_ptr<'a>(ptr: *mut Self::CType) -> &'a Self {
        &*(ptr as *mut _)
    }

    /// Constructs a mutable reference of this type from its raw type.
    #[inline]
    unsafe fn from_ptr_mut<'a>(ptr: *mut Self::CType) -> &'a mut Self {
        &mut *(ptr as *mut _)
    }

    /// Returns a raw pointer to the wrapped value.
    #[inline]
    fn as_ptr(&self) -> *mut Self::CType {
        self as *const _ as *mut _
    }
}
