use cfg_if::cfg_if;
use foreign_types::{ForeignType, ForeignTypeRef, Opaque};
use libc::c_int;
use std::borrow::Borrow;
use std::convert::AsRef;
use std::fmt;
use std::iter;
use std::marker::PhantomData;
use std::mem;
use std::ops::{Deref, DerefMut, Index, IndexMut, Range};

use crate::error::ErrorStack;
use crate::util::ForeignTypeExt;
use crate::{cvt, cvt_p, LenType};

cfg_if! {
    if #[cfg(any(ossl110, boringssl, awslc))] {
        use ffi::{
            OPENSSL_sk_pop, OPENSSL_sk_free, OPENSSL_sk_num, OPENSSL_sk_value, OPENSSL_STACK,
            OPENSSL_sk_new_null, OPENSSL_sk_push,
        };
    } else {
        use ffi::{
            sk_pop as OPENSSL_sk_pop, sk_free as OPENSSL_sk_free, sk_num as OPENSSL_sk_num,
            sk_value as OPENSSL_sk_value, _STACK as OPENSSL_STACK,
            sk_new_null as OPENSSL_sk_new_null, sk_push as OPENSSL_sk_push,
        };
    }
}

/// Trait implemented by types which can be placed in a stack.
///
/// It should not be implemented for any type outside of this crate.
pub trait Stackable: ForeignType {
    /// The C stack type for this element.
    ///
    /// Generally called `stack_st_{ELEMENT_TYPE}`, normally hidden by the
    /// `STACK_OF(ELEMENT_TYPE)` macro in the OpenSSL API.
    type StackType;
}

/// An owned stack of `T`.
pub struct Stack<T: Stackable>(*mut T::StackType);

unsafe impl<T: Stackable + Send> Send for Stack<T> {}
unsafe impl<T: Stackable + Sync> Sync for Stack<T> {}

impl<T> fmt::Debug for Stack<T>
where
    T: Stackable,
    T::Ref: fmt::Debug,
{
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_list().entries(self).finish()
    }
}
impl<T: Stackable> Drop for Stack<T> {
    fn drop(&mut self) {
        unsafe {
            while self.pop().is_some() {}
            OPENSSL_sk_free(self.0 as *mut _);
        }
    }
}

impl<T: Stackable> Stack<T> {
    pub fn new() -> Result<Stack<T>, ErrorStack> {
        unsafe {
            ffi::init();
            let ptr = cvt_p(OPENSSL_sk_new_null())?;
            Ok(Stack(ptr as *mut _))
        }
    }
}

impl<T: Stackable> iter::IntoIterator for Stack<T> {
    type IntoIter = IntoIter<T>;
    type Item = T;

    fn into_iter(self) -> IntoIter<T> {
        let it = IntoIter {
            stack: self.0,
            idxs: 0..self.len() as LenType,
        };
        mem::forget(self);
        it
    }
}

impl<T: Stackable> AsRef<StackRef<T>> for Stack<T> {
    fn as_ref(&self) -> &StackRef<T> {
        self
    }
}

impl<T: Stackable> Borrow<StackRef<T>> for Stack<T> {
    fn borrow(&self) -> &StackRef<T> {
        self
    }
}

impl<T: Stackable> ForeignType for Stack<T> {
    type CType = T::StackType;
    type Ref = StackRef<T>;

    #[inline]
    unsafe fn from_ptr(ptr: *mut T::StackType) -> Stack<T> {
        assert!(
            !ptr.is_null(),
            "Must not instantiate a Stack from a null-ptr - use Stack::new() in \
             that case"
        );
        Stack(ptr)
    }

    #[inline]
    fn as_ptr(&self) -> *mut T::StackType {
        self.0
    }
}

impl<T: Stackable> Deref for Stack<T> {
    type Target = StackRef<T>;

    fn deref(&self) -> &StackRef<T> {
        unsafe { StackRef::from_ptr(self.0) }
    }
}

impl<T: Stackable> DerefMut for Stack<T> {
    fn deref_mut(&mut self) -> &mut StackRef<T> {
        unsafe { StackRef::from_ptr_mut(self.0) }
    }
}

pub struct IntoIter<T: Stackable> {
    stack: *mut T::StackType,
    idxs: Range<LenType>,
}

impl<T: Stackable> Drop for IntoIter<T> {
    fn drop(&mut self) {
        unsafe {
            // https://github.com/rust-lang/rust-clippy/issues/7510
            #[allow(clippy::while_let_on_iterator)]
            while let Some(_) = self.next() {}
            OPENSSL_sk_free(self.stack as *mut _);
        }
    }
}

impl<T: Stackable> Iterator for IntoIter<T> {
    type Item = T;

    fn next(&mut self) -> Option<T> {
        unsafe {
            self.idxs
                .next()
                .map(|i| T::from_ptr(OPENSSL_sk_value(self.stack as *mut _, i) as *mut _))
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.idxs.size_hint()
    }
}

impl<T: Stackable> DoubleEndedIterator for IntoIter<T> {
    fn next_back(&mut self) -> Option<T> {
        unsafe {
            self.idxs
                .next_back()
                .map(|i| T::from_ptr(OPENSSL_sk_value(self.stack as *mut _, i) as *mut _))
        }
    }
}

impl<T: Stackable> ExactSizeIterator for IntoIter<T> {}

pub struct StackRef<T: Stackable>(Opaque, PhantomData<T>);

unsafe impl<T: Stackable + Send> Send for StackRef<T> {}
unsafe impl<T: Stackable + Sync> Sync for StackRef<T> {}

impl<T: Stackable> ForeignTypeRef for StackRef<T> {
    type CType = T::StackType;
}

impl<T: Stackable> StackRef<T> {
    fn as_stack(&self) -> *mut OPENSSL_STACK {
        self.as_ptr() as *mut _
    }

