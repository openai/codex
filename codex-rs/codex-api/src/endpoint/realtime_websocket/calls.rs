use crate::endpoint::realtime_websocket::methods::merge_request_headers;
use crate::endpoint::realtime_websocket::methods_common::session_update_session;
use crate::endpoint::realtime_websocket::protocol::RealtimeSessionConfig;
use crate::endpoint::realtime_websocket::protocol::SessionUpdateSession;
use crate::error::ApiError;
use crate::provider::Provider;
use codex_client::build_reqwest_client_with_custom_ca;
use http::HeaderMap;
use http::header::LOCATION;
use reqwest::multipart::Form;
use serde::Serialize;
use url::Url;

const REALTIME_CALLS_PATH: &str = "/v1/realtime/calls";

pub struct RealtimeCallCreateResponse {
    pub call_id: String,
    pub answer_sdp: String,
}

pub struct RealtimeCallClient {
    provider: Provider,
}

impl RealtimeCallClient {
    pub fn new(provider: Provider) -> Self {
        Self { provider }
    }

    pub async fn create(
        &self,
        config: &RealtimeSessionConfig,
        offer_sdp: String,
        extra_headers: HeaderMap,
        default_headers: HeaderMap,
    ) -> Result<RealtimeCallCreateResponse, ApiError> {
        let client = build_reqwest_client_with_custom_ca(reqwest::Client::builder())
            .map_err(|err| ApiError::Stream(format!("failed to configure realtime HTTP: {err}")))?;
        let url = realtime_calls_url(&self.provider.base_url)?;
        let headers = merge_request_headers(&self.provider.headers, extra_headers, default_headers);
        let response = client
            .post(url)
            .headers(headers)
            .multipart(session_form(config, offer_sdp)?)
            .send()
            .await
            .map_err(|err| ApiError::Stream(format!("failed to create realtime call: {err}")))?;
        let status = response.status();
        let headers = response.headers().clone();
        let answer_sdp = response.text().await.map_err(|err| {
            ApiError::Stream(format!("failed to read realtime call answer: {err}"))
        })?;
        if !status.is_success() {
            return Err(ApiError::Stream(format!(
                "realtime call failed with HTTP {status}: {answer_sdp}"
            )));
        }
        let call_id =
            call_id_from_location(headers.get(LOCATION).and_then(|value| value.to_str().ok()))?;
        Ok(RealtimeCallCreateResponse {
            call_id,
            answer_sdp,
        })
    }
}

fn realtime_calls_url(base_url: &str) -> Result<Url, ApiError> {
    let mut url =
        Url::parse(base_url).map_err(|err| ApiError::Stream(format!("invalid base URL: {err}")))?;
    match url.scheme() {
        "http" | "https" => {}
        "ws" => {
            let _ = url.set_scheme("http");
        }
        "wss" => {
            let _ = url.set_scheme("https");
        }
        scheme => {
            return Err(ApiError::Stream(format!(
                "unsupported realtime calls URL scheme: {scheme}"
            )));
        }
    }
    url.set_path(REALTIME_CALLS_PATH);
    url.set_query(None);
    Ok(url)
}

fn session_form(config: &RealtimeSessionConfig, offer_sdp: String) -> Result<Form, ApiError> {
    let session_json = serde_json::to_string(&session_payload(config)).map_err(|err| {
        ApiError::Stream(format!("failed to serialize realtime call session: {err}"))
    })?;
    Ok(Form::new()
        .text("sdp", offer_sdp)
        .text("session", session_json))
}

fn session_payload(config: &RealtimeSessionConfig) -> RealtimeCallSession {
    let session = session_update_session(
        config.event_parser,
        config.instructions.clone(),
        config.session_mode,
    );
    RealtimeCallSession {
        session,
        model: config.model.clone(),
    }
}

#[derive(Serialize)]
struct RealtimeCallSession {
    #[serde(flatten)]
    session: SessionUpdateSession,
    #[serde(skip_serializing_if = "Option::is_none")]
    model: Option<String>,
}

fn call_id_from_location(location: Option<&str>) -> Result<String, ApiError> {
    let Some(location) = location else {
        return Err(ApiError::Stream(
            "realtime call response missing Location header".to_string(),
        ));
    };
    let call_id = location
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .filter(|call_id| !call_id.is_empty())
        .ok_or_else(|| ApiError::Stream("invalid realtime call Location header".to_string()))?;
    Ok(call_id.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::endpoint::realtime_websocket::protocol::RealtimeEventParser;
    use crate::endpoint::realtime_websocket::protocol::RealtimeSessionMode;
    use pretty_assertions::assert_eq;

    #[test]
    fn realtime_calls_url_uses_calls_path() {
        assert_eq!(
            realtime_calls_url("wss://api.openai.com/v1/realtime")
                .expect("url")
                .as_str(),
            "https://api.openai.com/v1/realtime/calls"
        );
    }

    #[test]
    fn call_id_from_location_extracts_last_path_segment() {
        assert_eq!(
            call_id_from_location(Some("/v1/realtime/calls/rtc_123")).expect("call id"),
            "rtc_123"
        );
    }

    #[test]
    fn session_form_contains_offer_and_session() {
        let form = session_form(
            &RealtimeSessionConfig {
                instructions: "backend prompt".to_string(),
                model: Some("gpt-realtime".to_string()),
                session_id: Some("session".to_string()),
                event_parser: RealtimeEventParser::RealtimeV2,
                session_mode: RealtimeSessionMode::Conversational,
            },
            "v=0".to_string(),
        )
        .expect("form");
        assert!(!form.boundary().is_empty());
    }

    #[test]
    fn session_payload_includes_model() {
        let payload = serde_json::to_value(session_payload(&RealtimeSessionConfig {
            instructions: "backend prompt".to_string(),
            model: Some("gpt-realtime".to_string()),
            session_id: Some("session".to_string()),
            event_parser: RealtimeEventParser::RealtimeV2,
            session_mode: RealtimeSessionMode::Conversational,
        }))
        .expect("session payload");

        assert_eq!(payload["type"], "realtime");
        assert_eq!(payload["model"], "gpt-realtime");
        assert_eq!(payload["instructions"], "backend prompt");
    }
}
