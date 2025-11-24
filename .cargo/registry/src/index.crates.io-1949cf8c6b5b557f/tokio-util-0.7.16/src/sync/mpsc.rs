use futures_sink::Sink;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::{fmt, mem};
use tokio::sync::mpsc::OwnedPermit;
use tokio::sync::mpsc::Sender;

use super::ReusableBoxFuture;

/// Error returned by the `PollSender` when the channel is closed.
#[derive(Debug)]
pub struct PollSendError<T>(Option<T>);

impl<T> PollSendError<T> {
    /// Consumes the stored value, if any.
    ///
    /// If this error was encountered when calling `start_send`/`send_item`, this will be the item
    /// that the caller attempted to send.  Otherwise, it will be `None`.
    pub fn into_inner(self) -> Option<T> {
        self.0
    }
}

impl<T> fmt::Display for PollSendError<T> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(fmt, "channel closed")
    }
}

impl<T: fmt::Debug> std::error::Error for PollSendError<T> {}

#[derive(Debug)]
enum State<T> {
    Idle(Sender<T>),
    Acquiring,
    ReadyToSend(OwnedPermit<T>),
    Closed,
}

/// A wrapper around [`mpsc::Sender`] that can be polled.
///
/// [`mpsc::Sender`]: tokio::sync::mpsc::Sender
#[derive(Debug)]
pub struct PollSender<T> {
    sender: Option<Sender<T>>,
    state: State<T>,
    acquire: PollSenderFuture<T>,
}

// Creates a future for acquiring a permit from the underlying channel.  This is used to ensure
// there's capacity for a send to complete.
//
// By reusing the same async fn for both `Some` and `None`, we make sure every future passed to
// ReusableBoxFuture has the same underlying type, and hence the same size and alignment.
async fn make_acquire_future<T>(
    data: Option<Sender<T>>,
) -> Result<OwnedPermit<T>, PollSendError<T>> {
    match data {
        Some(sender) => sender
            .reserve_owned()
            .await
            .map_err(|_| PollSendError(None)),
        None => unreachable!("this future should not be pollable in this state"),
    }
}

type InnerFuture<'a, T> = ReusableBoxFuture<'a, Result<OwnedPermit<T>, PollSendError<T>>>;

