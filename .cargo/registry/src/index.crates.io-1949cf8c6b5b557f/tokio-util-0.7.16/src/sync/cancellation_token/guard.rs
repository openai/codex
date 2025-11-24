use crate::sync::CancellationToken;

/// A wrapper for cancellation token which automatically cancels
/// it on drop. It is created using [`drop_guard`] method on the [`CancellationToken`].
///
/// [`drop_guard`]: CancellationToken::drop_guard
#[derive(Debug)]
pub struct DropGuard {
    pub(super) inner: Option<CancellationToken>,
}

impl DropGuard {
    /// Returns stored cancellation token and removes this drop guard instance
    /// (i.e. it will no longer cancel token). Other guards for this token
    /// are not affected.
    pub fn disarm(mut self) -> CancellationToken {
        self.inner
            .take()
            .expect("`inner` can be only None in a destructor")
    }
}

impl Drop for DropGuard {
    fn drop(&mut self) {
        if let Some(inner) = &self.inner {
            inner.cancel();
        }
    }
}
