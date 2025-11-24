//! Windows-specific types for signal handling.
//!
//! This module is only defined on Windows and allows receiving "ctrl-c",
//! "ctrl-break", "ctrl-logoff", "ctrl-shutdown", and "ctrl-close"
//! notifications. These events are listened for via the `SetConsoleCtrlHandler`
//! function which receives the corresponding `windows_sys` event type.

#![cfg(any(windows, docsrs))]
#![cfg_attr(docsrs, doc(cfg(all(windows, feature = "signal"))))]

use crate::signal::RxFuture;
use std::io;
use std::task::{Context, Poll};

#[cfg(windows)]
#[path = "windows/sys.rs"]
mod imp;

#[cfg(windows)]
pub(crate) use self::imp::{OsExtraData, OsStorage};

// For building documentation on Unix machines when the `docsrs` flag is set.
#[cfg(not(windows))]
#[path = "windows/stub.rs"]
mod imp;

/// Creates a new listener which receives "ctrl-c" notifications sent to the
/// process.
///
/// # Examples
///
/// ```rust,no_run
/// use tokio::signal::windows::ctrl_c;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     // A listener of CTRL-C events.
///     let mut signal = ctrl_c()?;
///
///     // Print whenever a CTRL-C event is received.
///     for countdown in (0..3).rev() {
///         signal.recv().await;
///         println!("got CTRL-C. {} more to exit", countdown);
///     }
///
///     Ok(())
/// }
/// ```
pub fn ctrl_c() -> io::Result<CtrlC> {
    Ok(CtrlC {
        inner: self::imp::ctrl_c()?,
    })
}

/// Represents a listener which receives "ctrl-c" notifications sent to the process
/// via `SetConsoleCtrlHandler`.
///
/// This event can be turned into a `Stream` using [`CtrlCStream`].
///
/// [`CtrlCStream`]: https://docs.rs/tokio-stream/latest/tokio_stream/wrappers/struct.CtrlCStream.html
///
/// A notification to this process notifies *all* receivers for
/// this event. Moreover, the notifications **are coalesced** if they aren't processed
/// quickly enough. This means that if two notifications are received back-to-back,
/// then the listener may only receive one item about the two notifications.
#[must_use = "listeners do nothing unless polled"]
#[derive(Debug)]
pub struct CtrlC {
    inner: RxFuture,
}

impl CtrlC {
    /// Receives the next signal notification event.
    ///
    /// `None` is returned if no more events can be received by the listener.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use tokio::signal::windows::ctrl_c;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let mut signal = ctrl_c()?;
    ///
    ///     // Print whenever a CTRL-C event is received.
    ///     for countdown in (0..3).rev() {
    ///         signal.recv().await;
    ///         println!("got CTRL-C. {} more to exit", countdown);
    ///     }
    ///
    ///     Ok(())
    /// }
    /// ```
    pub async fn recv(&mut self) -> Option<()> {
        self.inner.recv().await
    }

    /// Polls to receive the next signal notification event, outside of an
    /// `async` context.
    ///
    /// `None` is returned if no more events can be received.
    ///
    /// # Examples
    ///
    /// Polling from a manually implemented future
    ///
    /// ```rust,no_run
    /// use std::pin::Pin;
    /// use std::future::Future;
    /// use std::task::{Context, Poll};
    /// use tokio::signal::windows::CtrlC;
    ///
    /// struct MyFuture {
    ///     ctrl_c: CtrlC,
    /// }
    ///
    /// impl Future for MyFuture {
    ///     type Output = Option<()>;
    ///
    ///     fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
    ///         println!("polling MyFuture");
    ///         self.ctrl_c.poll_recv(cx)
    ///     }
    /// }
    /// ```
    pub fn poll_recv(&mut self, cx: &mut Context<'_>) -> Poll<Option<()>> {
        self.inner.poll_recv(cx)
    }
}

/// Represents a listener which receives "ctrl-break" notifications sent to the process
/// via `SetConsoleCtrlHandler`.
///
/// This listener can be turned into a `Stream` using [`CtrlBreakStream`].
///
/// [`CtrlBreakStream`]: https://docs.rs/tokio-stream/latest/tokio_stream/wrappers/struct.CtrlBreakStream.html
///
/// A notification to this process notifies *all* receivers for
/// this event. Moreover, the notifications **are coalesced** if they aren't processed
/// quickly enough. This means that if two notifications are received back-to-back,
/// then the listener may only receive one item about the two notifications.
#[must_use = "listeners do nothing unless polled"]
#[derive(Debug)]
pub struct CtrlBreak {
    inner: RxFuture,
}

impl CtrlBreak {
    /// Receives the next signal notification event.
    ///
    /// `None` is returned if no more events can be received by this listener.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use tokio::signal::windows::ctrl_break;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     // A listener of CTRL-BREAK events.
    ///     let mut signal = ctrl_break()?;
    ///
    ///     // Print whenever a CTRL-BREAK event is received.
    ///     loop {
    ///         signal.recv().await;
    ///         println!("got signal CTRL-BREAK");
    ///     }
    /// }
    /// ```
    pub async fn recv(&mut self) -> Option<()> {
        self.inner.recv().await
    }