#[derive(Debug)]
// TODO: This should be replace with a type_alias_impl_trait to eliminate `'static` and all the transmutes
struct PollSenderFuture<T>(InnerFuture<'static, T>);

impl<T> PollSenderFuture<T> {
    /// Create with an empty inner future with no `Send` bound.
    fn empty() -> Self {
        // We don't use `make_acquire_future` here because our relaxed bounds on `T` are not
        // compatible with the transitive bounds required by `Sender<T>`.
        Self(ReusableBoxFuture::new(async { unreachable!() }))
    }
}

impl<T: Send> PollSenderFuture<T> {
    /// Create with an empty inner future.
    fn new() -> Self {
        let v = InnerFuture::new(make_acquire_future(None));
        // This is safe because `make_acquire_future(None)` is actually `'static`
        Self(unsafe { mem::transmute::<InnerFuture<'_, T>, InnerFuture<'static, T>>(v) })
    }

    /// Poll the inner future.
    fn poll(&mut self, cx: &mut Context<'_>) -> Poll<Result<OwnedPermit<T>, PollSendError<T>>> {
        self.0.poll(cx)
    }

    /// Replace the inner future.
    fn set(&mut self, sender: Option<Sender<T>>) {
        let inner: *mut InnerFuture<'static, T> = &mut self.0;
        let inner: *mut InnerFuture<'_, T> = inner.cast();
        // SAFETY: The `make_acquire_future(sender)` future must not exist after the type `T`
        // becomes invalid, and this casts away the type-level lifetime check for that. However, the
        // inner future is never moved out of this `PollSenderFuture<T>`, so the future will not
        // live longer than the `PollSenderFuture<T>` lives. A `PollSenderFuture<T>` is guaranteed
        // to not exist after the type `T` becomes invalid, because it is annotated with a `T`, so
        // this is ok.
        let inner = unsafe { &mut *inner };
        inner.set(make_acquire_future(sender));
    }
}

impl<T: Send> PollSender<T> {
    /// Creates a new `PollSender`.
    pub fn new(sender: Sender<T>) -> Self {
        Self {
            sender: Some(sender.clone()),
            state: State::Idle(sender),
            acquire: PollSenderFuture::new(),
        }
    }

    fn take_state(&mut self) -> State<T> {
        mem::replace(&mut self.state, State::Closed)
    }

    /// Attempts to prepare the sender to receive a value.
    ///
    /// This method must be called and return `Poll::Ready(Ok(()))` prior to each call to
    /// `send_item`.
    ///
    /// This method returns `Poll::Ready` once the underlying channel is ready to receive a value,
    /// by reserving a slot in the channel for the item to be sent. If this method returns
    /// `Poll::Pending`, the current task is registered to be notified (via
    /// `cx.waker().wake_by_ref()`) when `poll_reserve` should be called again.
    ///
    /// # Errors
    ///
    /// If the channel is closed, an error will be returned.  This is a permanent state.
    pub fn poll_reserve(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), PollSendError<T>>> {
        loop {
            let (result, next_state) = match self.take_state() {
                State::Idle(sender) => {
                    // Start trying to acquire a permit to reserve a slot for our send, and
                    // immediately loop back around to poll it the first time.
                    self.acquire.set(Some(sender));
                    (None, State::Acquiring)
                }
                State::Acquiring => match self.acquire.poll(cx) {
                    // Channel has capacity.
                    Poll::Ready(Ok(permit)) => {
                        (Some(Poll::Ready(Ok(()))), State::ReadyToSend(permit))
                    }
                    // Channel is closed.
                    Poll::Ready(Err(e)) => (Some(Poll::Ready(Err(e))), State::Closed),
                    // Channel doesn't have capacity yet, so we need to wait.
                    Poll::Pending => (Some(Poll::Pending), State::Acquiring),
                },
                // We're closed, either by choice or because the underlying sender was closed.
                s @ State::Closed => (Some(Poll::Ready(Err(PollSendError(None)))), s),
                // We're already ready to send an item.
                s @ State::ReadyToSend(_) => (Some(Poll::Ready(Ok(()))), s),
            };

            self.state = next_state;
            if let Some(result) = result {
                return result;
            }
        }
    }

    /// Sends an item to the channel.
    ///
    /// Before calling `send_item`, `poll_reserve` must be called with a successful return
    /// value of `Poll::Ready(Ok(()))`.
    ///
    /// # Errors
    ///
    /// If the channel is closed, an error will be returned.  This is a permanent state.
    ///
    /// # Panics
    ///
    /// If `poll_reserve` was not successfully called prior to calling `send_item`, then this method
    /// will panic.
    #[track_caller]
    pub fn send_item(&mut self, value: T) -> Result<(), PollSendError<T>> {
        let (result, next_state) = match self.take_state() {
            State::Idle(_) | State::Acquiring => {
                panic!("`send_item` called without first calling `poll_reserve`")
            }
            // We have a permit to send our item, so go ahead, which gets us our sender back.
            State::ReadyToSend(permit) => (Ok(()), State::Idle(permit.send(value))),
            // We're closed, either by choice or because the underlying sender was closed.
            State::Closed => (Err(PollSendError(Some(value))), State::Closed),
        };

        // Handle deferred closing if `close` was called between `poll_reserve` and `send_item`.
        self.state = if self.sender.is_some() {
            next_state
        } else {
            State::Closed
        };
        result
    }

    /// Checks whether this sender is been closed.
    ///
    /// The underlying channel that this sender was wrapping may still be open.
    pub fn is_closed(&self) -> bool {
        matches!(self.state, State::Closed) || self.sender.is_none()
    }

    /// Gets a reference to the `Sender` of the underlying channel.
    ///
    /// If `PollSender` has been closed, `None` is returned. The underlying channel that this sender
    /// was wrapping may still be open.
    pub fn get_ref(&self) -> Option<&Sender<T>> {
        self.sender.as_ref()
    }

    /// Closes this sender.
    ///
    /// No more messages will be able to be sent from this sender, but the underlying channel will
    /// remain open until all senders have dropped, or until the [`Receiver`] closes the channel.
    ///
    /// If a slot was previously reserved by calling `poll_reserve`, then a final call can be made
    /// to `send_item` in order to consume the reserved slot.  After that, no further sends will be
    /// possible.  If you do not intend to send another item, you can release the reserved slot back
    /// to the underlying sender by calling [`abort_send`].
    ///
    /// [`abort_send`]: crate::sync::PollSender::abort_send
    /// [`Receiver`]: tokio::sync::mpsc::Receiver
    pub fn close(&mut self) {
        // Mark ourselves officially closed by dropping our main sender.
        self.sender = None;

        // If we're already idle, closed, or we haven't yet reserved a slot, we can quickly
        // transition to the closed state.  Otherwise, leave the existing permit in place for the
        // caller if they want to complete the send.
        match self.state {
            State::Idle(_) => self.state = State::Closed,
            State::Acquiring => {
                self.acquire.set(None);
                self.state = State::Closed;
            }
            _ => {}
        }
    }

    /// Aborts the current in-progress send, if any.
    ///
    /// Returns `true` if a send was aborted.  If the sender was closed prior to calling
    /// `abort_send`, then the sender will remain in the closed state, otherwise the sender will be
    /// ready to attempt another send.
    pub fn abort_send(&mut self) -> bool {
        // We may have been closed in the meantime, after a call to `poll_reserve` already
        // succeeded.  We'll check if `self.sender` is `None` to see if we should transition to the
        // closed state when we actually abort a send, rather than resetting ourselves back to idle.

        let (result, next_state) = match self.take_state() {
            // We're currently trying to reserve a slot to send into.
            State::Acquiring => {
                // Replacing the future drops the in-flight one.
                self.acquire.set(None);

                // If we haven't closed yet, we have to clone our stored sender since we have no way
                // to get it back from the acquire future we just dropped.
                let state = match self.sender.clone() {
                    Some(sender) => State::Idle(sender),
                    None => State::Closed,
                };
                (true, state)
            }
            // We got the permit.  If we haven't closed yet, get the sender back.
            State::ReadyToSend(permit) => {
                let state = if self.sender.is_some() {
                    State::Idle(permit.release())
                } else {
                    State::Closed
                };
                (true, state)
            }
            s => (false, s),
        };

        self.state = next_state;
        result
    }
}

impl<T> Clone for PollSender<T> {
    /// Clones this `PollSender`.
    ///
    /// The resulting `PollSender` will have an initial state identical to calling `PollSender::new`.
    fn clone(&self) -> PollSender<T> {
        let (sender, state) = match self.sender.clone() {
            Some(sender) => (Some(sender.clone()), State::Idle(sender)),
            None => (None, State::Closed),
        };

        Self {
            sender,
            state,
            acquire: PollSenderFuture::empty(),
        }
    }
}

impl<T: Send> Sink<T> for PollSender<T> {
    type Error = PollSendError<T>;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Pin::into_inner(self).poll_reserve(cx)
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn start_send(self: Pin<&mut Self>, item: T) -> Result<(), Self::Error> {
        Pin::into_inner(self).send_item(item)
    }

    fn poll_close(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Pin::into_inner(self).close();
        Poll::Ready(Ok(()))
    }
}
