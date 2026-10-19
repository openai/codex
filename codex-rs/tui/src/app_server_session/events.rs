//! Retains the logical backend that produced a session event before UI dispatch.
//! Backend identity survives reconnects; it is not a transport generation or wire ID.

use crate::backend_id::BackendId;
use codex_app_server_client::AppServerEvent;

pub(crate) struct AppServerSessionEvent {
    pub(crate) backend_id: BackendId,
    pub(crate) event: AppServerEvent,
}
