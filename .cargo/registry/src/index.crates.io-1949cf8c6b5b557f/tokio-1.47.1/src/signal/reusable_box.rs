use std::alloc::Layout;
use std::future::Future;
use std::panic::AssertUnwindSafe;
use std::pin::Pin;
use std::ptr::{self, NonNull};
use std::task::{Context, Poll};
use std::{fmt, panic};

/// A reusable `Pin<Box<dyn Future<Output = T> + Send>>`.
///
/// This type lets you replace the future stored in the box without
/// reallocating when the size and alignment permits this.
pub(crate) struct ReusableBoxFuture<T> {
    boxed: NonNull<dyn Future<Output = T> + Send>,
}

impl<T> ReusableBoxFuture<T> {
    /// Create a new `ReusableBoxFuture<T>` containing the provided future.
    pub(crate) fn new<F>(future: F) -> Self
    where
        F: Future<Output = T> + Send + 'static,
    {
        let boxed: Box<dyn Future<Output = T> + Send> = Box::new(future);

        let boxed = Box::into_raw(boxed);

        // SAFETY: Box::into_raw does not return null pointers.
        let boxed = unsafe { NonNull::new_unchecked(boxed) };

        Self { boxed }
    }

    /// Replaces the future currently stored in this box.
    ///
    /// This reallocates if and only if the layout of the provided future is
    /// different from the layout of the currently stored future.
    pub(crate) fn set<F>(&mut self, future: F)
    where
        F: Future<Output = T> + Send + 'static,
    {
        if let Err(future) = self.try_set(future) {
            *self = Self::new(future);
        }
    }

    /// Replaces the future currently stored in this box.
    ///
    /// This function never reallocates, but returns an error if the provided
    /// future has a different size or alignment from the currently stored
    /// future.
    pub(crate) fn try_set<F>(&mut self, future: F) -> Result<(), F>
    where
        F: Future<Output = T> + Send + 'static,
    {
        // SAFETY: The pointer is not dangling.
        let self_layout = {
            let dyn_future: &(dyn Future<Output = T> + Send) = unsafe { self.boxed.as_ref() };
            Layout::for_value(dyn_future)
        };

        if Layout::new::<F>() == self_layout {
            // SAFETY: We just checked that the layout of F is correct.
            unsafe {
                self.set_same_layout(future);
            }

            Ok(())
        } else {
            Err(future)
        }
    }

    /// Sets the current future.
    ///
    /// # Safety
    ///
    /// This function requires that the layout of the provided future is the
    /// same as `self.layout`.
    unsafe fn set_same_layout<F>(&mut self, future: F)
    where
        F: Future<Output = T> + Send + 'static,
    {
        // Drop the existing future, catching any panics.
        let result = panic::catch_unwind(AssertUnwindSafe(|| {
            ptr::drop_in_place(self.boxed.as_ptr());
        }));

        // Overwrite the future behind the pointer. This is safe because the
        // allocation was allocated with the same size and alignment as the type F.
        let self_ptr: *mut F = self.boxed.as_ptr() as *mut F;
        ptr::write(self_ptr, future);

        // Update the vtable of self.boxed. The pointer is not null because we
        // just got it from self.boxed, which is not null.
        self.boxed = NonNull::new_unchecked(self_ptr);

        // If the old future's destructor panicked, resume unwinding.
        match result {
            Ok(()) => {}
            Err(payload) => {
                panic::resume_unwind(payload);
            }
        }
    }

    /// Gets a pinned reference to the underlying future.
    pub(crate) fn get_pin(&mut self) -> Pin<&mut (dyn Future<Output = T> + Send)> {
        // SAFETY: The user of this box cannot move the box, and we do not move it
        // either.
        unsafe { Pin::new_unchecked(self.boxed.as_mut()) }
    }

    /// Polls the future stored inside this box.
    pub(crate) fn poll(&mut self, cx: &mut Context<'_>) -> Poll<T> {
        self.get_pin().poll(cx)
    }
}

impl<T> Future for ReusableBoxFuture<T> {
    type Output = T;

    /// Polls the future stored inside this box.
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<T> {
        Pin::into_inner(self).get_pin().poll(cx)
    }
}

// The future stored inside ReusableBoxFuture<T> must be Send.
unsafe impl<T> Send for ReusableBoxFuture<T> {}

// The only method called on self.boxed is poll, which takes &mut self, so this
// struct being Sync does not permit any invalid access to the Future, even if
// the future is not Sync.
unsafe impl<T> Sync for ReusableBoxFuture<T> {}

// Just like a Pin<Box<dyn Future>> is always Unpin, so is this type.
impl<T> Unpin for ReusableBoxFuture<T> {}

impl<T> Drop for ReusableBoxFuture<T> {
    fn drop(&mut self) {
        unsafe {
            drop(Box::from_raw(self.boxed.as_ptr()));
        }
    }
}

impl<T> fmt::Debug for ReusableBoxFuture<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ReusableBoxFuture").finish()
    }
}

#[cfg(test)]
mod test {
    use super::ReusableBoxFuture;
    use futures::future::FutureExt;
    use std::alloc::Layout;
    use std::future::Future;
    use std::pin::Pin;
    use std::task::{Context, Poll};

    #[test]
    fn test_different_futures() {
        let fut = async move { 10 };
        // Not zero sized!
        assert_eq!(Layout::for_value(&fut).size(), 1);

        let mut b = ReusableBoxFuture::new(fut);

        assert_eq!(b.get_pin().now_or_never(), Some(10));

        b.try_set(async move { 20 })
            .unwrap_or_else(|_| panic!("incorrect size"));

        assert_eq!(b.get_pin().now_or_never(), Some(20));

        b.try_set(async move { 30 })
            .unwrap_or_else(|_| panic!("incorrect size"));

        assert_eq!(b.get_pin().now_or_never(), Some(30));
    }

    #[test]
    fn test_different_sizes() {
        let fut1 = async move { 10 };
        let val = [0u32; 1000];
        let fut2 = async move { val[0] };
        let fut3 = ZeroSizedFuture {};

        assert_eq!(Layout::for_value(&fut1).size(), 1);
        assert_eq!(Layout::for_value(&fut2).size(), 4004);
        assert_eq!(Layout::for_value(&fut3).size(), 0);

        let mut b = ReusableBoxFuture::new(fut1);
        assert_eq!(b.get_pin().now_or_never(), Some(10));
        b.set(fut2);
        assert_eq!(b.get_pin().now_or_never(), Some(0));
        b.set(fut3);
        assert_eq!(b.get_pin().now_or_never(), Some(5));
    }

    struct ZeroSizedFuture {}
    impl Future for ZeroSizedFuture {
        type Output = u32;
        fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<u32> {
            Poll::Ready(5)
        }
    }

    #[test]
    fn test_zero_sized() {
        let fut = ZeroSizedFuture {};
        // Zero sized!
        assert_eq!(Layout::for_value(&fut).size(), 0);

        let mut b = ReusableBoxFuture::new(fut);

        assert_eq!(b.get_pin().now_or_never(), Some(5));
        assert_eq!(b.get_pin().now_or_never(), Some(5));

        b.try_set(ZeroSizedFuture {})
            .unwrap_or_else(|_| panic!("incorrect size"));

        assert_eq!(b.get_pin().now_or_never(), Some(5));
        assert_eq!(b.get_pin().now_or_never(), Some(5));
    }
}
