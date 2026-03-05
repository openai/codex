use crate::CodexAuth;
use crate::ModelProviderInfo;
use crate::auth::read_openai_api_key_from_env;
use crate::codex::Session;
use crate::error::CodexErr;
use crate::error::Result as CodexResult;
use codex_protocol::protocol::CodexErrorInfo;
use codex_protocol::protocol::ErrorEvent;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::RealtimeHandoffRequested;
use http::HeaderMap;
use http::HeaderValue;
use http::header::AUTHORIZATION;
use std::sync::Arc;

pub(super) fn realtime_text_from_handoff_request(
    handoff: &RealtimeHandoffRequested,
) -> Option<String> {
    let messages = handoff
        .messages
        .iter()
        .map(|message| message.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    (!messages.is_empty()).then_some(messages).or_else(|| {
        (!handoff.input_transcript.is_empty()).then(|| handoff.input_transcript.clone())
    })
}

pub(super) fn realtime_api_key(
    auth: Option<&CodexAuth>,
    provider: &ModelProviderInfo,
) -> CodexResult<String> {
    if let Some(api_key) = provider.api_key()? {
        return Ok(api_key);
    }

    if let Some(token) = provider.experimental_bearer_token.clone() {
        return Ok(token);
    }

    if let Some(api_key) = auth.and_then(CodexAuth::api_key) {
        return Ok(api_key.to_string());
    }

    // TODO(aibrahim): Remove this temporary fallback once realtime auth no longer
    // requires API key auth for ChatGPT/SIWC sessions.
    if provider.is_openai()
        && let Some(api_key) = read_openai_api_key_from_env()
    {
        return Ok(api_key);
    }

    Err(CodexErr::InvalidRequest(
        "realtime conversation requires API key auth".to_string(),
    ))
}

pub(super) fn realtime_request_headers(
    session_id: Option<&str>,
    api_key: &str,
) -> CodexResult<Option<HeaderMap>> {
    let mut headers = HeaderMap::new();

    if let Some(session_id) = session_id
        && let Ok(session_id) = HeaderValue::from_str(session_id)
    {
        headers.insert("x-session-id", session_id);
    }

    let auth_value = HeaderValue::from_str(&format!("Bearer {api_key}")).map_err(|err| {
        CodexErr::InvalidRequest(format!("invalid realtime api key header: {err}"))
    })?;
    headers.insert(AUTHORIZATION, auth_value);

    Ok(Some(headers))
}

pub(super) async fn send_conversation_error(
    sess: &Arc<Session>,
    sub_id: String,
    message: String,
    codex_error_info: CodexErrorInfo,
) {
    sess.send_event_raw(Event {
        id: sub_id,
        msg: EventMsg::Error(ErrorEvent {
            message,
            codex_error_info: Some(codex_error_info),
        }),
    })
    .await;
}
