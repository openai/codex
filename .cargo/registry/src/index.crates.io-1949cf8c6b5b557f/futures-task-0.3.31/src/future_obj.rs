use core::{
    fmt,
    future::Future,
    marker::PhantomData,
    mem,
    pin::Pin,
    task::{Context, Poll},
};

/// A custom trait object for polling futures, roughly akin to
/// `Box<dyn Future<Output = T> + 'a>`.
///
/// This custom trait object was introduced as currently it is not possible to
/// take `dyn Trait` by value and `Box<dyn Trait>` is not available in no_std
/// contexts.
pub struct LocalFutureObj<'a, T> {
    future: *mut (dyn Future<Output = T> + 'static),
    drop_fn: unsafe fn(*mut (dyn Future<Output = T> + 'static)),
    _marker: PhantomData<&'a ()>,
}

// As LocalFutureObj only holds pointers, even if we move it, the pointed to values won't move,
// so this is safe as long as we don't provide any way for a user to directly access the pointers
// and move their values.
impl<T> Unpin for LocalFutureObj<'_, T> {}

#[allow(single_use_lifetimes)]
#[allow(clippy::transmute_ptr_to_ptr)]
unsafe fn remove_future_lifetime<'a, T>(
    ptr: *mut (dyn Future<Output = T> + 'a),
) -> *mut (dyn Future<Output = T> + 'static) {
    unsafe { mem::transmute(ptr) }
}

#[allow(single_use_lifetimes)]
unsafe fn remove_drop_lifetime<'a, T>(
    ptr: unsafe fn(*mut (dyn Future<Output = T> + 'a)),
) -> unsafe fn(*mut (dyn Future<Output = T> + 'static)) {
    unsafe { mem::transmute(ptr) }
}

impl<'a, T> LocalFutureObj<'a, T> {
    /// Create a `LocalFutureObj` from a custom trait object representation.
    #[inline]
    pub fn new<F: UnsafeFutureObj<'a, T> + 'a>(f: F) -> Self {
        Self {
            future: unsafe { remove_future_lifetime(f.into_raw()) },
            drop_fn: unsafe { remove_drop_lifetime(F::drop) },
            _marker: PhantomData,
        }
    }

    /// Converts the `LocalFutureObj` into a `FutureObj`.
    ///
    /// # Safety
    ///
    /// To make this operation safe one has to ensure that the `UnsafeFutureObj`
    /// instance from which this `LocalFutureObj` was created actually
    /// implements `Send`.
    #[inline]
    pub unsafe fn into_future_obj(self) -> FutureObj<'a, T> {
        FutureObj(self)
    }
}

impl<T> fmt::Debug for LocalFutureObj<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LocalFutureObj").finish()
    }
}

impl<'a, T> From<FutureObj<'a, T>> for LocalFutureObj<'a, T> {
    #[inline]
    fn from(f: FutureObj<'a, T>) -> Self {
        f.0
    }
}

impl<T> Future for LocalFutureObj<'_, T> {
    type Output = T;

    #[inline]
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<T> {
        unsafe { Pin::new_unchecked(&mut *self.future).poll(cx) }
    }
}

impl<T> Drop for LocalFutureObj<'_, T> {
    fn drop(&mut self) {
        unsafe { (self.drop_fn)(self.future) }
    }
}

/// A custom trait object for polling futures, roughly akin to
/// `Box<dyn Future<Output = T> + Send + 'a>`.
///
/// This custom trait object was introduced as currently it is not possible to
/// take `dyn Trait` by value and `Box<dyn Trait>` is not available in no_std
/// contexts.
///
/// You should generally not need to use this type outside of `no_std` or when
/// implementing `Spawn`, consider using `BoxFuture` instead.
pub struct FutureObj<'a, T>(LocalFutureObj<'a, T>);

impl<T> Unpin for FutureObj<'_, T> {}
unsafe impl<T> Send for FutureObj<'_, T> {}

impl<'a, T> FutureObj<'a, T> {
    /// Create a `FutureObj` from a custom trait object representation.
    #[inline]
    pub fn new<F: UnsafeFutureObj<'a, T> + Send>(f: F) -> Self {
        Self(LocalFutureObj::new(f))
    }
}

impl<T> fmt::Debug for FutureObj<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FutureObj").finish()
    }
}

