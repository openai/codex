use alloc::sync::Arc;

/// A way of waking up a specific task.
///
/// By implementing this trait, types that are expected to be wrapped in an `Arc`
/// can be converted into [`Waker`] objects.
/// Those Wakers can be used to signal executors that a task it owns
/// is ready to be `poll`ed again.
///
/// Currently, there are two ways to convert `ArcWake` into [`Waker`]:
///
/// * [`waker`](super::waker()) converts `Arc<impl ArcWake>` into [`Waker`].
/// * [`waker_ref`](super::waker_ref()) converts `&Arc<impl ArcWake>` into [`WakerRef`] that
///   provides access to a [`&Waker`][`Waker`].
///
/// [`Waker`]: std::task::Waker
/// [`WakerRef`]: super::WakerRef
// Note: Send + Sync required because `Arc<T>` doesn't automatically imply
// those bounds, but `Waker` implements them.
pub trait ArcWake: Send + Sync {
    /// Indicates that the associated task is ready to make progress and should
    /// be `poll`ed.
    ///
    /// This function can be called from an arbitrary thread, including threads which
    /// did not create the `ArcWake` based [`Waker`].
    ///
    /// Executors generally maintain a queue of "ready" tasks; `wake` should place
    /// the associated task onto this queue.
    ///
    /// [`Waker`]: std::task::Waker
    fn wake(self: Arc<Self>) {
        Self::wake_by_ref(&self)
    }

    /// Indicates that the associated task is ready to make progress and should
    /// be `poll`ed.
    ///
    /// This function can be called from an arbitrary thread, including threads which
    /// did not create the `ArcWake` based [`Waker`].
    ///
    /// Executors generally maintain a queue of "ready" tasks; `wake_by_ref` should place
    /// the associated task onto this queue.
    ///
    /// This function is similar to [`wake`](ArcWake::wake), but must not consume the provided data
    /// pointer.
    ///
    /// [`Waker`]: std::task::Waker
    fn wake_by_ref(arc_self: &Arc<Self>);
}
