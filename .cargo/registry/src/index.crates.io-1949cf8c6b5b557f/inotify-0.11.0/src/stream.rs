use std::{
    io,
    os::unix::io::{AsRawFd, RawFd},
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

use futures_core::{ready, Stream};
use tokio::io::unix::AsyncFd;

use crate::events::{Event, EventOwned};
use crate::fd_guard::FdGuard;
use crate::Inotify;
use crate::util::read_into_buffer;
use crate::watches::Watches;

/// Stream of inotify events
///
/// Allows for streaming events returned by [`Inotify::into_event_stream`].
#[derive(Debug)]
pub struct EventStream<T> {
    fd: AsyncFd<ArcFdGuard>,
    buffer: T,
    buffer_pos: usize,
    unused_bytes: usize,
}

impl<T> EventStream<T>
where
    T: AsMut<[u8]> + AsRef<[u8]>,
{
    /// Returns a new `EventStream` associated with the default reactor.
    pub(crate) fn new(fd: Arc<FdGuard>, buffer: T) -> io::Result<Self> {
        Ok(EventStream {
            fd: AsyncFd::new(ArcFdGuard(fd))?,
            buffer,
            buffer_pos: 0,
            unused_bytes: 0,
        })
    }

    /// Returns an instance of `Watches` to add and remove watches.
    /// See [`Watches::add`] and [`Watches::remove`].
    pub fn watches(&self) -> Watches {
        Watches::new(self.fd.get_ref().0.clone())
    }

    /// Consumes the `EventStream` instance and returns an `Inotify` using the original
    /// file descriptor that was passed from `Inotify` to create the `EventStream`.
    pub fn into_inotify(self) -> Inotify {
        Inotify::from_file_descriptor(self.fd.into_inner().0)
    }
}

impl<T> Stream for EventStream<T>
where
    T: AsMut<[u8]> + AsRef<[u8]>,
{
    type Item = io::Result<EventOwned>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        // Safety: safe because we never move out of `self_`.
        let self_ = unsafe { self.get_unchecked_mut() };

        if self_.unused_bytes == 0 {
            // Nothing usable in buffer. Need to reset and fill buffer.
            self_.buffer_pos = 0;
            self_.unused_bytes = ready!(read(&self_.fd, self_.buffer.as_mut(), cx))?;
        }

        if self_.unused_bytes == 0 {
            // The previous read returned `0` signalling end-of-file. Let's
            // signal end-of-stream to the caller.
            return Poll::Ready(None);
        }

        // We have bytes in the buffer. inotify doesn't put partial events in
        // there, and we only take complete events out. That means we have at
        // least one event in there and can call `from_buffer` to take it out.
        let (bytes_consumed, event) = Event::from_buffer(
            Arc::downgrade(&self_.fd.get_ref().0),
            &self_.buffer.as_ref()[self_.buffer_pos..],
        );
        self_.buffer_pos += bytes_consumed;
        self_.unused_bytes -= bytes_consumed;

        Poll::Ready(Some(Ok(event.to_owned())))
    }
}

// Newtype wrapper because AsRawFd isn't implemented for Arc<T> where T: AsRawFd.
#[derive(Debug)]
struct ArcFdGuard(Arc<FdGuard>);

impl AsRawFd for ArcFdGuard {
    fn as_raw_fd(&self) -> RawFd {
        self.0.as_raw_fd()
    }
}

fn read(fd: &AsyncFd<ArcFdGuard>, buffer: &mut [u8], cx: &mut Context) -> Poll<io::Result<usize>> {
    let mut guard = ready!(fd.poll_read_ready(cx))?;
    let result = guard.try_io(|_| {
        let read = read_into_buffer(fd.as_raw_fd(), buffer);
        if read == -1 {
            return Err(io::Error::last_os_error());
        }

        Ok(read as usize)
    });

    match result {
        Ok(result) => Poll::Ready(result),
        Err(_would_block) => {
            cx.waker().wake_by_ref();
            Poll::Pending
        }
    }
}
