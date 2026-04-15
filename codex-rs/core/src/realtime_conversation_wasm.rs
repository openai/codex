use std::sync::Arc;

use crate::codex::Session;
use crate::error::Result as CodexResult;
use codex_protocol::protocol::ConversationAudioParams;
use codex_protocol::protocol::ConversationStartParams;
use codex_protocol::protocol::ConversationTextParams;
use tokio::sync::Mutex;

#[derive(Default)]
pub(crate) struct RealtimeConversationManager {
    active_handoff: Mutex<Option<String>>,
}

impl RealtimeConversationManager {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) async fn running_state(&self) -> Option<()> {
        None
    }

    pub(crate) async fn handoff_out(&self, _output_text: String) -> CodexResult<()> {
        Ok(())
    }

    pub(crate) async fn handoff_complete(&self) -> CodexResult<()> {
        Ok(())
    }

    pub(crate) async fn active_handoff_id(&self) -> Option<String> {
        self.active_handoff.lock().await.clone()
    }

    pub(crate) async fn clear_active_handoff(&self) {
        *self.active_handoff.lock().await = None;
    }

    pub(crate) async fn shutdown(&self) -> CodexResult<()> {
        Ok(())
    }
}

pub(crate) async fn handle_start(
    _sess: &Arc<Session>,
    _sub_id: String,
    _params: ConversationStartParams,
) -> CodexResult<()> {
    Ok(())
}

pub(crate) async fn handle_audio(
    _sess: &Arc<Session>,
    _sub_id: String,
    _params: ConversationAudioParams,
) {
}

pub(crate) async fn handle_text(
    _sess: &Arc<Session>,
    _sub_id: String,
    _params: ConversationTextParams,
) {
}

pub(crate) async fn handle_close(_sess: &Arc<Session>, _sub_id: String) {}