impl<T> Future for FutureObj<'_, T> {
    type Output = T;

    #[inline]
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<T> {
        Pin::new(&mut self.0).poll(cx)
    }
}

/// A custom implementation of a future trait object for `FutureObj`, providing
/// a vtable with drop support.
///
/// This custom representation is typically used only in `no_std` contexts,
/// where the default `Box`-based implementation is not available.
///
/// # Safety
///
/// See the safety notes on individual methods for what guarantees an
/// implementor must provide.
pub unsafe trait UnsafeFutureObj<'a, T>: 'a {
    /// Convert an owned instance into a (conceptually owned) fat pointer.
    ///
    /// # Safety
    ///
    /// ## Implementor
    ///
    /// The trait implementor must guarantee that it is safe to convert the
    /// provided `*mut (dyn Future<Output = T> + 'a)` into a `Pin<&mut (dyn
    /// Future<Output = T> + 'a)>` and call methods on it, non-reentrantly,
    /// until `UnsafeFutureObj::drop` is called with it.
    #[allow(clippy::unnecessary_safety_doc)]
    fn into_raw(self) -> *mut (dyn Future<Output = T> + 'a);

    /// Drops the future represented by the given fat pointer.
    ///
    /// # Safety
    ///
    /// ## Implementor
    ///
    /// The trait implementor must guarantee that it is safe to call this
    /// function once per `into_raw` invocation.
    ///
    /// ## Caller
    ///
    /// The caller must ensure:
    ///
    ///  * the pointer passed was obtained from an `into_raw` invocation from
    ///    this same trait object
    ///  * the pointer is not currently in use as a `Pin<&mut (dyn Future<Output
    ///    = T> + 'a)>`
    ///  * the pointer must not be used again after this function is called
    unsafe fn drop(ptr: *mut (dyn Future<Output = T> + 'a));
}

unsafe impl<'a, T, F> UnsafeFutureObj<'a, T> for &'a mut F
where
    F: Future<Output = T> + Unpin + 'a,
{
    fn into_raw(self) -> *mut (dyn Future<Output = T> + 'a) {
        self as *mut dyn Future<Output = T>
    }

    unsafe fn drop(_ptr: *mut (dyn Future<Output = T> + 'a)) {}
}

unsafe impl<'a, T> UnsafeFutureObj<'a, T> for &'a mut (dyn Future<Output = T> + Unpin + 'a) {
    fn into_raw(self) -> *mut (dyn Future<Output = T> + 'a) {
        self as *mut dyn Future<Output = T>
    }

    unsafe fn drop(_ptr: *mut (dyn Future<Output = T> + 'a)) {}
}

unsafe impl<'a, T, F> UnsafeFutureObj<'a, T> for Pin<&'a mut F>
where
    F: Future<Output = T> + 'a,
{
    fn into_raw(self) -> *mut (dyn Future<Output = T> + 'a) {
        unsafe { self.get_unchecked_mut() as *mut dyn Future<Output = T> }
    }

    unsafe fn drop(_ptr: *mut (dyn Future<Output = T> + 'a)) {}
}

unsafe impl<'a, T> UnsafeFutureObj<'a, T> for Pin<&'a mut (dyn Future<Output = T> + 'a)> {
    fn into_raw(self) -> *mut (dyn Future<Output = T> + 'a) {
        unsafe { self.get_unchecked_mut() as *mut dyn Future<Output = T> }
    }

    unsafe fn drop(_ptr: *mut (dyn Future<Output = T> + 'a)) {}
}

#[cfg(feature = "alloc")]
mod if_alloc {
    use super::*;
    use alloc::boxed::Box;

    unsafe impl<'a, T, F> UnsafeFutureObj<'a, T> for Box<F>
    where
        F: Future<Output = T> + 'a,
    {
        fn into_raw(self) -> *mut (dyn Future<Output = T> + 'a) {
            Box::into_raw(self)
        }

        unsafe fn drop(ptr: *mut (dyn Future<Output = T> + 'a)) {
            drop(unsafe { Box::from_raw(ptr.cast::<F>()) })
        }
    }

    unsafe impl<'a, T: 'a> UnsafeFutureObj<'a, T> for Box<dyn Future<Output = T> + 'a> {
        fn into_raw(self) -> *mut (dyn Future<Output = T> + 'a) {
            Box::into_raw(self)
        }

        unsafe fn drop(ptr: *mut (dyn Future<Output = T> + 'a)) {
            drop(unsafe { Box::from_raw(ptr) })
        }
    }

    unsafe impl<'a, T: 'a> UnsafeFutureObj<'a, T> for Box<dyn Future<Output = T> + Send + 'a> {
        fn into_raw(self) -> *mut (dyn Future<Output = T> + 'a) {
            Box::into_raw(self)
        }

        unsafe fn drop(ptr: *mut (dyn Future<Output = T> + 'a)) {
            drop(unsafe { Box::from_raw(ptr) })
        }
    }

    unsafe impl<'a, T, F> UnsafeFutureObj<'a, T> for Pin<Box<F>>
    where
        F: Future<Output = T> + 'a,
    {
        fn into_raw(self) -> *mut (dyn Future<Output = T> + 'a) {
            let mut this = mem::ManuallyDrop::new(self);
            unsafe { this.as_mut().get_unchecked_mut() as *mut _ }
        }

        unsafe fn drop(ptr: *mut (dyn Future<Output = T> + 'a)) {
            drop(Pin::from(unsafe { Box::from_raw(ptr) }))
        }
    }

    unsafe impl<'a, T: 'a> UnsafeFutureObj<'a, T> for Pin<Box<dyn Future<Output = T> + 'a>> {
        fn into_raw(self) -> *mut (dyn Future<Output = T> + 'a) {
            let mut this = mem::ManuallyDrop::new(self);
            unsafe { this.as_mut().get_unchecked_mut() as *mut _ }
        }

        unsafe fn drop(ptr: *mut (dyn Future<Output = T> + 'a)) {
            drop(Pin::from(unsafe { Box::from_raw(ptr) }))
        }
    }

    unsafe impl<'a, T: 'a> UnsafeFutureObj<'a, T> for Pin<Box<dyn Future<Output = T> + Send + 'a>> {
        fn into_raw(self) -> *mut (dyn Future<Output = T> + 'a) {
            let mut this = mem::ManuallyDrop::new(self);
            unsafe { this.as_mut().get_unchecked_mut() as *mut _ }
        }

        unsafe fn drop(ptr: *mut (dyn Future<Output = T> + 'a)) {
            drop(Pin::from(unsafe { Box::from_raw(ptr) }))
        }
    }

    impl<'a, F: Future<Output = ()> + Send + 'a> From<Box<F>> for FutureObj<'a, ()> {
        fn from(boxed: Box<F>) -> Self {
            Self::new(boxed)
        }
    }

    impl<'a> From<Box<dyn Future<Output = ()> + Send + 'a>> for FutureObj<'a, ()> {
        fn from(boxed: Box<dyn Future<Output = ()> + Send + 'a>) -> Self {
            Self::new(boxed)
        }
    }

    impl<'a, F: Future<Output = ()> + Send + 'a> From<Pin<Box<F>>> for FutureObj<'a, ()> {
        fn from(boxed: Pin<Box<F>>) -> Self {
            Self::new(boxed)
        }
    }

    impl<'a> From<Pin<Box<dyn Future<Output = ()> + Send + 'a>>> for FutureObj<'a, ()> {
        fn from(boxed: Pin<Box<dyn Future<Output = ()> + Send + 'a>>) -> Self {
            Self::new(boxed)
        }
    }

    impl<'a, F: Future<Output = ()> + 'a> From<Box<F>> for LocalFutureObj<'a, ()> {
        fn from(boxed: Box<F>) -> Self {
            Self::new(boxed)
        }
    }

    impl<'a> From<Box<dyn Future<Output = ()> + 'a>> for LocalFutureObj<'a, ()> {
        fn from(boxed: Box<dyn Future<Output = ()> + 'a>) -> Self {
            Self::new(boxed)
        }
    }

    impl<'a, F: Future<Output = ()> + 'a> From<Pin<Box<F>>> for LocalFutureObj<'a, ()> {
        fn from(boxed: Pin<Box<F>>) -> Self {
            Self::new(boxed)
        }
    }

    impl<'a> From<Pin<Box<dyn Future<Output = ()> + 'a>>> for LocalFutureObj<'a, ()> {
        fn from(boxed: Pin<Box<dyn Future<Output = ()> + 'a>>) -> Self {
            Self::new(boxed)
        }
    }
}
