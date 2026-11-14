//! Serves persisted realtime history through the store without acquiring a runtime.

use super::ClientResponsePayload;
use super::JSONRPCErrorError;
use super::ThreadRequestProcessor;
use super::invalid_request;
use super::paginated_history_list_error;
use codex_app_server_protocol::ThreadRealtimeListParams;
use codex_protocol::ThreadId;
use codex_thread_store::ListRealtimeParams;

impl ThreadRequestProcessor {
    pub(crate) async fn thread_realtime_list(
        &self,
        params: ThreadRealtimeListParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        let thread_id = ThreadId::from_string(&params.thread_id)
            .map_err(|error| invalid_request(format!("invalid thread id: {error}")))?;
        let page = self
            .thread_store
            .list_realtime(ListRealtimeParams {
                thread_id,
                session_id: params.session_id,
                from_observed_at_ms: params.from_observed_at_ms,
                to_observed_at_ms: params.to_observed_at_ms,
                order: params.order.unwrap_or_default(),
                after_position: params.after_position,
                event_id: params.event_id,
                cursor: params.cursor,
                page_size: params.limit.unwrap_or(100) as usize,
                snapshot_end: params.snapshot_end,
                required_through_position: params.required_through_position,
            })
            .await
            .map_err(paginated_history_list_error)?;
        Ok(Some(page.into()))
    }
}
