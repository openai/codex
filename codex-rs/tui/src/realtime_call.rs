//! Negotiate the TUI's media connection using the local user's provider and credentials.

use crate::app_command::RealtimeSdp;
use crate::legacy_core::config::Config;
use codex_api::RealtimeCallClient;
use codex_api::RealtimeEventParser;
use codex_api::RealtimeSessionConfig;
use codex_api::RealtimeSessionMode;
use codex_api::ReqwestTransport;
use codex_api::build_realtime_thread_headers;
use codex_app_server_protocol::Thread;
use codex_config::config_toml::RealtimeWsMode;
use codex_http_client::ClientRouteClass;
use codex_login::default_client::ClientRedirectPolicy;
use codex_login::default_client::add_originator_header;
use codex_login::default_client::create_client_for_route;
use codex_login::default_client::originator;
use codex_model_provider::create_model_provider_from_local_config;
use codex_model_provider::prepare_provider_request;
use codex_prompts::prepare_realtime_backend_prompt;
use codex_protocol::protocol::RealtimeOutputModality;
use codex_protocol::protocol::RealtimeVoice;
use color_eyre::eyre::Result;
use color_eyre::eyre::ensure;
use http::HeaderValue;

#[derive(Debug)]
pub(crate) struct CreatedCall {
    pub(crate) call_id: String,
    pub(crate) answer_sdp: RealtimeSdp,
}

pub(crate) async fn create(
    config: &Config,
    thread: &Thread,
    offer: String,
    voice: Option<RealtimeVoice>,
) -> Result<CreatedCall> {
    ensure!(
        config.realtime.session_type == RealtimeWsMode::Conversational,
        "AVAS realtime calls require conversational realtime"
    );
    let provider =
        create_model_provider_from_local_config(config.model_provider.clone(), config).await?;
    let (mut api_provider, resolved_auth) =
        prepare_provider_request(provider.as_ref(), provider.info().auth.is_none(), |_| {
            provider.request_provider_and_auth(/*scope*/ None)
        })
        .await?;
    if let Some(url) = &config.experimental_realtime_webrtc_call_base_url {
        api_provider.base_url.clone_from(url);
    }
    let client = create_client_for_route(
        &config.http_client_factory(),
        &api_provider.url_for_path("realtime/calls"),
        ClientRouteClass::Api,
        ClientRedirectPolicy::Reject,
    )?;
    let id = thread.id.clone();
    let mut headers = build_realtime_thread_headers(
        thread.session_id.clone(),
        id.clone(),
        thread.thread_source.clone().map(Into::into),
    );
    headers.insert("openai-alpha", HeaderValue::from_static("quicksilver=v2"));
    headers.insert("x-session-id", HeaderValue::from_str(&id)?);
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
    let call = RealtimeCallClient::new(
        ReqwestTransport::from_http_client(client),
        api_provider,
        resolved_auth.auth,
    )
    .create_with_session_and_headers(offer, session, headers)
    .await?;
    Ok(CreatedCall {
        call_id: call.call_id,
        answer_sdp: call.sdp.into(),
    })
}
