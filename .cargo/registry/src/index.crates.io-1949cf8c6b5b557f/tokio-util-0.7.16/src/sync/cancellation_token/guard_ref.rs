use crate::sync::CancellationToken;

/// A wrapper for cancellation token which automatically cancels
/// it on drop. It is created using [`drop_guard_ref`] method on the [`CancellationToken`].
///
/// This is a borrowed version of [`DropGuard`].
///
/// [`drop_guard_ref`]: CancellationToken::drop_guard_ref
/// [`DropGuard`]: super::DropGuard
#[derive(Debug)]
pub struct DropGuardRef<'a> {
    pub(super) inner: Option<&'a CancellationToken>,
}

impl<'a> DropGuardRef<'a> {
    /// Returns stored cancellation token and removes this drop guard instance
    /// (i.e. it will no longer cancel token). Other guards for this token
    /// are not affected.
    pub fn disarm(mut self) -> &'a CancellationToken {
        self.inner
            .take()
            .expect("`inner` can be only None in a destructor")
    }
}

impl Drop for DropGuardRef<'_> {
    fn drop(&mut self) {
        if let Some(inner) = self.inner {
            inner.cancel();
        }
    }
}
