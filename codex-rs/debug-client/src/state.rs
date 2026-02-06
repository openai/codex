use std::collections::HashMap;

use codex_app_server_protocol::RequestId;

#[derive(Debug, Default)]
pub struct State {
    pub pending: HashMap<RequestId, PendingRequest>,
    pub thread_id: Option<String>,
    pub known_threads: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PendingRequest {
    Start,
    Resume,
    List,
}

#[derive(Debug, Clone)]
pub enum ReaderEvent {
    ThreadReady {
        thread_id: String,
    },
    ThreadList {
        thread_ids: Vec<String>,
        next_cursor: Option<String>,
    },
}
