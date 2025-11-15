use async_trait::async_trait;
use serde_json::Value;
use std::future::Future;
use tokio_util::sync::CancellationToken;

/// Cancellation error with optional context about dangling artifacts.
#[derive(Debug, Clone)]
pub struct CancelErr {
    /// Optional artifacts that were being processed when cancelled
    pub dangling_artifacts: Option<Vec<Value>>,
}

impl CancelErr {
    /// Create a new CancelErr without artifacts
    pub fn new() -> Self {
        Self {
            dangling_artifacts: None,
        }
    }

    /// Create a CancelErr with dangling artifacts
    pub fn with_artifacts(artifacts: Vec<Value>) -> Self {
        Self {
            dangling_artifacts: Some(artifacts),
        }
    }

    /// Add artifacts to this error
    pub fn set_artifacts(&mut self, artifacts: Vec<Value>) {
        self.dangling_artifacts = Some(artifacts);
    }
}

impl Default for CancelErr {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
pub trait OrCancelExt: Sized {
    type Output;

    async fn or_cancel(self, token: &CancellationToken) -> Result<Self::Output, CancelErr>;
}

#[async_trait]
impl<F> OrCancelExt for F
where
    F: Future + Send,
    F::Output: Send,
{
    type Output = F::Output;

    async fn or_cancel(self, token: &CancellationToken) -> Result<Self::Output, CancelErr> {
        tokio::select! {
            _ = token.cancelled() => Err(CancelErr::new()),
            res = self => Ok(res),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use std::time::Duration;
    use tokio::task;
    use tokio::time::sleep;

    #[tokio::test]
    async fn returns_ok_when_future_completes_first() {
        let token = CancellationToken::new();
        let value = async { 42 };

        let result = value.or_cancel(&token).await;

        assert!(result.is_ok());
        assert!(result.unwrap() == 42);
    }

    #[tokio::test]
    async fn returns_err_when_token_already_cancelled() {
        let token = CancellationToken::new();
        token.cancel();

        let result = async {
            sleep(Duration::from_millis(50)).await;
            5
        }
        .or_cancel(&token)
        .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().dangling_artifacts.is_none());
    }
}
