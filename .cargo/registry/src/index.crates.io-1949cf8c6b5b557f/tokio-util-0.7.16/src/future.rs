//! An extension trait for Futures that provides a variety of convenient adapters.

mod with_cancellation_token;
use with_cancellation_token::{WithCancellationTokenFuture, WithCancellationTokenFutureOwned};

use std::future::Future;

use crate::sync::CancellationToken;

/// A trait which contains a variety of convenient adapters and utilities for `Future`s.
pub trait FutureExt: Future {
    cfg_time! {
        /// A wrapper around [`tokio::time::timeout`], with the advantage that it is easier to write
        /// fluent call chains.
        ///
        /// # Examples
        ///
        /// ```rust
        /// use tokio::{sync::oneshot, time::Duration};
        /// use tokio_util::future::FutureExt;
        ///
        /// # async fn dox() {
        /// let (_tx, rx) = oneshot::channel::<()>();
        ///
        /// let res = rx.timeout(Duration::from_millis(10)).await;
        /// assert!(res.is_err());
        /// # }
        /// ```
        fn timeout(self, timeout: std::time::Duration) -> tokio::time::Timeout<Self>
        where
            Self: Sized,
        {
            tokio::time::timeout(timeout, self)
        }

        /// A wrapper around [`tokio::time::timeout_at`], with the advantage that it is easier to write
        /// fluent call chains.
        ///
        /// # Examples
        ///
        /// ```rust
        /// use tokio::{sync::oneshot, time::{Duration, Instant}};
        /// use tokio_util::future::FutureExt;
        ///
        /// # async fn dox() {
        /// let (_tx, rx) = oneshot::channel::<()>();
        /// let deadline = Instant::now() + Duration::from_millis(10);
        ///
        /// let res = rx.timeout_at(deadline).await;
        /// assert!(res.is_err());
        /// # }
        /// ```
        fn timeout_at(self, deadline: tokio::time::Instant) -> tokio::time::Timeout<Self>
        where
            Self: Sized,
        {
            tokio::time::timeout_at(deadline, self)
        }
    }

    /// Similar to [`CancellationToken::run_until_cancelled`],
    /// but with the advantage that it is easier to write fluent call chains,
    /// and biased towards waiting for [`CancellationToken`] to complete.
    ///
    /// # Fairness
    ///
    /// Calling this on an already-cancelled token directly returns `None`.
    /// For all subsequent polls, in case of concurrent completion and
    /// cancellation, this is biased towards the future completion.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use tokio::sync::oneshot;
    /// use tokio_util::future::FutureExt;
    /// use tokio_util::sync::CancellationToken;
    ///
    /// # async fn dox() {
    /// let (_tx, rx) = oneshot::channel::<()>();
    /// let token = CancellationToken::new();
    /// let token_clone = token.clone();
    /// tokio::spawn(async move {
    ///     tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    ///     token.cancel();
    /// });
    /// assert!(rx.with_cancellation_token(&token_clone).await.is_none())
    /// # }
    /// ```
    fn with_cancellation_token(
        self,
        cancellation_token: &CancellationToken,
    ) -> WithCancellationTokenFuture<'_, Self>
    where
        Self: Sized,
    {
        WithCancellationTokenFuture::new(cancellation_token, self)
    }

    /// Similar to [`CancellationToken::run_until_cancelled_owned`],
    /// but with the advantage that it is easier to write fluent call chains,
    /// and biased towards waiting for [`CancellationToken`] to complete.
    ///
    /// # Fairness
    ///
    /// Calling this on an already-cancelled token directly returns `None`.
    /// For all subsequent polls, in case of concurrent completion and
    /// cancellation, this is biased towards the future completion.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use tokio::sync::oneshot;
    /// use tokio_util::future::FutureExt;
    /// use tokio_util::sync::CancellationToken;
    ///
    /// # async fn dox() {
    /// let (_tx, rx) = oneshot::channel::<()>();
    /// let token = CancellationToken::new();
    /// let token_clone = token.clone();
    /// tokio::spawn(async move {
    ///     tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    ///     token.cancel();
    /// });
    /// assert!(rx.with_cancellation_token_owned(token_clone).await.is_none())
    /// # }
    /// ```
    fn with_cancellation_token_owned(
        self,
        cancellation_token: CancellationToken,
    ) -> WithCancellationTokenFutureOwned<Self>
    where
        Self: Sized,
    {
        WithCancellationTokenFutureOwned::new(cancellation_token, self)
    }
}

impl<T: Future + ?Sized> FutureExt for T {}
