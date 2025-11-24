use super::Handle;

use crate::runtime::Dump;

impl Handle {
    pub(crate) async fn dump(&self) -> Dump {
        let trace_status = &self.shared.trace_status;

        // If a dump is in progress, block.
        trace_status.start_trace_request(&self).await;

        let result = loop {
            if let Some(result) = trace_status.take_result() {
                break result;
            } else {
                self.notify_all();
                trace_status.result_ready.notified().await;
            }
        };

        // Allow other queued dumps to proceed.
        trace_status.end_trace_request(&self).await;

        result
    }
}
