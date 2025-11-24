//! A collection of tasks spawned on a Tokio runtime.
//!
//! This module provides the [`JoinSet`] type, a collection which stores a set
//! of spawned tasks and allows asynchronously awaiting the output of those
//! tasks as they complete. See the documentation for the [`JoinSet`] type for
//! details.
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::{fmt, panic};

use crate::runtime::Handle;
use crate::task::Id;
use crate::task::{unconstrained, AbortHandle, JoinError, JoinHandle, LocalSet};
use crate::util::IdleNotifiedSet;

/// A collection of tasks spawned on a Tokio runtime.
///
/// A `JoinSet` can be used to await the completion of some or all of the tasks
/// in the set. The set is not ordered, and the tasks will be returned in the
/// order they complete.
///
/// All of the tasks must have the same return type `T`.
///
/// When the `JoinSet` is dropped, all tasks in the `JoinSet` are immediately aborted.
///
/// # Examples
///
/// Spawn multiple tasks and wait for them.
///
/// ```
/// use tokio::task::JoinSet;
///
/// #[tokio::main]
/// async fn main() {
///     let mut set = JoinSet::new();
///
///     for i in 0..10 {
///         set.spawn(async move { i });
///     }
///
///     let mut seen = [false; 10];
///     while let Some(res) = set.join_next().await {
///         let idx = res.unwrap();
///         seen[idx] = true;
///     }
///
///     for i in 0..10 {
///         assert!(seen[i]);
///     }
/// }
/// ```
#[cfg_attr(docsrs, doc(cfg(feature = "rt")))]
pub struct JoinSet<T> {
    inner: IdleNotifiedSet<JoinHandle<T>>,
}

/// A variant of [`task::Builder`] that spawns tasks on a [`JoinSet`] rather
/// than on the current default runtime.
///
/// [`task::Builder`]: crate::task::Builder
#[cfg(all(tokio_unstable, feature = "tracing"))]
#[cfg_attr(docsrs, doc(cfg(all(tokio_unstable, feature = "tracing"))))]
#[must_use = "builders do nothing unless used to spawn a task"]
pub struct Builder<'a, T> {
    joinset: &'a mut JoinSet<T>,
    builder: super::Builder<'a>,
}

impl<T> JoinSet<T> {
    /// Create a new `JoinSet`.
    pub fn new() -> Self {
        Self {
            inner: IdleNotifiedSet::new(),
        }
    }

