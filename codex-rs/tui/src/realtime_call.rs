//! Negotiate media with the local user's credentials, sharing their runtime across TUI calls.

use crate::app_command::RealtimeSdp;
use crate::legacy_core::config::Config;
use codex_api::RealtimeCallClient;
use codex_api::RealtimeEventParser;
use codex_api::RealtimeSessionConfig;
use codex_api::RealtimeSessionMode;
use codex_api::build_realtime_thread_headers;
use codex_app_server_protocol::Thread;
use codex_config::config_toml::RealtimeWsMode;
use codex_features::Feature;
use codex_login::default_client::ClientRedirectPolicy;
use codex_login::default_client::add_originator_header;
use codex_login::default_client::originator;
use codex_model_provider::LocalProviderAuth;
use codex_model_provider::resolve_request;
use codex_prompts::prepare_realtime_backend_prompt;
use codex_protocol::protocol::RealtimeOutputModality;
use codex_protocol::protocol::RealtimeVoice;
use color_eyre::eyre::Result;
use color_eyre::eyre::ensure;
use http::HeaderValue;
use uuid::Uuid;

/// Keep the local credential owner and its runtime across voice calls.
#[derive(Default)]
pub(crate) struct Client {
    auth: LocalProviderAuth,
}

#[derive(Debug)]
pub(crate) struct CreatedCall {
    pub(crate) call_id: String,
    pub(crate) voice_session_id: Uuid,
    pub(crate) answer_sdp: RealtimeSdp,
}

impl Client {
    pub(crate) async fn create(
        &self,
        config: &Config,
        thread: &Thread,
        offer: String,
        voice: Option<RealtimeVoice>,
    ) -> Result<CreatedCall> {
        ensure!(
            config.realtime.session_type == RealtimeWsMode::Conversational,
            "AVAS realtime calls require conversational realtime"
        );
        let mut info = config.model_provider.clone();
        if let Some(url) = &config.experimental_realtime_webrtc_call_base_url {
            info.base_url = Some(url.clone());
        }
        let runtime_auth_mode = config
            .features
            .enabled(Feature::RuntimeAgentInstances)
            .then_some(config.runtime_auth_mode);
        let provider = self
            .auth
            .model_provider(info, config.auth_config(), runtime_auth_mode)
            .await?;
        let mut request = resolve_request(
            provider.as_ref(),
            /*scope*/ None,
            /*routing*/ None,
            config.http_client_factory(),
        )
        .await?;
        request.redirect_policy = ClientRedirectPolicy::Reject;
        let transport = request.http_transport("realtime/calls")?;
        let id = thread.id.clone();
        let voice_session_id = Uuid::new_v4();
        let mut headers = build_realtime_thread_headers(
            thread.session_id.clone(),
            id.clone(),
            thread.thread_source.clone().map(Into::into),
        );
        headers.insert("openai-alpha", HeaderValue::from_static("quicksilver=v2"));
        headers.insert("x-session-id", HeaderValue::from_str(&id)?);
        headers.insert(
            "voice-session-id",
            HeaderValue::from_str(&voice_session_id.to_string())?,
        );
        add_originator_header(&mut headers, &originator().value);
        let session = RealtimeSessionConfig {
            instructions: prepare_realtime_backend_prompt(
                /*prompt*/ None,
                config.experimental_realtime_ws_backend_prompt.clone(),
            ),
            initial_items: Vec::new(),
            delegation_ack_filler: None,
            model: Some(
                config
                    .experimental_realtime_ws_model
                    .clone()
                    .unwrap_or_else(|| "gpt-live-1.1-codex".to_string()),
            ),
            session_id: Some(id),
            event_parser: RealtimeEventParser::FramelessBidi,
            session_mode: RealtimeSessionMode::Conversational,
            output_modality: RealtimeOutputModality::Audio,
            voice,
        };
        let call = RealtimeCallClient::new(transport, request.api_provider, request.api_auth)
            .create_with_session_and_headers(offer, session, headers)
            .await?;
        Ok(CreatedCall {
            call_id: call.call_id,
            voice_session_id,
            answer_sdp: call.sdp.into(),
        })
    }
}

#[cfg(test)]
#[path = "realtime_call_tests.rs"]
mod tests;
