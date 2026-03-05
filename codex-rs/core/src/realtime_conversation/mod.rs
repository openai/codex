mod common;
mod v1;
mod v2;

use crate::auth::AuthMode;
use crate::codex::Session;
use crate::error::CodexErr;
use crate::error::Result as CodexResult;
use crate::features::RealtimeVoiceMode;
use async_channel::Receiver;
use codex_api::Provider as ApiProvider;
use codex_api::RealtimeApiMode;
use codex_api::RealtimeAudioFrame;
use codex_api::RealtimeEvent;
use codex_protocol::protocol::CodexErrorInfo;
use codex_protocol::protocol::ConversationAudioParams;
use codex_protocol::protocol::ConversationStartParams;
use codex_protocol::protocol::ConversationTextParams;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::RealtimeConversationClosedEvent;
use codex_protocol::protocol::RealtimeConversationRealtimeEvent;
use codex_protocol::protocol::RealtimeConversationStartedEvent;
use http::HeaderMap;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use tokio::sync::Mutex;
use tracing::debug;
use tracing::error;
use tracing::info;

const DEFAULT_REALTIME_MODEL: &str = "gpt-realtime-1.5";

pub(crate) struct RealtimeConversationManager {
    state: Mutex<Option<ConversationState>>,
}

enum ConversationState {
    V1(v1::ConversationState),
    V2(v2::ConversationState),
}

impl ConversationState {
    fn is_running(&self) -> bool {
        match self {
            Self::V1(state) => state.is_running(),
            Self::V2(state) => state.is_running(),
        }
    }

    async fn audio_in(&self, frame: RealtimeAudioFrame) -> CodexResult<()> {
        match self {
            Self::V1(state) => state.audio_in(frame).await,
            Self::V2(state) => state.audio_in(frame).await,
        }
    }

    async fn text_in(&self, text: String) -> CodexResult<()> {
        match self {
            Self::V1(state) => state.text_in(text).await,
            Self::V2(state) => state.text_in(text).await,
        }
    }

    async fn handoff_out(&self, output_text: String) -> CodexResult<()> {
        match self {
            Self::V1(state) => state.handoff_out(output_text).await,
            Self::V2(state) => state.handoff_out(output_text).await,
        }
    }

    async fn handoff_complete(&self) -> CodexResult<()> {
        match self {
            Self::V1(state) => state.handoff_complete().await,
            Self::V2(state) => state.handoff_complete().await,
        }
    }

    async fn active_handoff_id(&self) -> Option<String> {
        match self {
            Self::V1(state) => state.active_handoff_id().await,
            Self::V2(state) => state.active_handoff_id().await,
        }
    }

    async fn clear_active_handoff(&self) {
        match self {
            Self::V1(state) => state.clear_active_handoff().await,
            Self::V2(state) => state.clear_active_handoff().await,
        }
    }

    async fn shutdown(self) {
        match self {
            Self::V1(state) => state.shutdown().await,
            Self::V2(state) => state.shutdown().await,
        }
    }

    fn realtime_active(&self) -> Arc<AtomicBool> {
        match self {
            Self::V1(state) => state.realtime_active(),
            Self::V2(state) => state.realtime_active(),
        }
    }
}

impl RealtimeConversationManager {
    pub(crate) fn new() -> Self {
        Self {
            state: Mutex::new(None),
        }
    }

    pub(crate) async fn running_state(&self) -> Option<()> {
        let state = self.state.lock().await;
        state
            .as_ref()
            .and_then(|state| state.is_running().then_some(()))
    }

    pub(crate) async fn start(
        &self,
        api_provider: ApiProvider,
        extra_headers: Option<HeaderMap>,
        prompt: String,
        model: Option<String>,
        mode: RealtimeApiMode,
        session_id: Option<String>,
    ) -> CodexResult<(Receiver<RealtimeEvent>, Arc<AtomicBool>)> {
        let previous_state = {
            let mut guard = self.state.lock().await;
            guard.take()
        };
        if let Some(state) = previous_state {
            state.shutdown().await;
        }

        let (state, events_rx) = match mode {
            RealtimeApiMode::V1 => {
                let (state, events_rx) =
                    v1::start(api_provider, extra_headers, prompt, model, session_id).await?;
                (ConversationState::V1(state), events_rx)
            }
            RealtimeApiMode::V2 => {
                let (state, events_rx) =
                    v2::start(api_provider, extra_headers, prompt, model, session_id).await?;
                (ConversationState::V2(state), events_rx)
            }
        };

        let realtime_active = state.realtime_active();
        let mut guard = self.state.lock().await;
        *guard = Some(state);
        Ok((events_rx, realtime_active))
    }