    /// Returns the number of items in the stack.
    pub fn len(&self) -> usize {
        unsafe { OPENSSL_sk_num(self.as_stack()) as usize }
    }

    /// Determines if the stack is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn iter(&self) -> Iter<'_, T> {
        Iter {
            stack: self,
            idxs: 0..self.len() as LenType,
        }
    }

    pub fn iter_mut(&mut self) -> IterMut<'_, T> {
        IterMut {
            idxs: 0..self.len() as LenType,
            stack: self,
        }
    }

    /// Returns a reference to the element at the given index in the
    /// stack or `None` if the index is out of bounds
    pub fn get(&self, idx: usize) -> Option<&T::Ref> {
        unsafe {
            if idx >= self.len() {
                return None;
            }

            Some(T::Ref::from_ptr(self._get(idx)))
        }
    }

    /// Returns a mutable reference to the element at the given index in the
    /// stack or `None` if the index is out of bounds
    pub fn get_mut(&mut self, idx: usize) -> Option<&mut T::Ref> {
        unsafe {
            if idx >= self.len() {
                return None;
            }

            Some(T::Ref::from_ptr_mut(self._get(idx)))
        }
    }

    /// Pushes a value onto the top of the stack.
    pub fn push(&mut self, data: T) -> Result<(), ErrorStack> {
        unsafe {
            cvt(OPENSSL_sk_push(self.as_stack(), data.as_ptr() as *mut _) as c_int)?;
            mem::forget(data);
            Ok(())
        }
    }

    /// Removes the last element from the stack and returns it.
    pub fn pop(&mut self) -> Option<T> {
        unsafe {
            let ptr = OPENSSL_sk_pop(self.as_stack());
            T::from_ptr_opt(ptr as *mut _)
        }
    }

    unsafe fn _get(&self, idx: usize) -> *mut T::CType {
        OPENSSL_sk_value(self.as_stack(), idx as LenType) as *mut _
    }
}

impl<T: Stackable> Index<usize> for StackRef<T> {
    type Output = T::Ref;

    fn index(&self, index: usize) -> &T::Ref {
        self.get(index).unwrap()
    }
}

impl<T: Stackable> IndexMut<usize> for StackRef<T> {
    fn index_mut(&mut self, index: usize) -> &mut T::Ref {
        self.get_mut(index).unwrap()
    }
}

impl<'a, T: Stackable> iter::IntoIterator for &'a StackRef<T> {
    type Item = &'a T::Ref;
    type IntoIter = Iter<'a, T>;

    fn into_iter(self) -> Iter<'a, T> {
        self.iter()
    }
}

impl<'a, T: Stackable> iter::IntoIterator for &'a mut StackRef<T> {
    type Item = &'a mut T::Ref;
    type IntoIter = IterMut<'a, T>;

    fn into_iter(self) -> IterMut<'a, T> {
        self.iter_mut()
    }
}

impl<'a, T: Stackable> iter::IntoIterator for &'a Stack<T> {
    type Item = &'a T::Ref;
    type IntoIter = Iter<'a, T>;

    fn into_iter(self) -> Iter<'a, T> {
        self.iter()
    }
}

impl<'a, T: Stackable> iter::IntoIterator for &'a mut Stack<T> {
    type Item = &'a mut T::Ref;
    type IntoIter = IterMut<'a, T>;

    fn into_iter(self) -> IterMut<'a, T> {
        self.iter_mut()
    }
}

/// An iterator over the stack's contents.
pub struct Iter<'a, T: Stackable> {
    stack: &'a StackRef<T>,
    idxs: Range<LenType>,
}

impl<'a, T: Stackable> Iterator for Iter<'a, T> {
    type Item = &'a T::Ref;

    fn next(&mut self) -> Option<&'a T::Ref> {
        unsafe {
            self.idxs
                .next()
                .map(|i| T::Ref::from_ptr(OPENSSL_sk_value(self.stack.as_stack(), i) as *mut _))
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.idxs.size_hint()
    }
}

impl<'a, T: Stackable> DoubleEndedIterator for Iter<'a, T> {
    fn next_back(&mut self) -> Option<&'a T::Ref> {
        unsafe {
            self.idxs
                .next_back()
                .map(|i| T::Ref::from_ptr(OPENSSL_sk_value(self.stack.as_stack(), i) as *mut _))
        }
    }
}

impl<T: Stackable> ExactSizeIterator for Iter<'_, T> {}

/// A mutable iterator over the stack's contents.
pub struct IterMut<'a, T: Stackable> {
    stack: &'a mut StackRef<T>,
    idxs: Range<LenType>,
}

impl<'a, T: Stackable> Iterator for IterMut<'a, T> {
    type Item = &'a mut T::Ref;

    fn next(&mut self) -> Option<&'a mut T::Ref> {
        unsafe {
            self.idxs
                .next()
                .map(|i| T::Ref::from_ptr_mut(OPENSSL_sk_value(self.stack.as_stack(), i) as *mut _))
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.idxs.size_hint()
    }
}

impl<'a, T: Stackable> DoubleEndedIterator for IterMut<'a, T> {
    fn next_back(&mut self) -> Option<&'a mut T::Ref> {
        unsafe {
            self.idxs
                .next_back()
                .map(|i| T::Ref::from_ptr_mut(OPENSSL_sk_value(self.stack.as_stack(), i) as *mut _))
        }
    }
}

impl<T: Stackable> ExactSizeIterator for IterMut<'_, T> {}
