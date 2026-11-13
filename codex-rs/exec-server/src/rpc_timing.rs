//! Carries RPC reader completion time back to the caller so tracing stays in the awaiting task.

use std::time::Instant;

use codex_exec_server_protocol::ProcessStartErrorData;
use serde::Deserialize;
use serde_json::Value;

use crate::protocol::EXEC_METHOD;
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
        let (outcome, start_error_code, start_error_kind) = match &self.result {
            Ok(_) => ("success", None, None),
            Err(RpcCallError::Server(error)) if method == EXEC_METHOD => (
                "error",
                Some(error.code),
                error
                    .data
                    .as_ref()
                    .and_then(|data| ProcessStartErrorData::deserialize(data).ok()),
            ),
            Err(RpcCallError::Server(_)) => ("error", None, None),
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
            process.start_error_code = start_error_code,
            process.start_error_kind = start_error_kind.as_ref().map(tracing::field::debug),
            reader_to_caller_ns = i64::try_from(self.completed_at.elapsed().as_nanos())
                .unwrap_or(i64::MAX),
        );
    }
}
