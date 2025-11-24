use std::alloc::Layout;
use std::fmt;
use std::future::{self, Future};
use std::mem::{self, ManuallyDrop};
use std::pin::Pin;
use std::ptr;
use std::task::{Context, Poll};

/// A reusable `Pin<Box<dyn Future<Output = T> + Send + 'a>>`.
///
/// This type lets you replace the future stored in the box without
/// reallocating when the size and alignment permits this.
pub struct ReusableBoxFuture<'a, T> {
    boxed: Pin<Box<dyn Future<Output = T> + Send + 'a>>,
}

impl<'a, T> ReusableBoxFuture<'a, T> {
    /// Create a new `ReusableBoxFuture<T>` containing the provided future.
    pub fn new<F>(future: F) -> Self
    where
        F: Future<Output = T> + Send + 'a,
    {
        Self {
            boxed: Box::pin(future),
        }
    }

    /// Replace the future currently stored in this box.
    ///
    /// This reallocates if and only if the layout of the provided future is
    /// different from the layout of the currently stored future.
    pub fn set<F>(&mut self, future: F)
    where
        F: Future<Output = T> + Send + 'a,
    {
        if let Err(future) = self.try_set(future) {
            *self = Self::new(future);
        }
    }

    /// Replace the future currently stored in this box.
    ///
    /// This function never reallocates, but returns an error if the provided
    /// future has a different size or alignment from the currently stored
    /// future.
    pub fn try_set<F>(&mut self, future: F) -> Result<(), F>
    where
        F: Future<Output = T> + Send + 'a,
    {
        // If we try to inline the contents of this function, the type checker complains because
        // the bound `T: 'a` is not satisfied in the call to `pending()`. But by putting it in an
        // inner function that doesn't have `T` as a generic parameter, we implicitly get the bound
        // `F::Output: 'a` transitively through `F: 'a`, allowing us to call `pending()`.
        #[inline(always)]
        fn real_try_set<'a, F>(
            this: &mut ReusableBoxFuture<'a, F::Output>,
            future: F,
        ) -> Result<(), F>
        where
            F: Future + Send + 'a,
        {
            // future::Pending<T> is a ZST so this never allocates.
            let boxed = mem::replace(&mut this.boxed, Box::pin(future::pending()));
            reuse_pin_box(boxed, future, |boxed| this.boxed = Pin::from(boxed))
        }

        real_try_set(self, future)
    }

    /// Get a pinned reference to the underlying future.
    pub fn get_pin(&mut self) -> Pin<&mut (dyn Future<Output = T> + Send)> {
        self.boxed.as_mut()
    }

    /// Poll the future stored inside this box.
    pub fn poll(&mut self, cx: &mut Context<'_>) -> Poll<T> {
        self.get_pin().poll(cx)
    }
}

impl<T> Future for ReusableBoxFuture<'_, T> {
    type Output = T;

    /// Poll the future stored inside this box.
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<T> {
        Pin::into_inner(self).get_pin().poll(cx)
    }
}

// The only method called on self.boxed is poll, which takes &mut self, so this
// struct being Sync does not permit any invalid access to the Future, even if
// the future is not Sync.
unsafe impl<T> Sync for ReusableBoxFuture<'_, T> {}

impl<T> fmt::Debug for ReusableBoxFuture<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ReusableBoxFuture").finish()
    }
}

fn reuse_pin_box<T: ?Sized, U, O, F>(boxed: Pin<Box<T>>, new_value: U, callback: F) -> Result<O, U>
where
    F: FnOnce(Box<U>) -> O,
{
    let layout = Layout::for_value::<T>(&*boxed);
    if layout != Layout::new::<U>() {
        return Err(new_value);
    }

    // SAFETY: We don't ever construct a non-pinned reference to the old `T` from now on, and we
    // always drop the `T`.
    let raw: *mut T = Box::into_raw(unsafe { Pin::into_inner_unchecked(boxed) });

    // When dropping the old value panics, we still want to call `callback` — so move the rest of
    // the code into a guard type.
    let guard = CallOnDrop::new(|| {
        let raw: *mut U = raw.cast::<U>();
        unsafe { raw.write(new_value) };

        // SAFETY:
        // - `T` and `U` have the same layout.
        // - `raw` comes from a `Box` that uses the same allocator as this one.
        // - `raw` points to a valid instance of `U` (we just wrote it in).
        let boxed = unsafe { Box::from_raw(raw) };

        callback(boxed)
    });

    // Drop the old value.
    unsafe { ptr::drop_in_place(raw) };

    // Run the rest of the code.
    Ok(guard.call())
}

struct CallOnDrop<O, F: FnOnce() -> O> {
    f: ManuallyDrop<F>,
}

impl<O, F: FnOnce() -> O> CallOnDrop<O, F> {
    fn new(f: F) -> Self {
        let f = ManuallyDrop::new(f);
        Self { f }
    }
    fn call(self) -> O {
        let mut this = ManuallyDrop::new(self);
        let f = unsafe { ManuallyDrop::take(&mut this.f) };
        f()
    }
}

impl<O, F: FnOnce() -> O> Drop for CallOnDrop<O, F> {
    fn drop(&mut self) {
        let f = unsafe { ManuallyDrop::take(&mut self.f) };
        f();
    }
}
