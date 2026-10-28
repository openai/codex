//! Isolates optional synchronous metrics collection from the HTTP/2 driver and
//! session admission. A stuck collector occupies at most one detached thread,
//! which is never joined during host or Tokio runtime shutdown.

use std::panic::AssertUnwindSafe;
use std::panic::catch_unwind;
use std::sync::Arc;

use codex_code_mode_protocol::CodeModeProcessMetrics;
use codex_code_mode_protocol::grpc;
use tokio::sync::Semaphore;
use tokio::sync::oneshot;
use tonic::Response;
use tonic::Status;

/// Collectors capture any resources they need before the host enters its sandbox.
/// They must be bounded, read-only observations that do not acquire execution
/// locks. Isolation covers unwinding panics and stalled collection, not process
/// aborts, resource exhaustion, or interference through shared state. A collector
/// that never returns disables further sampling for this host; it is not retried
/// on another thread, and its worker is not joined during Tokio runtime shutdown.
pub type CodeModeProcessMetricsCollector =
    Arc<dyn Fn() -> Result<CodeModeProcessMetrics, String> + Send + Sync>;

pub(crate) struct ProcessMetrics {
    collector: CodeModeProcessMetricsCollector,
    permits: Arc<Semaphore>,
}

impl ProcessMetrics {
    pub(crate) fn new(collector: CodeModeProcessMetricsCollector) -> Self {
        Self {
            collector,
            permits: Arc::new(Semaphore::new(/*permits*/ 1)),
        }
    }

    pub(crate) async fn sample(&self) -> Result<Response<grpc::GetProcessMetricsResponse>, Status> {
        let permit = self
            .permits
            .clone()
            .try_acquire_owned()
            .map_err(|_| Status::resource_exhausted("process metrics collector is busy"))?;
        let collector = self.collector.clone();
        let (sender, receiver) = oneshot::channel();
        // Do not use Tokio's blocking pool: runtime drop joins running blocking
        // tasks even after the request times out. Dropping this thread's handle
        // detaches it; the permit still bounds abandoned collection to one thread.
        std::thread::Builder::new()
            .name("code-mode-metrics".to_string())
            .spawn(move || {
                let result = catch_unwind(AssertUnwindSafe(|| collector()));
                drop(permit);
                let _ = sender.send(result);
            })
            .map_err(|_| Status::internal("process metrics worker could not start"))?;
        let snapshot = receiver
            .await
            .map_err(|_| Status::internal("process metrics worker ended"))?
            .map_err(|_| Status::internal("process metrics collector panicked"))?
            .map_err(|_| Status::internal("process metrics collection failed"))?;
        Ok(Response::new(grpc::GetProcessMetricsResponse {
            process_id: snapshot.process_id,
            process_start_time_seconds: snapshot.process_start_time_seconds,
            cpu_seconds_total: snapshot.cpu_seconds_total,
            resident_memory_bytes: snapshot.resident_memory_bytes,
        }))
    }
}
