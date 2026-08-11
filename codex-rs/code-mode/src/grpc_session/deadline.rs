use std::fmt::Display;
use std::future::Future;
use std::time::Duration;

use super::SessionInner;

const TRANSPORT_TIMEOUT: Duration = Duration::from_secs(60);
const MAX_ERROR_BYTES: usize = 512;

pub(super) async fn startup<T, E: Display>(
    operation: &str,
    request: impl Future<Output = Result<T, E>>,
) -> Result<T, String> {
    match enforce(operation, Duration::ZERO, request).await {
        Ok(result) => Ok(result),
        Err(RequestError::Failed(error)) => Err(failure(operation, error)),
        Err(RequestError::TimedOut(reason)) => Err(reason),
    }
}

pub(super) async fn request<T>(
    session: &SessionInner,
    operation: &str,
    runtime_timeout: Duration,
    request: impl Future<Output = Result<T, tonic::Status>>,
) -> Result<T, String> {
    let result = tokio::select! {
        biased;
        _ = session.stopped.cancelled() => {
            return Err("gRPC code-mode session closed".to_string());
        }
        result = enforce(operation, runtime_timeout, request) => result,
    };

    match result {
        Ok(value) => Ok(value),
        Err(RequestError::Failed(error)) => Err(failure(operation, error)),
        Err(RequestError::TimedOut(reason)) => {
            session.fail(reason.clone());
            Err(reason)
        }
    }
}

pub(super) fn failure(operation: &str, error: impl Display) -> String {
    let mut message = format!("gRPC code-mode {operation} failed: {error}");
    if message.len() > MAX_ERROR_BYTES {
        let boundary = message.floor_char_boundary(MAX_ERROR_BYTES - "...".len());
        message.truncate(boundary);
        message.push_str("...");
    }
    message
}

async fn enforce<T, E>(
    operation: &str,
    runtime_timeout: Duration,
    request: impl Future<Output = Result<T, E>>,
) -> Result<T, RequestError<E>> {
    let timeout = runtime_timeout.saturating_add(TRANSPORT_TIMEOUT);
    match tokio::time::timeout(timeout, request).await {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(error)) => Err(RequestError::Failed(error)),
        Err(_) => Err(RequestError::TimedOut(format!(
            "gRPC code-mode host timed out waiting for {operation} response"
        ))),
    }
}

enum RequestError<E> {
    Failed(E),
    TimedOut(String),
}

#[cfg(test)]
#[path = "deadline_tests.rs"]
mod tests;
