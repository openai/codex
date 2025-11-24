//! Local versions of types that bindgen would use.

use crate::utils::{as_mut_ptr, as_ptr};

/// This represents an incomplete array field at the end of a struct.
///
/// This is called `__IncompleteArrayField` in bindgen bindings.
#[repr(C)]
#[derive(Default)]
pub struct IncompleteArrayField<T>(::core::marker::PhantomData<T>, [T; 0]);

#[allow(missing_docs)]
impl<T> IncompleteArrayField<T> {
    #[inline]
    pub const fn new() -> Self {
        Self(::core::marker::PhantomData, [])
    }

    #[inline]
    pub fn as_ptr(&self) -> *const T {
        as_ptr(self).cast::<T>()
    }

    #[inline]
    pub fn as_mut_ptr(&mut self) -> *mut T {
        as_mut_ptr(self).cast::<T>()
    }

    #[inline]
    pub unsafe fn as_slice(&self, len: usize) -> &[T] {
        ::core::slice::from_raw_parts(self.as_ptr(), len)
    }

    #[inline]
    pub unsafe fn as_mut_slice(&mut self, len: usize) -> &mut [T] {
        ::core::slice::from_raw_parts_mut(self.as_mut_ptr(), len)
    }
}

impl<T> ::core::fmt::Debug for IncompleteArrayField<T> {
    fn fmt(&self, fmt: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        fmt.write_str("IncompleteArrayField")
    }
}

/// This represents a toplevel union field.
///
/// This is called `__BindgenUnionField` in bindgen bindings.
pub struct UnionField<T>(::core::marker::PhantomData<T>);

#[allow(missing_docs)]
impl<T> UnionField<T> {
    #[inline]
    pub const fn new() -> Self {
        Self(::core::marker::PhantomData)
    }

    #[inline]
    pub unsafe fn as_ref(&self) -> &T {
        ::core::mem::transmute(self)
    }

    #[inline]
    pub unsafe fn as_mut(&mut self) -> &mut T {
        ::core::mem::transmute(self)
    }
}

impl<T> ::core::default::Default for UnionField<T> {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl<T> ::core::clone::Clone for UnionField<T> {
    #[inline]
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> ::core::marker::Copy for UnionField<T> {}

impl<T> ::core::fmt::Debug for UnionField<T> {
    fn fmt(&self, fmt: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        fmt.write_str("UnionField")
    }
}

impl<T> ::core::hash::Hash for UnionField<T> {
    fn hash<H: ::core::hash::Hasher>(&self, _state: &mut H) {}
}

impl<T> ::core::cmp::PartialEq for UnionField<T> {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl<T> ::core::cmp::Eq for UnionField<T> {}
