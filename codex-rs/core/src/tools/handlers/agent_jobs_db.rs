use std::future::Future;
use std::time::Duration;

const SQLITE_LOCK_RETRY_LIMIT: usize = 2;

fn is_sqlite_lock_error(err: &anyhow::Error) -> bool {
    err.chain().any(|cause| {
        let message = cause.to_string();
        message.contains("database is locked") || message.contains("database table is locked")
    })
}

fn retry_delay(attempt: usize) -> Duration {
    Duration::from_millis(250 * (attempt as u64 + 1))
}

pub(super) async fn retry_locked<T, F, Fut>(operation: &'static str, mut op: F) -> anyhow::Result<T>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = anyhow::Result<T>>,
{
    let mut attempt = 0usize;
    loop {
        match op().await {
            Ok(value) => return Ok(value),
            Err(err) if is_sqlite_lock_error(&err) && attempt < SQLITE_LOCK_RETRY_LIMIT => {
                let retry_in = retry_delay(attempt);
                tracing::warn!(
                    operation,
                    attempt = attempt + 1,
                    max_attempts = SQLITE_LOCK_RETRY_LIMIT + 1,
                    retry_delay_ms = retry_in.as_millis() as u64,
                    error = %err,
                    "agent job DB operation hit sqlite lock; retrying"
                );
                tokio::time::sleep(retry_in).await;
                attempt += 1;
            }
            Err(err) => return Err(err),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use std::sync::Arc;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;

    #[tokio::test]
    async fn retry_locked_retries_transient_sqlite_locks() -> anyhow::Result<()> {
        let attempts = Arc::new(AtomicUsize::new(0));
        let attempts_for_op = Arc::clone(&attempts);

        let result = retry_locked("test_sqlite_retry", move || {
            let attempts = Arc::clone(&attempts_for_op);
            async move {
                let attempt = attempts.fetch_add(1, Ordering::SeqCst);
                if attempt < 2 {
                    Err(anyhow::anyhow!(
                        "error returned from database: (code: 5) database is locked"
                    ))
                } else {
                    Ok("ok")
                }
            }
        })
        .await?;

        assert_eq!(result, "ok");
        assert_eq!(attempts.load(Ordering::SeqCst), 3);
        Ok(())
    }

    #[tokio::test]
    async fn retry_locked_does_not_retry_non_lock_errors() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let attempts_for_op = Arc::clone(&attempts);

        let err = retry_locked("test_non_sqlite_retry", move || {
            let attempts = Arc::clone(&attempts_for_op);
            async move {
                attempts.fetch_add(1, Ordering::SeqCst);
                Err::<(), anyhow::Error>(anyhow::anyhow!("boom"))
            }
        })
        .await
        .expect_err("non-lock errors should not be retried");

        assert_eq!(err.to_string(), "boom");
        assert_eq!(attempts.load(Ordering::SeqCst), 1);
    }
}
