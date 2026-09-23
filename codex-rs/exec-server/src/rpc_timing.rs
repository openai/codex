//! Carries RPC reader completion time back to the caller so tracing stays in the awaiting task.

use std::time::Instant;

use serde_json::Value;

use crate::rpc::RpcCallError;

pub(crate) struct RpcCompletion {
    pub(crate) result: Result<Value, RpcCallError>,
    completed_at: Instant,
}

impl RpcCompletion {
    pub(crate) fn new(result: Result<Value, RpcCallError>) -> Self {
        Self {
            result,
            completed_at: Instant::now(),
        }
    }

    pub(crate) fn record_receipt(&self, method: &str) {
        let outcome = match &self.result {
            Ok(_) => "success",
            Err(RpcCallError::Server(_)) => "error",
            Err(
                RpcCallError::Closed
                | RpcCallError::Json(_)
                | RpcCallError::TimedOut { .. }
                | RpcCallError::PendingRequestLimitExceeded { .. },
            ) => return,
        };
        tracing::event!(
            name: "codex.exec_server.response_received",
            target: "codex_otel.trace_safe",
            tracing::Level::INFO,
            event.name = "codex.exec_server.response_received",
            rpc.method = method,
            outcome,
            reader_to_caller_ns = i64::try_from(self.completed_at.elapsed().as_nanos())
                .unwrap_or(i64::MAX),
        );
    }
}