    /// Returns the number of tasks currently in the `JoinSet`.
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Returns whether the `JoinSet` is empty.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

impl<T: 'static> JoinSet<T> {
    /// Returns a [`Builder`] that can be used to configure a task prior to
    /// spawning it on this `JoinSet`.
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::task::JoinSet;
    ///
    /// #[tokio::main]
    /// async fn main() -> std::io::Result<()> {
    ///     let mut set = JoinSet::new();
    ///
    ///     // Use the builder to configure a task's name before spawning it.
    ///     set.build_task()
    ///         .name("my_task")
    ///         .spawn(async { /* ... */ })?;
    ///
    ///     Ok(())
    /// }
    /// ```
    #[cfg(all(tokio_unstable, feature = "tracing"))]
    #[cfg_attr(docsrs, doc(cfg(all(tokio_unstable, feature = "tracing"))))]
    pub fn build_task(&mut self) -> Builder<'_, T> {
        Builder {
            builder: super::Builder::new(),
            joinset: self,
        }
    }

    /// Spawn the provided task on the `JoinSet`, returning an [`AbortHandle`]
    /// that can be used to remotely cancel the task.
    ///
    /// The provided future will start running in the background immediately
    /// when this method is called, even if you don't await anything on this
    /// `JoinSet`.
    ///
    /// # Panics
    ///
    /// This method panics if called outside of a Tokio runtime.
    ///
    /// [`AbortHandle`]: crate::task::AbortHandle
    #[track_caller]
    pub fn spawn<F>(&mut self, task: F) -> AbortHandle
    where
        F: Future<Output = T>,
        F: Send + 'static,
        T: Send,
    {
        self.insert(crate::spawn(task))
    }

    /// Spawn the provided task on the provided runtime and store it in this
    /// `JoinSet` returning an [`AbortHandle`] that can be used to remotely
    /// cancel the task.
    ///
    /// The provided future will start running in the background immediately
    /// when this method is called, even if you don't await anything on this
    /// `JoinSet`.
    ///
    /// [`AbortHandle`]: crate::task::AbortHandle
    #[track_caller]
    pub fn spawn_on<F>(&mut self, task: F, handle: &Handle) -> AbortHandle
    where
        F: Future<Output = T>,
        F: Send + 'static,
        T: Send,
    {
        self.insert(handle.spawn(task))
    }

    /// Spawn the provided task on the current [`LocalSet`] and store it in this
    /// `JoinSet`, returning an [`AbortHandle`] that can be used to remotely
    /// cancel the task.
    ///
    /// The provided future will start running in the background immediately
    /// when this method is called, even if you don't await anything on this
    /// `JoinSet`.
    ///
    /// # Panics
    ///
    /// This method panics if it is called outside of a `LocalSet`.
    ///
    /// [`LocalSet`]: crate::task::LocalSet
    /// [`AbortHandle`]: crate::task::AbortHandle
    #[track_caller]
    pub fn spawn_local<F>(&mut self, task: F) -> AbortHandle
    where
        F: Future<Output = T>,
        F: 'static,
    {
        self.insert(crate::task::spawn_local(task))
    }

    /// Spawn the provided task on the provided [`LocalSet`] and store it in
    /// this `JoinSet`, returning an [`AbortHandle`] that can be used to
    /// remotely cancel the task.
    ///
    /// Unlike the [`spawn_local`] method, this method may be used to spawn local
    /// tasks on a `LocalSet` that is _not_ currently running. The provided
    /// future will start running whenever the `LocalSet` is next started.
    ///
    /// [`LocalSet`]: crate::task::LocalSet
    /// [`AbortHandle`]: crate::task::AbortHandle
    /// [`spawn_local`]: Self::spawn_local
    #[track_caller]
    pub fn spawn_local_on<F>(&mut self, task: F, local_set: &LocalSet) -> AbortHandle
    where
        F: Future<Output = T>,
        F: 'static,
    {
        self.insert(local_set.spawn_local(task))
    }

    /// Spawn the blocking code on the blocking threadpool and store
    /// it in this `JoinSet`, returning an [`AbortHandle`] that can be
    /// used to remotely cancel the task.
    ///
    /// # Examples
    ///
    /// Spawn multiple blocking tasks and wait for them.
    ///
    /// ```
    /// use tokio::task::JoinSet;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let mut set = JoinSet::new();
    ///
    ///     for i in 0..10 {
    ///         set.spawn_blocking(move || { i });
    ///     }
    ///
    ///     let mut seen = [false; 10];
    ///     while let Some(res) = set.join_next().await {
    ///         let idx = res.unwrap();
    ///         seen[idx] = true;
    ///     }
    ///
    ///     for i in 0..10 {
    ///         assert!(seen[i]);
    ///     }
    /// }
    /// ```
    ///
    /// # Panics
    ///
    /// This method panics if called outside of a Tokio runtime.
    ///
    /// [`AbortHandle`]: crate::task::AbortHandle
    #[track_caller]
    pub fn spawn_blocking<F>(&mut self, f: F) -> AbortHandle
    where
        F: FnOnce() -> T,
        F: Send + 'static,
        T: Send,
    {
        self.insert(crate::runtime::spawn_blocking(f))
    }

    /// Spawn the blocking code on the blocking threadpool of the
    /// provided runtime and store it in this `JoinSet`, returning an
    /// [`AbortHandle`] that can be used to remotely cancel the task.
    ///
    /// [`AbortHandle`]: crate::task::AbortHandle
    #[track_caller]
    pub fn spawn_blocking_on<F>(&mut self, f: F, handle: &Handle) -> AbortHandle
    where
        F: FnOnce() -> T,
        F: Send + 'static,
        T: Send,
    {
        self.insert(handle.spawn_blocking(f))
    }

    fn insert(&mut self, jh: JoinHandle<T>) -> AbortHandle {
        let abort = jh.abort_handle();
        let mut entry = self.inner.insert_idle(jh);

        // Set the waker that is notified when the task completes.
        entry.with_value_and_context(|jh, ctx| jh.set_join_waker(ctx.waker()));
        abort
    }

    /// Waits until one of the tasks in the set completes and returns its output.
    ///
    /// Returns `None` if the set is empty.
    ///
    /// # Cancel Safety
    ///
    /// This method is cancel safe. If `join_next` is used as the event in a `tokio::select!`
    /// statement and some other branch completes first, it is guaranteed that no tasks were
    /// removed from this `JoinSet`.
    pub async fn join_next(&mut self) -> Option<Result<T, JoinError>> {
        std::future::poll_fn(|cx| self.poll_join_next(cx)).await
    }

    /// Waits until one of the tasks in the set completes and returns its
    /// output, along with the [task ID] of the completed task.
    ///
    /// Returns `None` if the set is empty.
    ///
    /// When this method returns an error, then the id of the task that failed can be accessed
    /// using the [`JoinError::id`] method.
    ///
    /// # Cancel Safety
    ///
    /// This method is cancel safe. If `join_next_with_id` is used as the event in a `tokio::select!`
    /// statement and some other branch completes first, it is guaranteed that no tasks were
    /// removed from this `JoinSet`.
    ///
    /// [task ID]: crate::task::Id
    /// [`JoinError::id`]: fn@crate::task::JoinError::id
    pub async fn join_next_with_id(&mut self) -> Option<Result<(Id, T), JoinError>> {
        std::future::poll_fn(|cx| self.poll_join_next_with_id(cx)).await
    }

    /// Tries to join one of the tasks in the set that has completed and return its output.
    ///
    /// Returns `None` if there are no completed tasks, or if the set is empty.
    pub fn try_join_next(&mut self) -> Option<Result<T, JoinError>> {
        // Loop over all notified `JoinHandle`s to find one that's ready, or until none are left.
        loop {
            let mut entry = self.inner.try_pop_notified()?;

            let res = entry.with_value_and_context(|jh, ctx| {
                // Since this function is not async and cannot be forced to yield, we should
                // disable budgeting when we want to check for the `JoinHandle` readiness.
                Pin::new(&mut unconstrained(jh)).poll(ctx)
            });

            if let Poll::Ready(res) = res {
                let _entry = entry.remove();

                return Some(res);
            }
        }
    }

    /// Tries to join one of the tasks in the set that has completed and return its output,
    /// along with the [task ID] of the completed task.
    ///
    /// Returns `None` if there are no completed tasks, or if the set is empty.
    ///
    /// When this method returns an error, then the id of the task that failed can be accessed
    /// using the [`JoinError::id`] method.
    ///
    /// [task ID]: crate::task::Id
    /// [`JoinError::id`]: fn@crate::task::JoinError::id
    pub fn try_join_next_with_id(&mut self) -> Option<Result<(Id, T), JoinError>> {
        // Loop over all notified `JoinHandle`s to find one that's ready, or until none are left.
        loop {
            let mut entry = self.inner.try_pop_notified()?;

            let res = entry.with_value_and_context(|jh, ctx| {
                // Since this function is not async and cannot be forced to yield, we should
                // disable budgeting when we want to check for the `JoinHandle` readiness.
                Pin::new(&mut unconstrained(jh)).poll(ctx)
            });

            if let Poll::Ready(res) = res {
                let entry = entry.remove();

                return Some(res.map(|output| (entry.id(), output)));
            }
        }
    }

    /// Aborts all tasks and waits for them to finish shutting down.
    ///
    /// Calling this method is equivalent to calling [`abort_all`] and then calling [`join_next`] in
    /// a loop until it returns `None`.
    ///
    /// This method ignores any panics in the tasks shutting down. When this call returns, the
    /// `JoinSet` will be empty.
    ///
    /// [`abort_all`]: fn@Self::abort_all
    /// [`join_next`]: fn@Self::join_next
    pub async fn shutdown(&mut self) {
        self.abort_all();
        while self.join_next().await.is_some() {}
    }

    /// Awaits the completion of all tasks in this `JoinSet`, returning a vector of their results.
    ///
    /// The results will be stored in the order they completed not the order they were spawned.
    /// This is a convenience method that is equivalent to calling [`join_next`] in
    /// a loop. If any tasks on the `JoinSet` fail with an [`JoinError`], then this call
    /// to `join_all` will panic and all remaining tasks on the `JoinSet` are
    /// cancelled. To handle errors in any other way, manually call [`join_next`]
    /// in a loop.
    ///
    /// # Examples
    ///
    /// Spawn multiple tasks and `join_all` them.
    ///
    /// ```
    /// use tokio::task::JoinSet;
    /// use std::time::Duration;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let mut set = JoinSet::new();
    ///
    ///     for i in 0..3 {
    ///        set.spawn(async move {
    ///            tokio::time::sleep(Duration::from_secs(3 - i)).await;
    ///            i
    ///        });
    ///     }
    ///
    ///     let output = set.join_all().await;
    ///     assert_eq!(output, vec![2, 1, 0]);
    /// }
    /// ```
    ///
    /// Equivalent implementation of `join_all`, using [`join_next`] and loop.
    ///
    /// ```
    /// use tokio::task::JoinSet;
    /// use std::panic;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let mut set = JoinSet::new();
    ///
    ///     for i in 0..3 {
    ///        set.spawn(async move {i});
    ///     }
    ///
    ///     let mut output = Vec::new();
    ///     while let Some(res) = set.join_next().await{
    ///         match res {
    ///             Ok(t) => output.push(t),
    ///             Err(err) if err.is_panic() => panic::resume_unwind(err.into_panic()),
    ///             Err(err) => panic!("{err}"),
    ///         }
    ///     }
    ///     assert_eq!(output.len(),3);
    /// }
    /// ```
    /// [`join_next`]: fn@Self::join_next
    /// [`JoinError::id`]: fn@crate::task::JoinError::id
    pub async fn join_all(mut self) -> Vec<T> {
        let mut output = Vec::with_capacity(self.len());

        while let Some(res) = self.join_next().await {
            match res {
                Ok(t) => output.push(t),
                Err(err) if err.is_panic() => panic::resume_unwind(err.into_panic()),
                Err(err) => panic!("{err}"),
            }
        }
        output
    }

    /// Aborts all tasks on this `JoinSet`.
    ///
    /// This does not remove the tasks from the `JoinSet`. To wait for the tasks to complete
    /// cancellation, you should call `join_next` in a loop until the `JoinSet` is empty.
    pub fn abort_all(&mut self) {
        self.inner.for_each(|jh| jh.abort());
    }

    /// Removes all tasks from this `JoinSet` without aborting them.
    ///
    /// The tasks removed by this call will continue to run in the background even if the `JoinSet`
    /// is dropped.
    pub fn detach_all(&mut self) {
        self.inner.drain(drop);
    }

    /// Polls for one of the tasks in the set to complete.
    ///
    /// If this returns `Poll::Ready(Some(_))`, then the task that completed is removed from the set.
    ///
    /// When the method returns `Poll::Pending`, the `Waker` in the provided `Context` is scheduled
    /// to receive a wakeup when a task in the `JoinSet` completes. Note that on multiple calls to
    /// `poll_join_next`, only the `Waker` from the `Context` passed to the most recent call is
    /// scheduled to receive a wakeup.
    ///
    /// # Returns
    ///
    /// This function returns:
    ///
    ///  * `Poll::Pending` if the `JoinSet` is not empty but there is no task whose output is
    ///    available right now.
    ///  * `Poll::Ready(Some(Ok(value)))` if one of the tasks in this `JoinSet` has completed.
    ///    The `value` is the return value of one of the tasks that completed.
    ///  * `Poll::Ready(Some(Err(err)))` if one of the tasks in this `JoinSet` has panicked or been
    ///    aborted. The `err` is the `JoinError` from the panicked/aborted task.
    ///  * `Poll::Ready(None)` if the `JoinSet` is empty.
    ///
    /// Note that this method may return `Poll::Pending` even if one of the tasks has completed.
    /// This can happen if the [coop budget] is reached.
    ///
    /// [coop budget]: crate::task::coop#cooperative-scheduling
    pub fn poll_join_next(&mut self, cx: &mut Context<'_>) -> Poll<Option<Result<T, JoinError>>> {
        // The call to `pop_notified` moves the entry to the `idle` list. It is moved back to
        // the `notified` list if the waker is notified in the `poll` call below.
        let mut entry = match self.inner.pop_notified(cx.waker()) {
            Some(entry) => entry,
            None => {
                if self.is_empty() {
                    return Poll::Ready(None);
                } else {
                    // The waker was set by `pop_notified`.
                    return Poll::Pending;
                }
            }
        };

        let res = entry.with_value_and_context(|jh, ctx| Pin::new(jh).poll(ctx));

        if let Poll::Ready(res) = res {
            let _entry = entry.remove();
            Poll::Ready(Some(res))
        } else {
            // A JoinHandle generally won't emit a wakeup without being ready unless
            // the coop limit has been reached. We yield to the executor in this
            // case.
            cx.waker().wake_by_ref();
            Poll::Pending
        }
    }

    /// Polls for one of the tasks in the set to complete.
    ///
    /// If this returns `Poll::Ready(Some(_))`, then the task that completed is removed from the set.
    ///
    /// When the method returns `Poll::Pending`, the `Waker` in the provided `Context` is scheduled
    /// to receive a wakeup when a task in the `JoinSet` completes. Note that on multiple calls to
    /// `poll_join_next`, only the `Waker` from the `Context` passed to the most recent call is
    /// scheduled to receive a wakeup.
    ///
    /// # Returns
    ///
    /// This function returns:
    ///
    ///  * `Poll::Pending` if the `JoinSet` is not empty but there is no task whose output is
    ///    available right now.
    ///  * `Poll::Ready(Some(Ok((id, value))))` if one of the tasks in this `JoinSet` has completed.
    ///    The `value` is the return value of one of the tasks that completed, and
    ///    `id` is the [task ID] of that task.
    ///  * `Poll::Ready(Some(Err(err)))` if one of the tasks in this `JoinSet` has panicked or been
    ///    aborted. The `err` is the `JoinError` from the panicked/aborted task.
    ///  * `Poll::Ready(None)` if the `JoinSet` is empty.
    ///
    /// Note that this method may return `Poll::Pending` even if one of the tasks has completed.
    /// This can happen if the [coop budget] is reached.
    ///
    /// [coop budget]: crate::task::coop#cooperative-scheduling
    /// [task ID]: crate::task::Id
    pub fn poll_join_next_with_id(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<(Id, T), JoinError>>> {
        // The call to `pop_notified` moves the entry to the `idle` list. It is moved back to
        // the `notified` list if the waker is notified in the `poll` call below.
        let mut entry = match self.inner.pop_notified(cx.waker()) {
            Some(entry) => entry,
            None => {
                if self.is_empty() {
                    return Poll::Ready(None);
                } else {
                    // The waker was set by `pop_notified`.
                    return Poll::Pending;
                }
            }
        };

        let res = entry.with_value_and_context(|jh, ctx| Pin::new(jh).poll(ctx));

        if let Poll::Ready(res) = res {
            let entry = entry.remove();
            // If the task succeeded, add the task ID to the output. Otherwise, the
            // `JoinError` will already have the task's ID.
            Poll::Ready(Some(res.map(|output| (entry.id(), output))))
        } else {
            // A JoinHandle generally won't emit a wakeup without being ready unless
            // the coop limit has been reached. We yield to the executor in this
            // case.
            cx.waker().wake_by_ref();
            Poll::Pending
        }
    }
}