    pub(crate) async fn audio_in(&self, frame: RealtimeAudioFrame) -> CodexResult<()> {
        let guard = self.state.lock().await;
        let Some(state) = guard.as_ref() else {
            return Err(CodexErr::InvalidRequest(
                "conversation is not running".to_string(),
            ));
        };
        state.audio_in(frame).await
    }

    pub(crate) async fn text_in(&self, text: String) -> CodexResult<()> {
        let guard = self.state.lock().await;
        let Some(state) = guard.as_ref() else {
            return Err(CodexErr::InvalidRequest(
                "conversation is not running".to_string(),
            ));
        };
        state.text_in(text).await
    }

    pub(crate) async fn handoff_out(&self, output_text: String) -> CodexResult<()> {
        let guard = self.state.lock().await;
        let Some(state) = guard.as_ref() else {
            return Err(CodexErr::InvalidRequest(
                "conversation is not running".to_string(),
            ));
        };
        state.handoff_out(output_text).await
    }

    pub(crate) async fn handoff_complete(&self) -> CodexResult<()> {
        let guard = self.state.lock().await;
        let Some(state) = guard.as_ref() else {
            return Ok(());
        };
        state.handoff_complete().await
    }

    pub(crate) async fn active_handoff_id(&self) -> Option<String> {
        let guard = self.state.lock().await;
        let state = guard.as_ref()?;
        state.active_handoff_id().await
    }

    pub(crate) async fn clear_active_handoff(&self) {
        let guard = self.state.lock().await;
        if let Some(state) = guard.as_ref() {
            state.clear_active_handoff().await;
        }
    }

    pub(crate) async fn shutdown(&self) -> CodexResult<()> {
        let state = {
            let mut guard = self.state.lock().await;
            guard.take()
        };

        if let Some(state) = state {
            state.shutdown().await;
        }
        Ok(())
    }
}

pub(crate) async fn handle_start(
    sess: &Arc<Session>,
    sub_id: String,
    params: ConversationStartParams,
) -> CodexResult<()> {
    let provider = sess.provider().await;
    let auth = sess.services.auth_manager.auth().await;
    let realtime_api_key = common::realtime_api_key(auth.as_ref(), &provider)?;
    let mut api_provider = provider.to_api_provider(Some(AuthMode::ApiKey))?;
    let config = sess.get_config().await;
    if let Some(realtime_ws_base_url) = &config.experimental_realtime_ws_base_url {
        api_provider.base_url = realtime_ws_base_url.clone();
    }
    let prompt = config
        .experimental_realtime_ws_backend_prompt
        .clone()
        .unwrap_or(params.prompt);
    let mode = config
        .features
        .realtime_voice_mode()
        .unwrap_or(RealtimeVoiceMode::V2);
    let (realtime_api_mode, model) = match mode {
        RealtimeVoiceMode::V1 => (
            RealtimeApiMode::V1,
            config.experimental_realtime_ws_model.clone(),
        ),
        RealtimeVoiceMode::V2 => (
            RealtimeApiMode::V2,
            Some(DEFAULT_REALTIME_MODEL.to_string()),
        ),
    };

    let requested_session_id = params
        .session_id
        .or_else(|| Some(sess.conversation_id.to_string()));
    let extra_headers = common::realtime_request_headers(
        requested_session_id.as_deref(),
        realtime_api_key.as_str(),
    )?;
    info!("starting realtime conversation");
    let (events_rx, realtime_active) = match sess
        .conversation
        .start(
            api_provider,
            extra_headers,
            prompt,
            model,
            realtime_api_mode,
            requested_session_id.clone(),
        )
        .await
    {
        Ok(events_rx) => events_rx,
        Err(err) => {
            error!("failed to start realtime conversation: {err}");
            common::send_conversation_error(sess, sub_id, err.to_string(), CodexErrorInfo::Other)
                .await;
            return Ok(());
        }
    };

    info!("realtime conversation started");

    sess.send_event_raw(Event {
        id: sub_id.clone(),
        msg: EventMsg::RealtimeConversationStarted(RealtimeConversationStartedEvent {
            session_id: requested_session_id,
        }),
    })
    .await;

    let sess_clone = Arc::clone(sess);
    tokio::spawn(async move {
        let ev = |msg| Event {
            id: sub_id.clone(),
            msg,
        };
        while let Ok(event) = events_rx.recv().await {
            debug!(conversation_id = %sess_clone.conversation_id, "received realtime conversation event");
            if let RealtimeEvent::HandoffRequested(handoff) = &event
                && let Some(text) = common::realtime_text_from_handoff_request(handoff)
            {
                debug!(text = %text, "[realtime-text] realtime conversation text output");
                let sess_for_routed_text = Arc::clone(&sess_clone);
                sess_for_routed_text.route_realtime_text_input(text).await;
            }
            sess_clone
                .send_event_raw(ev(EventMsg::RealtimeConversationRealtime(
                    RealtimeConversationRealtimeEvent {
                        payload: event.clone(),
                    },
                )))
                .await;
        }
        if realtime_active.swap(false, Ordering::Relaxed) {
            info!("realtime conversation transport closed");
            sess_clone
                .send_event_raw(ev(EventMsg::RealtimeConversationClosed(
                    RealtimeConversationClosedEvent {
                        reason: Some("transport_closed".to_string()),
                    },
                )))
                .await;
        }
    });

    Ok(())
}

