use alloc::boxed::Box;
use core::marker::PhantomData;
use core::ptr::NonNull;

#[repr(transparent)]
pub struct Own<T>
where
    T: ?Sized,
{
    pub ptr: NonNull<T>,
}

unsafe impl<T> Send for Own<T> where T: ?Sized {}

unsafe impl<T> Sync for Own<T> where T: ?Sized {}

impl<T> Copy for Own<T> where T: ?Sized {}

impl<T> Clone for Own<T>
where
    T: ?Sized,
{
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Own<T>
where
    T: ?Sized,
{
    pub fn new(ptr: Box<T>) -> Self {
        Own {
            ptr: unsafe { NonNull::new_unchecked(Box::into_raw(ptr)) },
        }
    }

    pub fn cast<U: CastTo>(self) -> Own<U::Target> {
        Own {
            ptr: self.ptr.cast(),
        }
    }

    pub unsafe fn boxed(self) -> Box<T> {
        unsafe { Box::from_raw(self.ptr.as_ptr()) }
    }

    pub fn by_ref(&self) -> Ref<T> {
        Ref {
            ptr: self.ptr,
            lifetime: PhantomData,
        }
    }

    pub fn by_mut(&mut self) -> Mut<T> {
        Mut {
            ptr: self.ptr,
            lifetime: PhantomData,
        }
    }
}

#[repr(transparent)]
pub struct Ref<'a, T>
where
    T: ?Sized,
{
    pub ptr: NonNull<T>,
    lifetime: PhantomData<&'a T>,
}

impl<'a, T> Copy for Ref<'a, T> where T: ?Sized {}

impl<'a, T> Clone for Ref<'a, T>
where
    T: ?Sized,
{
    fn clone(&self) -> Self {
        *self
    }
}

impl<'a, T> Ref<'a, T>
where
    T: ?Sized,
{
    pub fn new(ptr: &'a T) -> Self {
        Ref {
            ptr: NonNull::from(ptr),
            lifetime: PhantomData,
        }
    }

    #[cfg(not(anyhow_no_ptr_addr_of))]
    pub fn from_raw(ptr: NonNull<T>) -> Self {
        Ref {
            ptr,
            lifetime: PhantomData,
        }
    }

    pub fn cast<U: CastTo>(self) -> Ref<'a, U::Target> {
        Ref {
            ptr: self.ptr.cast(),
            lifetime: PhantomData,
        }
    }

    #[cfg(not(anyhow_no_ptr_addr_of))]
    pub fn by_mut(self) -> Mut<'a, T> {
        Mut {
            ptr: self.ptr,
            lifetime: PhantomData,
        }
    }

    #[cfg(not(anyhow_no_ptr_addr_of))]
    pub fn as_ptr(self) -> *const T {
        self.ptr.as_ptr() as *const T
    }

    pub unsafe fn deref(self) -> &'a T {
        unsafe { &*self.ptr.as_ptr() }
    }
}

#[repr(transparent)]
pub struct Mut<'a, T>
where
    T: ?Sized,
{
    pub ptr: NonNull<T>,
    lifetime: PhantomData<&'a mut T>,
}

impl<'a, T> Copy for Mut<'a, T> where T: ?Sized {}

impl<'a, T> Clone for Mut<'a, T>
where
    T: ?Sized,
{
    fn clone(&self) -> Self {
        *self
    }
}

impl<'a, T> Mut<'a, T>
where
    T: ?Sized,
{
    #[cfg(anyhow_no_ptr_addr_of)]
    pub fn new(ptr: &'a mut T) -> Self {
        Mut {
            ptr: NonNull::from(ptr),
            lifetime: PhantomData,
        }
    }

    pub fn cast<U: CastTo>(self) -> Mut<'a, U::Target> {
        Mut {
            ptr: self.ptr.cast(),
            lifetime: PhantomData,
        }
    }

    #[cfg(not(anyhow_no_ptr_addr_of))]
    pub fn by_ref(self) -> Ref<'a, T> {
        Ref {
            ptr: self.ptr,
            lifetime: PhantomData,
        }
    }

    pub fn extend<'b>(self) -> Mut<'b, T> {
        Mut {
            ptr: self.ptr,
            lifetime: PhantomData,
        }
    }

    pub unsafe fn deref_mut(self) -> &'a mut T {
        unsafe { &mut *self.ptr.as_ptr() }
    }
}

impl<'a, T> Mut<'a, T> {
    pub unsafe fn read(self) -> T {
        unsafe { self.ptr.as_ptr().read() }
    }
}

// Force turbofish on all calls of `.cast::<U>()`.
pub trait CastTo {
    type Target;
}

impl<T> CastTo for T {
    type Target = T;
}
