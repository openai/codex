use crate::{
    dispatcher::{self, Dispatch},
    span::Span,
};
use core::{
    future::Future,
    marker::Sized,
    mem::ManuallyDrop,
    pin::Pin,
    task::{Context, Poll},
};
use pin_project_lite::pin_project;

/// Attaches spans to a [`std::future::Future`].
///
/// Extension trait allowing futures to be
/// instrumented with a `tracing` [span].
///
/// [span]: super::Span
pub trait Instrument: Sized {
    /// Instruments this type with the provided [`Span`], returning an
    /// `Instrumented` wrapper.
    ///
    /// The attached [`Span`] will be [entered] every time the instrumented
    /// [`Future`] is polled or [`Drop`]ped.
    ///
    /// # Examples
    ///
    /// Instrumenting a future:
    ///
    /// ```rust
    /// use tracing::Instrument;
    ///
    /// # async fn doc() {
    /// let my_future = async {
    ///     // ...
    /// };
    ///
    /// my_future
    ///     .instrument(tracing::info_span!("my_future"))
    ///     .await
    /// # }
    /// ```
    ///
    /// The [`Span::or_current`] combinator can be used in combination with
    /// `instrument` to ensure that the [current span] is attached to the
    /// future if the span passed to `instrument` is [disabled]:
    ///
    /// ```
    /// use tracing::Instrument;
    /// # mod tokio {
    /// #     pub(super) fn spawn(_: impl std::future::Future) {}
    /// # }
    ///
    /// let my_future = async {
    ///     // ...
    /// };
    ///
    /// let outer_span = tracing::info_span!("outer").entered();
    ///
    /// // If the "my_future" span is enabled, then the spawned task will
    /// // be within both "my_future" *and* "outer", since "outer" is
    /// // "my_future"'s parent. However, if "my_future" is disabled,
    /// // the spawned task will *not* be in any span.
    /// tokio::spawn(
    ///     my_future
    ///         .instrument(tracing::debug_span!("my_future"))
    /// );
    ///
    /// // Using `Span::or_current` ensures the spawned task is instrumented
    /// // with the current span, if the new span passed to `instrument` is
    /// // not enabled. This means that if the "my_future"  span is disabled,
    /// // the spawned task will still be instrumented with the "outer" span:
    /// # let my_future = async {};
    /// tokio::spawn(
    ///    my_future
    ///         .instrument(tracing::debug_span!("my_future").or_current())
    /// );
    /// ```
    ///
    /// [entered]: super::Span::enter()
    /// [`Span::or_current`]: super::Span::or_current()
    /// [current span]: super::Span::current()
    /// [disabled]: super::Span::is_disabled()
    /// [`Future`]: std::future::Future
    fn instrument(self, span: Span) -> Instrumented<Self> {
        Instrumented {
            inner: ManuallyDrop::new(self),
            span,
        }
    }

    /// Instruments this type with the [current] [`Span`], returning an
    /// `Instrumented` wrapper.
    ///
    /// The attached [`Span`] will be [entered] every time the instrumented
    /// [`Future`] is polled or [`Drop`]ped.
    ///
    /// This can be used to propagate the current span when spawning a new future.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use tracing::Instrument;
    ///
    /// # mod tokio {
    /// #     pub(super) fn spawn(_: impl std::future::Future) {}
    /// # }
    /// # async fn doc() {
    /// let span = tracing::info_span!("my_span");
    /// let _enter = span.enter();
    ///
    /// // ...
    ///
    /// let future = async {
    ///     tracing::debug!("this event will occur inside `my_span`");
    ///     // ...
    /// };
    /// tokio::spawn(future.in_current_span());
    /// # }
    /// ```
    ///
    /// [current]: super::Span::current()
    /// [entered]: super::Span::enter()
    /// [`Span`]: crate::Span
    /// [`Future`]: std::future::Future
    #[inline]
    fn in_current_span(self) -> Instrumented<Self> {
        self.instrument(Span::current())
    }
}

