use super::assert_stream;
use crate::stream::{AbortHandle, Abortable};
use crate::Stream;

/// Creates a new `Abortable` stream and an `AbortHandle` which can be used to stop it.
///
/// This function is a convenient (but less flexible) alternative to calling
/// `AbortHandle::new` and `Abortable::new` manually.
///
/// This function is only available when the `std` or `alloc` feature of this
/// library is activated, and it is activated by default.
pub fn abortable<St>(stream: St) -> (Abortable<St>, AbortHandle)
where
    St: Stream,
{
    let (handle, reg) = AbortHandle::new_pair();
    let abortable = assert_stream::<St::Item, _>(Abortable::new(stream, reg));
    (abortable, handle)
}
