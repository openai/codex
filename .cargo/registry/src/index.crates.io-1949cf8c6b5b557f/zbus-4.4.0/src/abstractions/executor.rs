#[cfg(not(feature = "tokio"))]
use async_executor::Executor as AsyncExecutor;
#[cfg(not(feature = "tokio"))]
use async_task::Task as AsyncTask;
#[cfg(not(feature = "tokio"))]
use std::sync::Arc;
#[cfg(feature = "tokio")]
use std::{future::pending, marker::PhantomData};
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
#[cfg(feature = "tokio")]
use tokio::task::JoinHandle;

/// A wrapper around the underlying runtime/executor.
///
/// This is used to run asynchronous tasks internally and allows integration with various runtimes.
/// See [`crate::Connection::executor`] for an example of integration with external runtimes.
///
/// **Note:** You can (and should) completely ignore this type when building with `tokio` feature
/// enabled.
#[cfg(not(feature = "tokio"))]
#[derive(Debug, Clone)]
pub struct Executor<'a> {
    executor: Arc<AsyncExecutor<'a>>,
}
#[cfg(feature = "tokio")]
#[derive(Debug, Clone)]
pub struct Executor<'a> {
    phantom: PhantomData<&'a ()>,
}

impl<'a> Executor<'a> {
    /// Spawns a task onto the executor.
    #[doc(hidden)]
    pub fn spawn<T: Send + 'static>(
        &self,
        future: impl Future<Output = T> + Send + 'static,
        #[allow(unused)] name: &str,
    ) -> Task<T> {
        #[cfg(not(feature = "tokio"))]
        {
            Task(Some(self.executor.spawn(future)))
        }

        #[cfg(feature = "tokio")]
        {
            #[cfg(tokio_unstable)]
            {
                Task(Some(
                    tokio::task::Builder::new()
                        .name(name)
                        .spawn(future)
                        // SAFETY: Looking at the code, this call always returns an `Ok`.
                        .unwrap(),
                ))
            }
            #[cfg(not(tokio_unstable))]
            {
                Task(Some(tokio::task::spawn(future)))
            }
        }
    }

    /// Returns `true` if there are no unfinished tasks.
    ///
    /// With `tokio` feature enabled, this always returns `true`.
    pub fn is_empty(&self) -> bool {
        #[cfg(not(feature = "tokio"))]
        {
            self.executor.is_empty()
        }

        #[cfg(feature = "tokio")]
        true
    }

    /// Runs a single task.
    ///
    /// With `tokio` feature enabled, its a noop and never returns.
    pub async fn tick(&self) {
        #[cfg(not(feature = "tokio"))]
        {
            self.executor.tick().await
        }

        #[cfg(feature = "tokio")]
        {
            pending().await
        }
    }

    /// Create a new `Executor`.
    pub(crate) fn new() -> Self {
        #[cfg(not(feature = "tokio"))]
        {
            Self {
                executor: Arc::new(AsyncExecutor::new()),
            }
        }

        #[cfg(feature = "tokio")]
        {
            Self {
                phantom: PhantomData,
            }
        }
    }

    /// Runs the executor until the given future completes.
    ///
    /// With `tokio` feature enabled, it just awaits on the `future`.
    pub(crate) async fn run<T>(&self, future: impl Future<Output = T>) -> T {
        #[cfg(not(feature = "tokio"))]
        {
            self.executor.run(future).await
        }
        #[cfg(feature = "tokio")]
        {
            future.await
        }
    }
}

/// A wrapper around the task API of the underlying runtime/executor.
///
/// This follows the semantics of `async_task::Task` on drop:
///
/// * it will be cancelled, rather than detached. For detaching, use the `detach` method.
/// * errors from the task cancellation will will be ignored. If you need to know about task errors,
///   convert the task to a `FallibleTask` using the `fallible` method.
#[cfg(not(feature = "tokio"))]
#[doc(hidden)]
#[derive(Debug)]
pub struct Task<T>(Option<AsyncTask<T>>);
#[cfg(feature = "tokio")]
#[doc(hidden)]
#[derive(Debug)]
pub struct Task<T>(Option<JoinHandle<T>>);

impl<T> Task<T> {
    /// Detaches the task to let it keep running in the background.
    #[allow(unused_mut)]
    #[allow(unused)]
    pub fn detach(mut self) {
        #[cfg(not(feature = "tokio"))]
        {
            self.0.take().expect("async_task::Task is none").detach()
        }

        #[cfg(feature = "tokio")]
        {
            self.0.take().expect("tokio::task::JoinHandle is none");
        }
    }
}

impl<T> Task<T>
where
    T: Send + 'static,
{
    /// Launch the given blocking function in a task.
    #[allow(unused)]
    pub(crate) fn spawn_blocking<F>(f: F, #[allow(unused)] name: &str) -> Self
    where
        F: FnOnce() -> T + Send + 'static,
    {
        #[cfg(not(feature = "tokio"))]
        {
            Self(Some(blocking::unblock(f)))
        }

        #[cfg(feature = "tokio")]
        {
            #[cfg(tokio_unstable)]
            {
                Self(Some(
                    tokio::task::Builder::new()
                        .name(name)
                        .spawn_blocking(f)
                        // SAFETY: Looking at the code, this call always returns an `Ok`.
                        .unwrap(),
                ))
            }
            #[cfg(not(tokio_unstable))]
            {
                Self(Some(tokio::task::spawn_blocking(f)))
            }
        }
    }
}

impl<T> Drop for Task<T> {
    fn drop(&mut self) {
        #[cfg(feature = "tokio")]
        {
            if let Some(join_handle) = self.0.take() {
                join_handle.abort();
            }
        }
    }
}

impl<T> Future for Task<T> {
    type Output = T;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        #[cfg(not(feature = "tokio"))]
        {
            Pin::new(&mut self.get_mut().0.as_mut().expect("async_task::Task is none")).poll(cx)
        }

        #[cfg(feature = "tokio")]
        {
            Pin::new(
                &mut self
                    .get_mut()
                    .0
                    .as_mut()
                    .expect("tokio::task::JoinHandle is none"),
            )
            .poll(cx)
            .map(|r| r.expect("tokio::task::JoinHandle error"))
        }
    }
}