/// Extension trait allowing futures to be instrumented with
/// a `tracing` [`Subscriber`](crate::Subscriber).
#[cfg_attr(docsrs, doc(cfg(feature = "std")))]
pub trait WithSubscriber: Sized {
    /// Attaches the provided [`Subscriber`] to this type, returning a
    /// [`WithDispatch`] wrapper.
    ///
    /// The attached [`Subscriber`] will be set as the [default] when the returned
    /// [`Future`] is polled.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tracing::subscriber::NoSubscriber as MySubscriber;
    /// # use tracing::subscriber::NoSubscriber as MyOtherSubscriber;
    /// # async fn docs() {
    /// use tracing::instrument::WithSubscriber;
    ///
    /// // Set the default `Subscriber`
    /// let _default = tracing::subscriber::set_default(MySubscriber::default());
    ///
    /// tracing::info!("this event will be recorded by the default `Subscriber`");
    ///
    /// // Create a different `Subscriber` and attach it to a future.
    /// let other_subscriber = MyOtherSubscriber::default();
    /// let future = async {
    ///     tracing::info!("this event will be recorded by the other `Subscriber`");
    ///     // ...
    /// };
    ///
    /// future
    ///     // Attach the other `Subscriber` to the future before awaiting it
    ///     .with_subscriber(other_subscriber)
    ///     .await;
    ///
    /// // Once the future has completed, we return to the default `Subscriber`.
    /// tracing::info!("this event will be recorded by the default `Subscriber`");
    /// # }
    /// ```
    ///
    /// [`Subscriber`]: super::Subscriber
    /// [default]: crate::dispatcher#setting-the-default-subscriber
    /// [`Future`]: std::future::Future
    fn with_subscriber<S>(self, subscriber: S) -> WithDispatch<Self>
    where
        S: Into<Dispatch>,
    {
        WithDispatch {
            inner: self,
            dispatcher: subscriber.into(),
        }
    }

    /// Attaches the current [default] [`Subscriber`] to this type, returning a
    /// [`WithDispatch`] wrapper.
    ///
    /// The attached `Subscriber` will be set as the [default] when the returned
    /// [`Future`] is polled.
    ///
    /// This can be used to propagate the current dispatcher context when
    /// spawning a new future that may run on a different thread.
    ///
    /// # Examples
    ///
    /// ```
    /// # mod tokio {
    /// #     pub(super) fn spawn(_: impl std::future::Future) {}
    /// # }
    /// # use tracing::subscriber::NoSubscriber as MySubscriber;
    /// # async fn docs() {
    /// use tracing::instrument::WithSubscriber;
    ///
    /// // Using `set_default` (rather than `set_global_default`) sets the
    /// // default `Subscriber` for *this* thread only.
    /// let _default = tracing::subscriber::set_default(MySubscriber::default());
    ///
    /// let future = async {
    ///     // ...
    /// };
    ///
    /// // If a multi-threaded async runtime is in use, this spawned task may
    /// // run on a different thread, in a different default `Subscriber`'s context.
    /// tokio::spawn(future);
    ///
    /// // However, calling `with_current_subscriber` on the future before
    /// // spawning it, ensures that the current thread's default `Subscriber` is
    /// // propagated to the spawned task, regardless of where it executes:
    /// # let future = async { };
    /// tokio::spawn(future.with_current_subscriber());
    /// # }
    /// ```
    /// [`Subscriber`]: super::Subscriber
    /// [default]: crate::dispatcher#setting-the-default-subscriber
    /// [`Future`]: std::future::Future
    #[inline]
    fn with_current_subscriber(self) -> WithDispatch<Self> {
        WithDispatch {
            inner: self,
            dispatcher: crate::dispatcher::get_default(|default| default.clone()),
        }
    }
}

pin_project! {
    /// A [`Future`] that has been instrumented with a `tracing` [`Subscriber`].
    ///
    /// This type is returned by the [`WithSubscriber`] extension trait. See that
    /// trait's documentation for details.
    ///
    /// [`Future`]: std::future::Future
    /// [`Subscriber`]: crate::Subscriber
    #[derive(Clone, Debug)]
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    #[cfg_attr(docsrs, doc(cfg(feature = "std")))]
    pub struct WithDispatch<T> {
        #[pin]
        inner: T,
        dispatcher: Dispatch,
    }
}

pin_project! {
    /// A [`Future`] that has been instrumented with a `tracing` [`Span`].
    ///
    /// This type is returned by the [`Instrument`] extension trait. See that
    /// trait's documentation for details.
    ///
    /// [`Future`]: std::future::Future
    /// [`Span`]: crate::Span
    #[project = InstrumentedProj]
    #[project_ref = InstrumentedProjRef]
    #[derive(Debug, Clone)]
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    pub struct Instrumented<T> {
        // `ManuallyDrop` is used here to to enter instrument `Drop` by entering
        // `Span` and executing `ManuallyDrop::drop`.
        #[pin]
        inner: ManuallyDrop<T>,
        span: Span,
    }

    impl<T> PinnedDrop for Instrumented<T> {
        fn drop(this: Pin<&mut Self>) {
            let this = this.project();
            let _enter = this.span.enter();
            // SAFETY: 1. `Pin::get_unchecked_mut()` is safe, because this isn't
            //             different from wrapping `T` in `Option` and calling
            //             `Pin::set(&mut this.inner, None)`, except avoiding
            //             additional memory overhead.
            //         2. `ManuallyDrop::drop()` is safe, because
            //            `PinnedDrop::drop()` is guaranteed to be called only
            //            once.
            unsafe { ManuallyDrop::drop(this.inner.get_unchecked_mut()) }
        }
    }
}

