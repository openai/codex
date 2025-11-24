use super::{
    error::{Closed, ServiceError},
    message::Message,
};
use futures_core::ready;
use std::sync::{Arc, Mutex};
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
use tokio::sync::mpsc;
use tower_service::Service;

pin_project_lite::pin_project! {
    /// Task that handles processing the buffer. This type should not be used
    /// directly, instead `Buffer` requires an `Executor` that can accept this task.
    ///
    /// The struct is `pub` in the private module and the type is *not* re-exported
    /// as part of the public API. This is the "sealed" pattern to include "private"
    /// types in public traits that are not meant for consumers of the library to
    /// implement (only call).
    #[derive(Debug)]
    pub struct Worker<T, Request>
    where
        T: Service<Request>,
    {
        current_message: Option<Message<Request, T::Future>>,
        rx: mpsc::Receiver<Message<Request, T::Future>>,
        service: T,
        finish: bool,
        failed: Option<ServiceError>,
        handle: Handle,
    }
}

/// Get the error out
#[derive(Debug)]
pub(crate) struct Handle {
    inner: Arc<Mutex<Option<ServiceError>>>,
}

impl<T, Request> Worker<T, Request>
where
    T: Service<Request>,
    T::Error: Into<crate::BoxError>,
{
    pub(crate) fn new(
        service: T,
        rx: mpsc::Receiver<Message<Request, T::Future>>,
    ) -> (Handle, Worker<T, Request>) {
        let handle = Handle {
            inner: Arc::new(Mutex::new(None)),
        };

        let worker = Worker {
            current_message: None,
            finish: false,
            failed: None,
            rx,
            service,
            handle: handle.clone(),
        };

        (handle, worker)
    }

    /// Return the next queued Message that hasn't been canceled.
    ///
    /// If a `Message` is returned, the `bool` is true if this is the first time we received this
    /// message, and false otherwise (i.e., we tried to forward it to the backing service before).
    fn poll_next_msg(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Poll<Option<(Message<Request, T::Future>, bool)>> {
        if self.finish {
            // We've already received None and are shutting down
            return Poll::Ready(None);
        }

        tracing::trace!("worker polling for next message");
        if let Some(msg) = self.current_message.take() {
            // If the oneshot sender is closed, then the receiver is dropped,
            // and nobody cares about the response. If this is the case, we
            // should continue to the next request.
            if !msg.tx.is_closed() {
                tracing::trace!("resuming buffered request");
                return Poll::Ready(Some((msg, false)));
            }

            tracing::trace!("dropping cancelled buffered request");
        }

        // Get the next request
        while let Some(msg) = ready!(Pin::new(&mut self.rx).poll_recv(cx)) {
            if !msg.tx.is_closed() {
                tracing::trace!("processing new request");
                return Poll::Ready(Some((msg, true)));
            }
            // Otherwise, request is canceled, so pop the next one.
            tracing::trace!("dropping cancelled request");
        }

        Poll::Ready(None)
    }

    fn failed(&mut self, error: crate::BoxError) {
        // The underlying service failed when we called `poll_ready` on it with the given `error`. We
        // need to communicate this to all the `Buffer` handles. To do so, we wrap up the error in
        // an `Arc`, send that `Arc<E>` to all pending requests, and store it so that subsequent
        // requests will also fail with the same error.

        // Note that we need to handle the case where some handle is concurrently trying to send us
        // a request. We need to make sure that *either* the send of the request fails *or* it
        // receives an error on the `oneshot` it constructed. Specifically, we want to avoid the
        // case where we send errors to all outstanding requests, and *then* the caller sends its
        // request. We do this by *first* exposing the error, *then* closing the channel used to
        // send more requests (so the client will see the error when the send fails), and *then*
        // sending the error to all outstanding requests.
        let error = ServiceError::new(error);

        let mut inner = self.handle.inner.lock().unwrap();

        if inner.is_some() {
            // Future::poll was called after we've already errored out!
            return;
        }

        *inner = Some(error.clone());
        drop(inner);

        self.rx.close();

        // By closing the mpsc::Receiver, we know that poll_next_msg will soon return Ready(None),
        // which will trigger the `self.finish == true` phase. We just need to make sure that any
        // requests that we receive before we've exhausted the receiver receive the error:
        self.failed = Some(error);
    }
}

impl<T, Request> Future for Worker<T, Request>
where
    T: Service<Request>,
    T::Error: Into<crate::BoxError>,
{
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.finish {
            return Poll::Ready(());
        }

        loop {
            match ready!(self.poll_next_msg(cx)) {
                Some((msg, first)) => {
                    let _guard = msg.span.enter();
                    if let Some(ref failed) = self.failed {
                        tracing::trace!("notifying caller about worker failure");
                        let _ = msg.tx.send(Err(failed.clone()));
                        continue;
                    }

                    // Wait for the service to be ready
                    tracing::trace!(
                        resumed = !first,
                        message = "worker received request; waiting for service readiness"
                    );
                    match self.service.poll_ready(cx) {
                        Poll::Ready(Ok(())) => {
                            tracing::debug!(service.ready = true, message = "processing request");
                            let response = self.service.call(msg.request);

                            // Send the response future back to the sender.
                            //
                            // An error means the request had been canceled in-between
                            // our calls, the response future will just be dropped.
                            tracing::trace!("returning response future");
                            let _ = msg.tx.send(Ok(response));
                        }
                        Poll::Pending => {
                            tracing::trace!(service.ready = false, message = "delay");
                            // Put out current message back in its slot.
                            drop(_guard);
                            self.current_message = Some(msg);
                            return Poll::Pending;
                        }
                        Poll::Ready(Err(e)) => {
                            let error = e.into();
                            tracing::debug!({ %error }, "service failed");
                            drop(_guard);
                            self.failed(error);
                            let _ = msg.tx.send(Err(self
                                .failed
                                .as_ref()
                                .expect("Worker::failed did not set self.failed?")
                                .clone()));
                        }
                    }
                }
                None => {
                    // No more more requests _ever_.
                    self.finish = true;
                    return Poll::Ready(());
                }
            }
        }
    }
}

impl Handle {
    pub(crate) fn get_error_on_closed(&self) -> crate::BoxError {
        self.inner
            .lock()
            .unwrap()
            .as_ref()
            .map(|svc_err| svc_err.clone().into())
            .unwrap_or_else(|| Closed::new().into())
    }
}

impl Clone for Handle {
    fn clone(&self) -> Handle {
        Handle {
            inner: self.inner.clone(),
        }
    }
}
