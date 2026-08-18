use std::future::Future;
use std::pin::pin;
use std::sync::Arc;
use std::sync::OnceLock;
use std::time::Duration;
use std::time::Instant;

/// Transport metadata available while cleaning up an abandoned HTTP policy request.
/// This does not cancel the approval reviewer or change its decision.
#[derive(Clone, Debug, Default)]
pub struct NetworkRequestDisconnect(Arc<OnceLock<Duration>>);

impl NetworkRequestDisconnect {
    pub fn elapsed(&self) -> Option<Duration> {
        self.0.get().copied()
    }

    pub(crate) async fn track_http_request<F: Future>(
        &self,
        started_at: Instant,
        future: F,
    ) -> F::Output {
        // Publish the cause before dropping the policy future: its cleanup may
        // immediately finish the owning tool call.
        let mut future = pin!(future);
        let mut guard = HttpRequestGuard(Some((self, started_at)));
        let result = future.as_mut().await;
        guard.0 = None;
        result
    }
}

struct HttpRequestGuard<'a>(Option<(&'a NetworkRequestDisconnect, Instant)>);

impl Drop for HttpRequestGuard<'_> {
    fn drop(&mut self) {
        if let Some((disconnect, started_at)) = self.0 {
            let _ = disconnect.0.set(started_at.elapsed());
        }
    }
}

#[cfg(test)]
#[path = "request_disconnect_tests.rs"]
mod tests;