    /// Polls to receive the next signal notification event, outside of an
    /// `async` context.
    ///
    /// `None` is returned if no more events can be received by this listener.
    ///
    /// # Examples
    ///
    /// Polling from a manually implemented future
    ///
    /// ```rust,no_run
    /// use std::pin::Pin;
    /// use std::future::Future;
    /// use std::task::{Context, Poll};
    /// use tokio::signal::windows::CtrlBreak;
    ///
    /// struct MyFuture {
    ///     ctrl_break: CtrlBreak,
    /// }
    ///
    /// impl Future for MyFuture {
    ///     type Output = Option<()>;
    ///
    ///     fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
    ///         println!("polling MyFuture");
    ///         self.ctrl_break.poll_recv(cx)
    ///     }
    /// }
    /// ```
    pub fn poll_recv(&mut self, cx: &mut Context<'_>) -> Poll<Option<()>> {
        self.inner.poll_recv(cx)
    }
}

/// Creates a new listener which receives "ctrl-break" notifications sent to the
/// process.
///
/// # Examples
///
/// ```rust,no_run
/// use tokio::signal::windows::ctrl_break;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     // A listener of CTRL-BREAK events.
///     let mut signal = ctrl_break()?;
///
///     // Print whenever a CTRL-BREAK event is received.
///     loop {
///         signal.recv().await;
///         println!("got signal CTRL-BREAK");
///     }
/// }
/// ```
pub fn ctrl_break() -> io::Result<CtrlBreak> {
    Ok(CtrlBreak {
        inner: self::imp::ctrl_break()?,
    })
}

/// Creates a new listener which receives "ctrl-close" notifications sent to the
/// process.
///
/// # Examples
///
/// ```rust,no_run
/// use tokio::signal::windows::ctrl_close;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     // A listener of CTRL-CLOSE events.
///     let mut signal = ctrl_close()?;
///
///     // Print whenever a CTRL-CLOSE event is received.
///     for countdown in (0..3).rev() {
///         signal.recv().await;
///         println!("got CTRL-CLOSE. {} more to exit", countdown);
///     }
///
///     Ok(())
/// }
/// ```
pub fn ctrl_close() -> io::Result<CtrlClose> {
    Ok(CtrlClose {
        inner: self::imp::ctrl_close()?,
    })
}

/// Represents a listener which receives "ctrl-close" notifications sent to the process
/// via `SetConsoleCtrlHandler`.
///
/// A notification to this process notifies *all* listeners listening for
/// this event. Moreover, the notifications **are coalesced** if they aren't processed
/// quickly enough. This means that if two notifications are received back-to-back,
/// then the listener may only receive one item about the two notifications.
#[must_use = "listeners do nothing unless polled"]
#[derive(Debug)]
pub struct CtrlClose {
    inner: RxFuture,
}

impl CtrlClose {
    /// Receives the next signal notification event.
    ///
    /// `None` is returned if no more events can be received by this listener.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use tokio::signal::windows::ctrl_close;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     // A listener of CTRL-CLOSE events.
    ///     let mut signal = ctrl_close()?;
    ///
    ///     // Print whenever a CTRL-CLOSE event is received.
    ///     signal.recv().await;
    ///     println!("got CTRL-CLOSE. Cleaning up before exiting");
    ///
    ///     Ok(())
    /// }
    /// ```
    pub async fn recv(&mut self) -> Option<()> {
        self.inner.recv().await
    }

    /// Polls to receive the next signal notification event, outside of an
    /// `async` context.
    ///
    /// `None` is returned if no more events can be received by this listener.
    ///
    /// # Examples
    ///
    /// Polling from a manually implemented future
    ///
    /// ```rust,no_run
    /// use std::pin::Pin;
    /// use std::future::Future;
    /// use std::task::{Context, Poll};
    /// use tokio::signal::windows::CtrlClose;
    ///
    /// struct MyFuture {
    ///     ctrl_close: CtrlClose,
    /// }
    ///
    /// impl Future for MyFuture {
    ///     type Output = Option<()>;
    ///
    ///     fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
    ///         println!("polling MyFuture");
    ///         self.ctrl_close.poll_recv(cx)
    ///     }
    /// }
    /// ```
    pub fn poll_recv(&mut self, cx: &mut Context<'_>) -> Poll<Option<()>> {
        self.inner.poll_recv(cx)
    }
}

/// Creates a new listener which receives "ctrl-shutdown" notifications sent to the
/// process.
///
/// # Examples
///
/// ```rust,no_run
/// use tokio::signal::windows::ctrl_shutdown;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     // A listener of CTRL-SHUTDOWN events.
///     let mut signal = ctrl_shutdown()?;
///
///     signal.recv().await;
///     println!("got CTRL-SHUTDOWN. Cleaning up before exiting");
///
///     Ok(())
/// }
/// ```
pub fn ctrl_shutdown() -> io::Result<CtrlShutdown> {
    Ok(CtrlShutdown {
        inner: self::imp::ctrl_shutdown()?,
    })
}

