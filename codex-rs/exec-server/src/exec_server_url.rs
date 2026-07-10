use std::sync::Arc;
use std::sync::OnceLock;

use tokio::sync::watch;

use crate::ExecServerError;

type ExecServerUrlResult = Result<String, String>;

struct ExecServerUrlInner {
    result: OnceLock<ExecServerUrlResult>,
    result_available: watch::Sender<bool>,
}

/// The stable WebSocket URL for one exec-server environment.
///
/// A URL may be available when the environment is registered or supplied later by the owner that
/// provisions the environment. Every consumer observes the same one-shot ready or failed result.
#[derive(Clone)]
pub(crate) struct ExecServerUrl {
    inner: Arc<ExecServerUrlInner>,
}

impl ExecServerUrl {
    pub(crate) fn ready(url: String) -> Self {
        let (result_available, _receiver) = watch::channel(true);
        Self {
            inner: Arc::new(ExecServerUrlInner {
                result: OnceLock::from(Ok(url)),
                result_available,
            }),
        }
    }

    pub(crate) fn pending() -> Self {
        let (result_available, _receiver) = watch::channel(false);
        Self {
            inner: Arc::new(ExecServerUrlInner {
                result: OnceLock::new(),
                result_available,
            }),
        }
    }

    pub(crate) async fn resolve(&self) -> Result<String, ExecServerError> {
        let mut result_available = self.inner.result_available.subscribe();
        while !*result_available.borrow_and_update() {
            if result_available.changed().await.is_err() {
                return Err(ExecServerError::Disconnected(
                    "environment URL registration ended before completion".to_string(),
                ));
            }
        }
        self.inner
            .result
            .get()
            .ok_or_else(|| {
                ExecServerError::Protocol(
                    "exec-server URL was marked available without a result".to_string(),
                )
            })?
            .clone()
            .map_err(|message| {
                ExecServerError::Disconnected(format!("environment unavailable: {message}"))
            })
    }

    pub(crate) fn current(&self) -> Option<&str> {
        match self.inner.result.get()? {
            Ok(url) => Some(url.as_str()),
            Err(_) => None,
        }
    }

    pub(crate) fn set_ready(&self, url: String) -> Result<(), ExecServerError> {
        self.complete(Ok(url))
    }

    pub(crate) fn set_failed(&self, message: String) -> Result<(), ExecServerError> {
        self.complete(Err(message))
    }

    fn complete(&self, result: ExecServerUrlResult) -> Result<(), ExecServerError> {
        self.inner
            .result
            .set(result)
            .map_err(|_| ExecServerError::Protocol("exec-server URL is not pending".to_string()))?;
        self.inner.result_available.send_replace(true);
        Ok(())
    }
}

impl std::fmt::Debug for ExecServerUrl {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.inner.result.get() {
            Some(Ok(url)) => std::fmt::Debug::fmt(url, formatter),
            Some(Err(_)) => formatter.write_str("<failed>"),
            None => formatter.write_str("<pending>"),
        }
    }
}

#[cfg(test)]
#[path = "exec_server_url_tests.rs"]
mod tests;
