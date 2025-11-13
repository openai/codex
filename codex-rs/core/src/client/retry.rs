use std::time::Duration;

use crate::error::CodexErr;
use crate::error::Result;

/// Common interface for classifying stream start errors as retryable or fatal.
pub(crate) trait RetryableStreamError {
    /// Returns a delay for the next retry attempt, or `None` if the error
    /// should be treated as fatal and not retried.
    fn delay(&self, attempt: u64) -> Option<Duration>;

    /// Converts this error into the final `CodexErr` that should be surfaced
    /// to callers when retries are exhausted or the error is fatal.
    fn into_error(self) -> CodexErr;
}

/// Helper to retry a streaming operation with provider-configured backoff.
///
/// The caller supplies an `attempt_fn` that is invoked once per attempt with
/// the current attempt index in `[0, max_attempts]`. On success, the value is
/// returned immediately. On error, the error's [`RetryableStreamError`]
/// implementation decides whether to retry (with an optional delay) or to
/// surface a final error.
pub(crate) async fn retry_stream<F, Fut, T, E>(max_attempts: u64, mut attempt_fn: F) -> Result<T>
where
    F: FnMut(u64) -> Fut,
    Fut: std::future::Future<Output = std::result::Result<T, E>>,
    E: RetryableStreamError,
{
    for attempt in 0..=max_attempts {
        match attempt_fn(attempt).await {
            Ok(value) => return Ok(value),
            Err(err) => {
                let delay = err.delay(attempt);

                // Fatal error or final attempt: surface to caller.
                if attempt == max_attempts || delay.is_none() {
                    return Err(err.into_error());
                }

                if let Some(duration) = delay {
                    tokio::time::sleep(duration).await;
                }
            }
        }
    }

    unreachable!("retry_stream should always return");
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[derive(Clone)]
    struct TestError {
        fatal: bool,
    }

    impl RetryableStreamError for TestError {
        fn delay(&self, attempt: u64) -> Option<Duration> {
            if self.fatal {
                None
            } else {
                Some(Duration::from_millis(attempt * 10))
            }
        }

        fn into_error(self) -> CodexErr {
            if self.fatal {
                CodexErr::InternalServerError
            } else {
                CodexErr::Io(std::io::Error::new(std::io::ErrorKind::Other, "retryable"))
            }
        }
    }

    #[tokio::test]
    async fn retries_until_success_before_max_attempts() {
        let max_attempts = 3;

        let result: Result<&str> = retry_stream(max_attempts, |attempt| async move {
            if attempt < 2 {
                Err(TestError { fatal: false })
            } else {
                Ok("ok")
            }
        })
        .await;

        assert_eq!(result.unwrap(), "ok");
    }

    #[tokio::test]
    async fn stops_on_fatal_error_without_retrying() {
        use std::sync::Arc;
        use std::sync::atomic::AtomicUsize;
        use std::sync::atomic::Ordering;

        let calls = Arc::new(AtomicUsize::new(0));
        let calls_ref = calls.clone();

        let result: Result<()> = retry_stream(5, move |_attempt| {
            let calls_ref = calls_ref.clone();
            async move {
                calls_ref.fetch_add(1, Ordering::SeqCst);
                Err(TestError { fatal: true })
            }
        })
        .await;

        assert!(result.is_err());
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn stops_after_max_attempts_for_retryable_errors() {
        use std::sync::Arc;
        use std::sync::atomic::AtomicUsize;
        use std::sync::atomic::Ordering;

        let calls = Arc::new(AtomicUsize::new(0));
        let calls_ref = calls.clone();

        let max_attempts = 2;

        let result: Result<()> = retry_stream(max_attempts, move |_attempt| {
            let calls_ref = calls_ref.clone();
            async move {
                calls_ref.fetch_add(1, Ordering::SeqCst);
                Err(TestError { fatal: false })
            }
        })
        .await;

        assert!(result.is_err());
        assert_eq!(calls.load(Ordering::SeqCst), (max_attempts + 1) as usize);
    }
}
