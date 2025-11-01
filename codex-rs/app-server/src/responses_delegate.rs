use std::sync::Arc;

use codex_app_server_protocol::ResponsesApiCallParams;
use codex_app_server_protocol::ServerRequestPayload;
use tokio::task::JoinHandle;

use crate::outgoing_message::OutgoingMessageSender;

pub(crate) struct AppServerResponsesDelegate {
    outgoing: Arc<OutgoingMessageSender>,
}

impl AppServerResponsesDelegate {
    pub(crate) fn new(outgoing: Arc<OutgoingMessageSender>) -> Self {
        Self { outgoing }
    }
}

impl codex_core::responses_delegate::ResponsesHttpDelegate for AppServerResponsesDelegate {
    fn start_call(&self, params: ResponsesApiCallParams) {
        let outgoing = self.outgoing.clone();
        let call_id = params.call_id.clone();
        // Fire-and-forget: send the request and await the terminal JSON-RPC response.
        let _handle: JoinHandle<()> = tokio::spawn(async move {
            let rx = outgoing
                .send_request(ServerRequestPayload::ResponsesApiCall(params))
                .await;
            let _ = rx.await; // Ignore content; events are streamed separately.
            codex_core::responses_delegate::finish_call(&call_id);
        });
    }
}