impl<'a, T> InstrumentedProj<'a, T> {
    /// Get a mutable reference to the [`Span`] a pinned mutable reference to
    /// the wrapped type.
    fn span_and_inner_pin_mut(self) -> (&'a mut Span, Pin<&'a mut T>) {
        // SAFETY: As long as `ManuallyDrop<T>` does not move, `T` won't move
        //         and `inner` is valid, because `ManuallyDrop::drop` is called
        //         only inside `Drop` of the `Instrumented`.
        let inner = unsafe { self.inner.map_unchecked_mut(|v| &mut **v) };
        (self.span, inner)
    }
}

impl<'a, T> InstrumentedProjRef<'a, T> {
    /// Get a reference to the [`Span`] a pinned reference to the wrapped type.
    fn span_and_inner_pin_ref(self) -> (&'a Span, Pin<&'a T>) {
        // SAFETY: As long as `ManuallyDrop<T>` does not move, `T` won't move
        //         and `inner` is valid, because `ManuallyDrop::drop` is called
        //         only inside `Drop` of the `Instrumented`.
        let inner = unsafe { self.inner.map_unchecked(|v| &**v) };
        (self.span, inner)
    }
}

// === impl Instrumented ===

impl<T: Future> Future for Instrumented<T> {
    type Output = T::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let (span, inner) = self.project().span_and_inner_pin_mut();
        let _enter = span.enter();
        inner.poll(cx)
    }
}

impl<T: Sized> Instrument for T {}

impl<T> Instrumented<T> {
    /// Borrows the `Span` that this type is instrumented by.
    pub fn span(&self) -> &Span {
        &self.span
    }

    /// Mutably borrows the `Span` that this type is instrumented by.
    pub fn span_mut(&mut self) -> &mut Span {
        &mut self.span
    }

    /// Borrows the wrapped type.
    pub fn inner(&self) -> &T {
        &self.inner
    }

    /// Mutably borrows the wrapped type.
    pub fn inner_mut(&mut self) -> &mut T {
        &mut self.inner
    }

    /// Get a pinned reference to the wrapped type.
    pub fn inner_pin_ref(self: Pin<&Self>) -> Pin<&T> {
        self.project_ref().span_and_inner_pin_ref().1
    }

    /// Get a pinned mutable reference to the wrapped type.
    pub fn inner_pin_mut(self: Pin<&mut Self>) -> Pin<&mut T> {
        self.project().span_and_inner_pin_mut().1
    }

    /// Consumes the `Instrumented`, returning the wrapped type.
    ///
    /// Note that this drops the span.
    pub fn into_inner(self) -> T {
        // To manually destructure `Instrumented` without `Drop`, we
        // move it into a ManuallyDrop and use pointers to its fields
        let this = ManuallyDrop::new(self);
        let span: *const Span = &this.span;
        let inner: *const ManuallyDrop<T> = &this.inner;
        // SAFETY: Those pointers are valid for reads, because `Drop` didn't
        //         run, and properly aligned, because `Instrumented` isn't
        //         `#[repr(packed)]`.
        let _span = unsafe { span.read() };
        let inner = unsafe { inner.read() };
        ManuallyDrop::into_inner(inner)
    }
}

// === impl WithDispatch ===

#[cfg(feature = "std")]
#[cfg_attr(docsrs, doc(cfg(feature = "std")))]
impl<T: Future> Future for WithDispatch<T> {
    type Output = T::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        let dispatcher = this.dispatcher;
        let future = this.inner;
        let _default = dispatcher::set_default(dispatcher);
        future.poll(cx)
    }
}

#[cfg_attr(docsrs, doc(cfg(feature = "std")))]
impl<T: Sized> WithSubscriber for T {}

#[cfg(feature = "std")]
#[cfg_attr(docsrs, doc(cfg(feature = "std")))]
impl<T> WithDispatch<T> {
    /// Borrows the [`Dispatch`] that is entered when this type is polled.
    pub fn dispatcher(&self) -> &Dispatch {
        &self.dispatcher
    }

    /// Borrows the wrapped type.
    pub fn inner(&self) -> &T {
        &self.inner
    }

    /// Mutably borrows the wrapped type.
    pub fn inner_mut(&mut self) -> &mut T {
        &mut self.inner
    }

    /// Get a pinned reference to the wrapped type.
    pub fn inner_pin_ref(self: Pin<&Self>) -> Pin<&T> {
        self.project_ref().inner
    }

    /// Get a pinned mutable reference to the wrapped type.
    pub fn inner_pin_mut(self: Pin<&mut Self>) -> Pin<&mut T> {
        self.project().inner
    }

    /// Consumes the `Instrumented`, returning the wrapped type.
    ///
    /// Note that this drops the span.
    pub fn into_inner(self) -> T {
        self.inner
    }
}
