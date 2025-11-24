//! Tokio context aware futures utilities.
//!
//! This module includes utilities around integrating tokio with other runtimes
//! by allowing the context to be attached to futures. This allows spawning
//! futures on other executors while still using tokio to drive them. This
//! can be useful if you need to use a tokio based library in an executor/runtime
//! that does not provide a tokio context.

use pin_project_lite::pin_project;
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
use tokio::runtime::{Handle, Runtime};

pin_project! {
    /// `TokioContext` allows running futures that must be inside Tokio's
    /// context on a non-Tokio runtime.
    ///
    /// It contains a [`Handle`] to the runtime. A handle to the runtime can be
    /// obtain by calling the [`Runtime::handle()`] method.
    ///
    /// Note that the `TokioContext` wrapper only works if the `Runtime` it is
    /// connected to has not yet been destroyed. You must keep the `Runtime`
    /// alive until the future has finished executing.
    ///
    /// **Warning:** If `TokioContext` is used together with a [current thread]
    /// runtime, that runtime must be inside a call to `block_on` for the
    /// wrapped future to work. For this reason, it is recommended to use a
    /// [multi thread] runtime, even if you configure it to only spawn one
    /// worker thread.
    ///
    /// # Examples
    ///
    /// This example creates two runtimes, but only [enables time] on one of
    /// them. It then uses the context of the runtime with the timer enabled to
    /// execute a [`sleep`] future on the runtime with timing disabled.
    /// ```
    /// use tokio::time::{sleep, Duration};
    /// use tokio_util::context::RuntimeExt;
    ///
    /// // This runtime has timers enabled.
    /// let rt = tokio::runtime::Builder::new_multi_thread()
    ///     .enable_all()
    ///     .build()
    ///     .unwrap();
    ///
    /// // This runtime has timers disabled.
    /// let rt2 = tokio::runtime::Builder::new_multi_thread()
    ///     .build()
    ///     .unwrap();
    ///
    /// // Wrap the sleep future in the context of rt.
    /// let fut = rt.wrap(async { sleep(Duration::from_millis(2)).await });
    ///
    /// // Execute the future on rt2.
    /// rt2.block_on(fut);
    /// ```
    ///
    /// [`Handle`]: struct@tokio::runtime::Handle
    /// [`Runtime::handle()`]: fn@tokio::runtime::Runtime::handle
    /// [`RuntimeExt`]: trait@crate::context::RuntimeExt
    /// [`new_static`]: fn@Self::new_static
    /// [`sleep`]: fn@tokio::time::sleep
    /// [current thread]: fn@tokio::runtime::Builder::new_current_thread
    /// [enables time]: fn@tokio::runtime::Builder::enable_time
    /// [multi thread]: fn@tokio::runtime::Builder::new_multi_thread
    pub struct TokioContext<F> {
        #[pin]
        inner: F,
        handle: Handle,
    }
}

impl<F> TokioContext<F> {
    /// Associate the provided future with the context of the runtime behind
    /// the provided `Handle`.
    ///
    /// This constructor uses a `'static` lifetime to opt-out of checking that
    /// the runtime still exists.
    ///
    /// # Examples
    ///
    /// This is the same as the example above, but uses the `new` constructor
    /// rather than [`RuntimeExt::wrap`].
    ///
    /// [`RuntimeExt::wrap`]: fn@RuntimeExt::wrap
    ///
    /// ```
    /// use tokio::time::{sleep, Duration};
    /// use tokio_util::context::TokioContext;
    ///
    /// // This runtime has timers enabled.
    /// let rt = tokio::runtime::Builder::new_multi_thread()
    ///     .enable_all()
    ///     .build()
    ///     .unwrap();
    ///
    /// // This runtime has timers disabled.
    /// let rt2 = tokio::runtime::Builder::new_multi_thread()
    ///     .build()
    ///     .unwrap();
    ///
    /// let fut = TokioContext::new(
    ///     async { sleep(Duration::from_millis(2)).await },
    ///     rt.handle().clone(),
    /// );
    ///
    /// // Execute the future on rt2.
    /// rt2.block_on(fut);
    /// ```
    pub fn new(future: F, handle: Handle) -> TokioContext<F> {
        TokioContext {
            inner: future,
            handle,
        }
    }

    /// Obtain a reference to the handle inside this `TokioContext`.
    pub fn handle(&self) -> &Handle {
        &self.handle
    }

    /// Remove the association between the Tokio runtime and the wrapped future.
    pub fn into_inner(self) -> F {
        self.inner
    }
}

impl<F: Future> Future for TokioContext<F> {
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let me = self.project();
        let handle = me.handle;
        let fut = me.inner;

        let _enter = handle.enter();
        fut.poll(cx)
    }
}

/// Extension trait that simplifies bundling a `Handle` with a `Future`.
pub trait RuntimeExt {
    /// Create a [`TokioContext`] that wraps the provided future and runs it in
    /// this runtime's context.
    ///
    /// # Examples
    ///
    /// This example creates two runtimes, but only [enables time] on one of
    /// them. It then uses the context of the runtime with the timer enabled to
    /// execute a [`sleep`] future on the runtime with timing disabled.
    ///
    /// ```
    /// use tokio::time::{sleep, Duration};
    /// use tokio_util::context::RuntimeExt;
    ///
    /// // This runtime has timers enabled.
    /// let rt = tokio::runtime::Builder::new_multi_thread()
    ///     .enable_all()
    ///     .build()
    ///     .unwrap();
    ///
    /// // This runtime has timers disabled.
    /// let rt2 = tokio::runtime::Builder::new_multi_thread()
    ///     .build()
    ///     .unwrap();
    ///
    /// // Wrap the sleep future in the context of rt.
    /// let fut = rt.wrap(async { sleep(Duration::from_millis(2)).await });
    ///
    /// // Execute the future on rt2.
    /// rt2.block_on(fut);
    /// ```
    ///
    /// [`TokioContext`]: struct@crate::context::TokioContext
    /// [`sleep`]: fn@tokio::time::sleep
    /// [enables time]: fn@tokio::runtime::Builder::enable_time
    fn wrap<F: Future>(&self, fut: F) -> TokioContext<F>;
}

impl RuntimeExt for Runtime {
    fn wrap<F: Future>(&self, fut: F) -> TokioContext<F> {
        TokioContext {
            inner: fut,
            handle: self.handle().clone(),
        }
    }
}
