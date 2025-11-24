use super::assert_future;
use crate::future::{AbortHandle, Abortable, Aborted};
use futures_core::future::Future;

/// Creates a new `Abortable` future and an `AbortHandle` which can be used to stop it.
///
/// This function is a convenient (but less flexible) alternative to calling
/// `AbortHandle::new` and `Abortable::new` manually.
///
/// This function is only available when the `std` or `alloc` feature of this
/// library is activated, and it is activated by default.
pub fn abortable<Fut>(future: Fut) -> (Abortable<Fut>, AbortHandle)
where
    Fut: Future,
{
    let (handle, reg) = AbortHandle::new_pair();
    let abortable = assert_future::<Result<Fut::Output, Aborted>, _>(Abortable::new(future, reg));
    (abortable, handle)
}
