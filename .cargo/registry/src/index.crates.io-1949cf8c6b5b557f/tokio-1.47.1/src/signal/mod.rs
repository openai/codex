//! Asynchronous signal handling for Tokio.
//!
//! Note that signal handling is in general a very tricky topic and should be
//! used with great care. This crate attempts to implement 'best practice' for
//! signal handling, but it should be evaluated for your own applications' needs
//! to see if it's suitable.
//!
//! There are some fundamental limitations of this crate documented on the OS
//! specific structures, as well.
//!
//! # Examples
//!
//! Print on "ctrl-c" notification.
//!
//! ```rust,no_run
//! use tokio::signal;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     signal::ctrl_c().await?;
//!     println!("ctrl-c received!");
//!     Ok(())
//! }
//! ```
//!
//! Wait for `SIGHUP` on Unix
//!
//! ```rust,no_run
//! # #[cfg(unix)] {
//! use tokio::signal::unix::{signal, SignalKind};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // An infinite stream of hangup signals.
//!     let mut stream = signal(SignalKind::hangup())?;
//!
//!     // Print whenever a HUP signal is received
//!     loop {
//!         stream.recv().await;
//!         println!("got signal HUP");
//!     }
//! }
//! # }
//! ```
use crate::sync::watch::Receiver;
use std::task::{Context, Poll};

#[cfg(feature = "signal")]
mod ctrl_c;
#[cfg(feature = "signal")]
pub use ctrl_c::ctrl_c;

pub(crate) mod registry;

mod os {
    #[cfg(unix)]
    pub(crate) use super::unix::{OsExtraData, OsStorage};

    #[cfg(windows)]
    pub(crate) use super::windows::{OsExtraData, OsStorage};
}

pub mod unix;
pub mod windows;

mod reusable_box;
use self::reusable_box::ReusableBoxFuture;

#[derive(Debug)]
struct RxFuture {
    inner: ReusableBoxFuture<Receiver<()>>,
}

async fn make_future(mut rx: Receiver<()>) -> Receiver<()> {
    rx.changed().await.expect("signal sender went away");
    rx
}

impl RxFuture {
    fn new(rx: Receiver<()>) -> Self {
        Self {
            inner: ReusableBoxFuture::new(make_future(rx)),
        }
    }

    async fn recv(&mut self) -> Option<()> {
        use std::future::poll_fn;
        poll_fn(|cx| self.poll_recv(cx)).await
    }

    fn poll_recv(&mut self, cx: &mut Context<'_>) -> Poll<Option<()>> {
        match self.inner.poll(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(rx) => {
                self.inner.set(make_future(rx));
                Poll::Ready(Some(()))
            }
        }
    }
}
