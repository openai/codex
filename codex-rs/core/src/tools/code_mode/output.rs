//! Code-mode response headers use the host measurement and completed handler timing.
//! Content is already truncated; only the bounded header changes at completion.

use std::time::Duration;

use codex_protocol::models::FunctionCallOutputContentItem;
use codex_protocol::models::ResponseInputItem;

use crate::tools::context::FunctionToolOutput;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;

pub(super) struct CodeModeToolOutput {
    output: FunctionToolOutput,
    status: String,
    host_duration: Option<Duration>,
}

impl CodeModeToolOutput {
    pub(super) fn new(
        mut output: FunctionToolOutput,
        status: String,
        wall_time: Duration,
        host_duration: Option<Duration>,
    ) -> Self {
        // Use the host-only header when overhead is hidden or host timing is unavailable.
        let wall_time_seconds = (wall_time.as_secs_f32() * 10.0).round() / 10.0;
        output.body.insert(
            /*index*/ 0,
            FunctionCallOutputContentItem::InputText {
                text: format!("{status}\nWall time {wall_time_seconds:.1} seconds\nOutput:\n"),
            },
        );
        Self {
            output,
            status,
            host_duration,
        }
    }
}

impl ToolOutput for CodeModeToolOutput {
    fn log_output(&self) -> String {
        self.output.log_output()
    }

    fn success_for_logging(&self) -> bool {
        self.output.success_for_logging()
    }

    fn set_handler_duration_ms(&mut self, handler_duration_ms: u64) {
        let Some(host_duration) = self.host_duration else {
            return;
        };
        let total_seconds = handler_duration_ms as f64 / 1_000.0;
        let host_seconds = host_duration.as_secs_f64();
        // Match the logged measurements before rounding. Millisecond quantization of the
        // outer measurement can legitimately produce a small negative difference.
        let overhead_seconds = total_seconds - host_seconds;
        let status = &self.status;
        // The constructor owns this slot; script text and media stay untouched.
        self.output.body[0] = FunctionCallOutputContentItem::InputText {
            text: format!(
                "{status}\nWall time {total_seconds:.3} seconds (code-mode {host_seconds:.3} seconds; overhead {overhead_seconds:.3} seconds)\nOutput:\n"
            ),
        };
    }

    fn to_response_item(&self, call_id: &str, payload: &ToolPayload) -> ResponseInputItem {
        self.output.to_response_item(call_id, payload)
    }
}

#[cfg(test)]
#[path = "output_tests.rs"]
mod tests;
