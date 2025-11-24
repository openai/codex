/*!
This module re-exports `Arc`.

That is, it provides some indirection for the case when `alloc::sync::Arc` is
unavailable.

It also defines a "dumb" `Arc` in core-only mode that doesn't actually do
anything (no indirection, no reference counting).
*/

#[cfg(all(feature = "alloc", not(target_has_atomic = "ptr")))]
pub(crate) use portable_atomic_util::Arc;

#[cfg(all(feature = "alloc", target_has_atomic = "ptr"))]
pub(crate) use alloc::sync::Arc;

/// A "fake" `Arc`.
///
/// Basically, it exposes the `Arc` APIs we use in Jiff, but doesn't
/// actually introduce indirection or reference counting. It's only used
/// in core-only mode and in effect results in inlining all data into its
/// container.
///
/// Not ideal, but we use `Arc` in very few places. One is `TimeZone`,
/// which ends up being pretty small in core-only mode since it doesn't
/// support carrying TZif data.
#[cfg(not(feature = "alloc"))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Arc<T>(T);

#[cfg(not(feature = "alloc"))]
impl<T> Arc<T> {
    pub(crate) fn new(t: T) -> Arc<T> {
        Arc(t)
    }

    pub(crate) fn get_mut(this: &mut Arc<T>) -> Option<&mut T> {
        Some(&mut this.0)
    }
}

#[cfg(not(feature = "alloc"))]
impl<T> core::ops::Deref for Arc<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0
    }
}