impl<T> Drop for JoinSet<T> {
    fn drop(&mut self) {
        self.inner.drain(|join_handle| join_handle.abort());
    }
}

impl<T> fmt::Debug for JoinSet<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("JoinSet").field("len", &self.len()).finish()
    }
}

impl<T> Default for JoinSet<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// Collect an iterator of futures into a [`JoinSet`].
///
/// This is equivalent to calling [`JoinSet::spawn`] on each element of the iterator.
///
/// # Examples
///
/// The main example from [`JoinSet`]'s documentation can also be written using [`collect`]:
///
/// ```
/// use tokio::task::JoinSet;
///
/// #[tokio::main]
/// async fn main() {
///     let mut set: JoinSet<_> = (0..10).map(|i| async move { i }).collect();
///
///     let mut seen = [false; 10];
///     while let Some(res) = set.join_next().await {
///         let idx = res.unwrap();
///         seen[idx] = true;
///     }
///
///     for i in 0..10 {
///         assert!(seen[i]);
///     }
/// }
/// ```
///
/// [`collect`]: std::iter::Iterator::collect
impl<T, F> std::iter::FromIterator<F> for JoinSet<T>
where
    F: Future<Output = T>,
    F: Send + 'static,
    T: Send + 'static,
{
    fn from_iter<I: IntoIterator<Item = F>>(iter: I) -> Self {
        let mut set = Self::new();
        iter.into_iter().for_each(|task| {
            set.spawn(task);
        });
        set
    }
}