/// Represents a listener which receives "ctrl-shutdown" notifications sent to the process
/// via `SetConsoleCtrlHandler`.
///
/// A notification to this process notifies *all* listeners listening for
/// this event. Moreover, the notifications **are coalesced** if they aren't processed
/// quickly enough. This means that if two notifications are received back-to-back,
/// then the listener may only receive one item about the two notifications.
#[must_use = "listeners do nothing unless polled"]
#[derive(Debug)]
pub struct CtrlShutdown {
    inner: RxFuture,
}

impl CtrlShutdown {
    /// Receives the next signal notification event.
    ///
    /// `None` is returned if no more events can be received by this listener.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use tokio::signal::windows::ctrl_shutdown;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     // A listener of CTRL-SHUTDOWN events.
    ///     let mut signal = ctrl_shutdown()?;
    ///
    ///     // Print whenever a CTRL-SHUTDOWN event is received.
    ///     signal.recv().await;
    ///     println!("got CTRL-SHUTDOWN. Cleaning up before exiting");
    ///
    ///     Ok(())
    /// }
    /// ```
    pub async fn recv(&mut self) -> Option<()> {
        self.inner.recv().await
    }

    /// Polls to receive the next signal notification event, outside of an
    /// `async` context.
    ///
    /// `None` is returned if no more events can be received by this listener.
    ///
    /// # Examples
    ///
    /// Polling from a manually implemented future
    ///
    /// ```rust,no_run
    /// use std::pin::Pin;
    /// use std::future::Future;
    /// use std::task::{Context, Poll};
    /// use tokio::signal::windows::CtrlShutdown;
    ///
    /// struct MyFuture {
    ///     ctrl_shutdown: CtrlShutdown,
    /// }
    ///
    /// impl Future for MyFuture {
    ///     type Output = Option<()>;
    ///
    ///     fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
    ///         println!("polling MyFuture");
    ///         self.ctrl_shutdown.poll_recv(cx)
    ///     }
    /// }
    /// ```
    pub fn poll_recv(&mut self, cx: &mut Context<'_>) -> Poll<Option<()>> {
        self.inner.poll_recv(cx)
    }
}

/// Creates a new listener which receives "ctrl-logoff" notifications sent to the
/// process.
///
/// # Examples
///
/// ```rust,no_run
/// use tokio::signal::windows::ctrl_logoff;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     // A listener of CTRL-LOGOFF events.
///     let mut signal = ctrl_logoff()?;
///
///     signal.recv().await;
///     println!("got CTRL-LOGOFF. Cleaning up before exiting");
///
///     Ok(())
/// }
/// ```
pub fn ctrl_logoff() -> io::Result<CtrlLogoff> {
    Ok(CtrlLogoff {
        inner: self::imp::ctrl_logoff()?,
    })
}

/// Represents a listener which receives "ctrl-logoff" notifications sent to the process
/// via `SetConsoleCtrlHandler`.
///
/// A notification to this process notifies *all* listeners listening for
/// this event. Moreover, the notifications **are coalesced** if they aren't processed
/// quickly enough. This means that if two notifications are received back-to-back,
/// then the listener may only receive one item about the two notifications.
#[must_use = "listeners do nothing unless polled"]
#[derive(Debug)]
pub struct CtrlLogoff {
    inner: RxFuture,
}

impl CtrlLogoff {
    /// Receives the next signal notification event.
    ///
    /// `None` is returned if no more events can be received by this listener.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use tokio::signal::windows::ctrl_logoff;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     // An listener of CTRL-LOGOFF events.
    ///     let mut signal = ctrl_logoff()?;
    ///
    ///     // Print whenever a CTRL-LOGOFF event is received.
    ///     signal.recv().await;
    ///     println!("got CTRL-LOGOFF. Cleaning up before exiting");
    ///
    ///     Ok(())
    /// }
    /// ```
    pub async fn recv(&mut self) -> Option<()> {
        self.inner.recv().await
    }

    /// Polls to receive the next signal notification event, outside of an
    /// `async` context.
    ///
    /// `None` is returned if no more events can be received by this listener.
    ///
    /// # Examples
    ///
    /// Polling from a manually implemented future
    ///
    /// ```rust,no_run
    /// use std::pin::Pin;
    /// use std::future::Future;
    /// use std::task::{Context, Poll};
    /// use tokio::signal::windows::CtrlLogoff;
    ///
    /// struct MyFuture {
    ///     ctrl_logoff: CtrlLogoff,
    /// }
    ///
    /// impl Future for MyFuture {
    ///     type Output = Option<()>;
    ///
    ///     fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
    ///         println!("polling MyFuture");
    ///         self.ctrl_logoff.poll_recv(cx)
    ///     }
    /// }
    /// ```
    pub fn poll_recv(&mut self, cx: &mut Context<'_>) -> Poll<Option<()>> {
        self.inner.poll_recv(cx)
    }
}
