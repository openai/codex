use core::pin::Pin;
use futures_core::future::{FusedFuture, Future};
use futures_core::task::{Context, Poll};
use pin_project_lite::pin_project;

pin_project! {
    /// Future for the [`fuse`](super::FutureExt::fuse) method.
    #[derive(Debug)]
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    pub struct Fuse<Fut> {
        #[pin]
        inner: Option<Fut>,
    }
}

impl<Fut> Fuse<Fut> {
    pub(super) fn new(f: Fut) -> Self {
        Self { inner: Some(f) }
    }
}

impl<Fut: Future> Fuse<Fut> {
    /// Creates a new `Fuse`-wrapped future which is already terminated.
    ///
    /// This can be useful in combination with looping and the `select!`
    /// macro, which bypasses terminated futures.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures::executor::block_on(async {
    /// use futures::channel::mpsc;
    /// use futures::future::{Fuse, FusedFuture, FutureExt};
    /// use futures::select;
    /// use futures::stream::StreamExt;
    /// use futures::pin_mut;
    ///
    /// let (sender, mut stream) = mpsc::unbounded();
    ///
    /// // Send a few messages into the stream
    /// sender.unbounded_send(()).unwrap();
    /// sender.unbounded_send(()).unwrap();
    /// drop(sender);
    ///
    /// // Use `Fuse::terminated()` to create an already-terminated future
    /// // which may be instantiated later.
    /// let foo_printer = Fuse::terminated();
    /// pin_mut!(foo_printer);
    ///
    /// loop {
    ///     select! {
    ///         _ = foo_printer => {},
    ///         () = stream.select_next_some() => {
    ///             if !foo_printer.is_terminated() {
    ///                 println!("Foo is already being printed!");
    ///             } else {
    ///                 foo_printer.set(async {
    ///                     // do some other async operations
    ///                     println!("Printing foo from `foo_printer` future");
    ///                 }.fuse());
    ///             }
    ///         },
    ///         complete => break, // `foo_printer` is terminated and the stream is done
    ///     }
    /// }
    /// # });
    /// ```
    pub fn terminated() -> Self {
        Self { inner: None }
    }
}

impl<Fut: Future> FusedFuture for Fuse<Fut> {
    fn is_terminated(&self) -> bool {
        self.inner.is_none()
    }
}

impl<Fut: Future> Future for Fuse<Fut> {
    type Output = Fut::Output;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Fut::Output> {
        match self.as_mut().project().inner.as_pin_mut() {
            Some(fut) => fut.poll(cx).map(|output| {
                self.project().inner.set(None);
                output
            }),
            None => Poll::Pending,
        }
    }
}