// === impl Builder ===

#[cfg(all(tokio_unstable, feature = "tracing"))]
#[cfg_attr(docsrs, doc(cfg(all(tokio_unstable, feature = "tracing"))))]
impl<'a, T: 'static> Builder<'a, T> {
    /// Assigns a name to the task which will be spawned.
    pub fn name(self, name: &'a str) -> Self {
        let builder = self.builder.name(name);
        Self { builder, ..self }
    }

    /// Spawn the provided task with this builder's settings and store it in the
    /// [`JoinSet`], returning an [`AbortHandle`] that can be used to remotely
    /// cancel the task.
    ///
    /// # Returns
    ///
    /// An [`AbortHandle`] that can be used to remotely cancel the task.
    ///
    /// # Panics
    ///
    /// This method panics if called outside of a Tokio runtime.
    ///
    /// [`AbortHandle`]: crate::task::AbortHandle
    #[track_caller]
    pub fn spawn<F>(self, future: F) -> std::io::Result<AbortHandle>
    where
        F: Future<Output = T>,
        F: Send + 'static,
        T: Send,
    {
        Ok(self.joinset.insert(self.builder.spawn(future)?))
    }

    /// Spawn the provided task on the provided [runtime handle] with this
    /// builder's settings, and store it in the [`JoinSet`].
    ///
    /// # Returns
    ///
    /// An [`AbortHandle`] that can be used to remotely cancel the task.
    ///
    ///
    /// [`AbortHandle`]: crate::task::AbortHandle
    /// [runtime handle]: crate::runtime::Handle
    #[track_caller]
    pub fn spawn_on<F>(self, future: F, handle: &Handle) -> std::io::Result<AbortHandle>
    where
        F: Future<Output = T>,
        F: Send + 'static,
        T: Send,
    {
        Ok(self.joinset.insert(self.builder.spawn_on(future, handle)?))
    }

    /// Spawn the blocking code on the blocking threadpool with this builder's
    /// settings, and store it in the [`JoinSet`].
    ///
    /// # Returns
    ///
    /// An [`AbortHandle`] that can be used to remotely cancel the task.
    ///
    /// # Panics
    ///
    /// This method panics if called outside of a Tokio runtime.
    ///
    /// [`JoinSet`]: crate::task::JoinSet
    /// [`AbortHandle`]: crate::task::AbortHandle
    #[track_caller]
    pub fn spawn_blocking<F>(self, f: F) -> std::io::Result<AbortHandle>
    where
        F: FnOnce() -> T,
        F: Send + 'static,
        T: Send,
    {
        Ok(self.joinset.insert(self.builder.spawn_blocking(f)?))
    }

    /// Spawn the blocking code on the blocking threadpool of the provided
    /// runtime handle with this builder's settings, and store it in the
    /// [`JoinSet`].
    ///
    /// # Returns
    ///
    /// An [`AbortHandle`] that can be used to remotely cancel the task.
    ///
    /// [`JoinSet`]: crate::task::JoinSet
    /// [`AbortHandle`]: crate::task::AbortHandle
    #[track_caller]
    pub fn spawn_blocking_on<F>(self, f: F, handle: &Handle) -> std::io::Result<AbortHandle>
    where
        F: FnOnce() -> T,
        F: Send + 'static,
        T: Send,
    {
        Ok(self
            .joinset
            .insert(self.builder.spawn_blocking_on(f, handle)?))
    }

    /// Spawn the provided task on the current [`LocalSet`] with this builder's
    /// settings, and store it in the [`JoinSet`].
    ///
    /// # Returns
    ///
    /// An [`AbortHandle`] that can be used to remotely cancel the task.
    ///
    /// # Panics
    ///
    /// This method panics if it is called outside of a `LocalSet`.
    ///
    /// [`LocalSet`]: crate::task::LocalSet
    /// [`AbortHandle`]: crate::task::AbortHandle
    #[track_caller]
    pub fn spawn_local<F>(self, future: F) -> std::io::Result<AbortHandle>
    where
        F: Future<Output = T>,
        F: 'static,
    {
        Ok(self.joinset.insert(self.builder.spawn_local(future)?))
    }

    /// Spawn the provided task on the provided [`LocalSet`] with this builder's
    /// settings, and store it in the [`JoinSet`].
    ///
    /// # Returns
    ///
    /// An [`AbortHandle`] that can be used to remotely cancel the task.
    ///
    /// [`LocalSet`]: crate::task::LocalSet
    /// [`AbortHandle`]: crate::task::AbortHandle
    #[track_caller]
    pub fn spawn_local_on<F>(self, future: F, local_set: &LocalSet) -> std::io::Result<AbortHandle>
    where
        F: Future<Output = T>,
        F: 'static,
    {
        Ok(self
            .joinset
            .insert(self.builder.spawn_local_on(future, local_set)?))
    }
}

// Manual `Debug` impl so that `Builder` is `Debug` regardless of whether `T` is
// `Debug`.
#[cfg(all(tokio_unstable, feature = "tracing"))]
#[cfg_attr(docsrs, doc(cfg(all(tokio_unstable, feature = "tracing"))))]
impl<'a, T> fmt::Debug for Builder<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("join_set::Builder")
            .field("joinset", &self.joinset)
            .field("builder", &self.builder)
            .finish()
    }
}
