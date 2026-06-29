use std::collections::HashMap;

use codex_exec_server_protocol::EXEC_CLOSED_METHOD;
use codex_exec_server_protocol::JSONRPCMessage;
use codex_exec_server_protocol::RequestId;
use codex_protocol::protocol::W3cTraceContext;

/// Correlates return traffic with the trace carried by its originating request.
///
/// Responses do not repeat the request's W3C carrier, and process notifications
/// are correlated by `processId`. Keeping this state per virtual stream avoids
/// adding telemetry-only fields to the JSON-RPC wire protocol.
#[derive(Default)]
pub(super) struct NoiseTraceContext {
    requests: HashMap<RequestId, W3cTraceContext>,
    processes: HashMap<String, W3cTraceContext>,
}

impl NoiseTraceContext {
    pub(super) fn observe_request(&mut self, message: &JSONRPCMessage) {
        let JSONRPCMessage::Request(request) = message else {
            return;
        };
        let Some(trace) = request.trace.as_ref() else {
            return;
        };
        self.requests.insert(request.id.clone(), trace.clone());
        if let Some(process_id) = message_process_id(message) {
            self.processes
                .entry(process_id.to_string())
                .or_insert_with(|| trace.clone());
        }
    }

    pub(super) fn return_trace(&mut self, message: &JSONRPCMessage) -> Option<W3cTraceContext> {
        match message {
            JSONRPCMessage::Response(response) => self.requests.remove(&response.id),
            JSONRPCMessage::Error(error) => self.requests.remove(&error.id),
            JSONRPCMessage::Notification(notification) => {
                let process_id = message_process_id(message)?;
                let trace = self.processes.get(process_id).cloned();
                if notification.method == EXEC_CLOSED_METHOD {
                    self.processes.remove(process_id);
                }
                trace
            }
            JSONRPCMessage::Request(request) => request.trace.clone(),
        }
    }
}

fn message_process_id(message: &JSONRPCMessage) -> Option<&str> {
    let params = match message {
        JSONRPCMessage::Request(request) => request.params.as_ref(),
        JSONRPCMessage::Notification(notification) => notification.params.as_ref(),
        JSONRPCMessage::Response(_) | JSONRPCMessage::Error(_) => None,
    }?;
    params.get("processId")?.as_str()
}

#[cfg(test)]
#[path = "trace_context_tests.rs"]
mod tests;