pub(crate) async fn handle_audio(
    sess: &Arc<Session>,
    sub_id: String,
    params: ConversationAudioParams,
) {
    if let Err(err) = sess.conversation.audio_in(params.frame).await {
        error!("failed to append realtime audio: {err}");
        common::send_conversation_error(sess, sub_id, err.to_string(), CodexErrorInfo::BadRequest)
            .await;
    }
}

pub(crate) async fn handle_text(
    sess: &Arc<Session>,
    sub_id: String,
    params: ConversationTextParams,
) {
    debug!(text = %params.text, "[realtime-text] appending realtime conversation text input");

    if let Err(err) = sess.conversation.text_in(params.text).await {
        error!("failed to append realtime text: {err}");
        common::send_conversation_error(sess, sub_id, err.to_string(), CodexErrorInfo::BadRequest)
            .await;
    }
}

pub(crate) async fn handle_close(sess: &Arc<Session>, sub_id: String) {
    match sess.conversation.shutdown().await {
        Ok(()) => {
            sess.send_event_raw(Event {
                id: sub_id,
                msg: EventMsg::RealtimeConversationClosed(RealtimeConversationClosedEvent {
                    reason: Some("requested".to_string()),
                }),
            })
            .await;
        }
        Err(err) => {
            common::send_conversation_error(sess, sub_id, err.to_string(), CodexErrorInfo::Other)
                .await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::common::realtime_text_from_handoff_request;
    use codex_protocol::protocol::RealtimeHandoffMessage;
    use codex_protocol::protocol::RealtimeHandoffRequested;
    use pretty_assertions::assert_eq;

    #[test]
    fn extracts_text_from_handoff_request_messages() {
        let handoff = RealtimeHandoffRequested {
            handoff_id: "handoff_1".to_string(),
            item_id: "item_1".to_string(),
            input_transcript: "ignored".to_string(),
            messages: vec![
                RealtimeHandoffMessage {
                    role: "user".to_string(),
                    text: "hello".to_string(),
                },
                RealtimeHandoffMessage {
                    role: "assistant".to_string(),
                    text: "hi there".to_string(),
                },
            ],
        };
        assert_eq!(
            realtime_text_from_handoff_request(&handoff),
            Some("hello\nhi there".to_string())
        );
    }

    #[test]
    fn extracts_text_from_handoff_request_input_transcript_if_messages_missing() {
        let handoff = RealtimeHandoffRequested {
            handoff_id: "handoff_1".to_string(),
            item_id: "item_1".to_string(),
            input_transcript: "ignored".to_string(),
            messages: vec![],
        };
        assert_eq!(
            realtime_text_from_handoff_request(&handoff),
            Some("ignored".to_string())
        );
    }

    #[test]
    fn ignores_empty_handoff_request_input_transcript() {
        let handoff = RealtimeHandoffRequested {
            handoff_id: "handoff_1".to_string(),
            item_id: "item_1".to_string(),
            input_transcript: String::new(),
            messages: vec![],
        };
        assert_eq!(realtime_text_from_handoff_request(&handoff), None);
    }
}
