use crate::config::RealtimeFeaturesConfig;
use crate::prompt::prepare_realtime_backend_prompt;
use codex_api::RealtimeEventParser;
use codex_api::RealtimeSessionConfig;
use codex_api::RealtimeSessionMode;
use codex_config::config_toml::RealtimeWsMode;
use codex_config::config_toml::RealtimeWsVersion;
use codex_login::CodexAuth;
use codex_login::read_openai_api_key_from_env;
use codex_model_provider_info::ModelProviderInfo;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result as CodexResult;
use codex_protocol::protocol::RealtimeOutputModality;
use codex_protocol::protocol::RealtimeVoice;
use codex_protocol::protocol::RealtimeVoicesList;
use http::HeaderMap;
use http::HeaderValue;
use http::header::AUTHORIZATION;

pub const DEFAULT_REALTIME_MODEL: &str = "gpt-realtime-1.5";

pub fn build_realtime_session_config(
    config: &RealtimeFeaturesConfig,
    prompt: Option<Option<String>>,
    session_id: Option<String>,
    output_modality: RealtimeOutputModality,
    voice: Option<RealtimeVoice>,
    startup_context: String,
) -> CodexResult<RealtimeSessionConfig> {
    let prompt = prepare_realtime_backend_prompt(prompt, config.websocket_backend_prompt.clone());
    let startup_context = config
        .websocket_startup_context
        .clone()
        .unwrap_or(startup_context);
    let prompt = match (prompt.is_empty(), startup_context.is_empty()) {
        (true, true) => String::new(),
        (true, false) => startup_context,
        (false, true) => prompt,
        (false, false) => format!("{prompt}\n\n{startup_context}"),
    };
    let model = Some(
        config
            .websocket_model
            .clone()
            .unwrap_or_else(|| DEFAULT_REALTIME_MODEL.to_string()),
    );
    let event_parser = match config.session.version {
        RealtimeWsVersion::V1 => RealtimeEventParser::V1,
        RealtimeWsVersion::V2 => RealtimeEventParser::RealtimeV2,
    };
    if config.session.version == RealtimeWsVersion::V1
        && matches!(output_modality, RealtimeOutputModality::Text)
    {
        return Err(CodexErr::InvalidRequest(
            "text realtime output modality requires realtime v2".to_string(),
        ));
    }
    let session_mode = match config.session.session_type {
        RealtimeWsMode::Conversational => RealtimeSessionMode::Conversational,
        RealtimeWsMode::Transcription => RealtimeSessionMode::Transcription,
    };
    let voice = voice
        .or(config.session.voice)
        .unwrap_or_else(|| default_realtime_voice(config.session.version));
    validate_realtime_voice(config.session.version, voice)?;
    Ok(RealtimeSessionConfig {
        instructions: prompt,
        model,
        session_id: Some(session_id.unwrap_or_default()),
        event_parser,
        session_mode,
        output_modality,
        voice,
    })
}

pub fn default_realtime_voice(version: RealtimeWsVersion) -> RealtimeVoice {
    let voices = RealtimeVoicesList::builtin();
    match version {
        RealtimeWsVersion::V1 => voices.default_v1,
        RealtimeWsVersion::V2 => voices.default_v2,
    }
}

pub fn validate_realtime_voice(
    version: RealtimeWsVersion,
    voice: RealtimeVoice,
) -> CodexResult<()> {
    let voices = RealtimeVoicesList::builtin();
    let allowed = match version {
        RealtimeWsVersion::V1 => &voices.v1,
        RealtimeWsVersion::V2 => &voices.v2,
    };
    if allowed.contains(&voice) {
        return Ok(());
    }

    let version = match version {
        RealtimeWsVersion::V1 => "v1",
        RealtimeWsVersion::V2 => "v2",
    };
    let allowed = allowed
        .iter()
        .map(|voice| voice.wire_name())
        .collect::<Vec<_>>()
        .join(", ");
    Err(CodexErr::InvalidRequest(format!(
        "realtime voice `{}` is not supported for {version}; supported voices: {allowed}",
        voice.wire_name()
    )))
}

pub fn realtime_api_key(
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

    if provider.is_openai()
        && let Some(api_key) = read_openai_api_key_from_env()
    {
        return Ok(api_key);
    }

    Err(CodexErr::InvalidRequest(
        "realtime conversation requires API key auth".to_string(),
    ))
}

pub fn realtime_request_headers(
    session_id: Option<&str>,
    api_key: Option<&str>,
) -> CodexResult<Option<HeaderMap>> {
    let mut headers = HeaderMap::new();

    if let Some(session_id) = session_id
        && let Ok(session_id) = HeaderValue::from_str(session_id)
    {
        headers.insert("x-session-id", session_id);
    }

    if let Some(api_key) = api_key {
        let auth_value = HeaderValue::from_str(&format!("Bearer {api_key}")).map_err(|err| {
            CodexErr::InvalidRequest(format!("invalid realtime api key header: {err}"))
        })?;
        headers.insert(AUTHORIZATION, auth_value);
    }

    Ok(Some(headers))
}
