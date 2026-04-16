use super::enroll::REMOTE_CONTROL_ACCOUNT_ID_HEADER;
use super::enroll::RemoteControlConnectionAuth;
use super::enroll::RemoteControlEnrollment;
use super::enroll::format_headers;
use super::enroll::load_persisted_remote_control_enrollment;
use super::enroll::preview_remote_control_response_body;
use super::protocol::RemoteControlTarget;
use super::protocol::normalize_remote_control_url;
use super::websocket::load_remote_control_auth;
use codex_login::AuthManager;
use codex_login::default_client::build_reqwest_client;
use codex_state::StateRuntime;
use serde::Deserialize;
use serde::Serialize;
use std::io;
use std::io::ErrorKind;
use std::sync::Arc;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

const REMOTE_CONTROL_PAIRING_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RemoteControlPairingMode {
    Session,
    Server,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RemoteControlPairing {
    pub(crate) pairing_id: String,
    pub(crate) pairing_code: String,
    pub(crate) expires_at: i64,
    pub(crate) server_id: String,
    pub(crate) environment_id: String,
    pub(crate) mode: RemoteControlPairingMode,
    pub(crate) prompt: String,
}

#[derive(Debug, Serialize)]
struct StartRemoteControlPairingRequest<'a> {
    server_id: &'a str,
    environment_id: &'a str,
    mode: RemoteControlPairingMode,
}

#[derive(Debug, Deserialize)]
struct StartRemoteControlPairingResponse {
    pairing_id: String,
    pairing_code: String,
    expires_at: String,
    server_id: String,
    environment_id: String,
    mode: RemoteControlPairingMode,
    prompt: String,
}

pub(crate) async fn start_remote_control_pairing(
    remote_control_url: &str,
    state_db: Option<&StateRuntime>,
    auth_manager: &Arc<AuthManager>,
    app_server_client_name: Option<&str>,
    mode: RemoteControlPairingMode,
) -> io::Result<RemoteControlPairing> {
    let remote_control_target = normalize_remote_control_url(remote_control_url)?;
    let auth = load_remote_control_auth(auth_manager).await?;
    let enrollment = load_persisted_remote_control_enrollment(
        state_db,
        &remote_control_target,
        &auth.account_id,
        app_server_client_name,
    )
    .await
    .ok_or_else(|| {
        io::Error::new(
            ErrorKind::NotFound,
            "remote control server is not enrolled yet; enable remote control and wait for the server connection before starting pairing",
        )
    })?;

    post_start_remote_control_pairing(&remote_control_target, &auth, &enrollment, mode).await
}

async fn post_start_remote_control_pairing(
    remote_control_target: &RemoteControlTarget,
    auth: &RemoteControlConnectionAuth,
    enrollment: &RemoteControlEnrollment,
    mode: RemoteControlPairingMode,
) -> io::Result<RemoteControlPairing> {
    let pairing_start_url = &remote_control_target.pairing_start_url;
    let request = StartRemoteControlPairingRequest {
        server_id: &enrollment.server_id,
        environment_id: &enrollment.environment_id,
        mode,
    };
    let client = build_reqwest_client();
    let http_request = client
        .post(pairing_start_url)
        .timeout(REMOTE_CONTROL_PAIRING_TIMEOUT)
        .bearer_auth(&auth.bearer_token)
        .header(REMOTE_CONTROL_ACCOUNT_ID_HEADER, &auth.account_id)
        .json(&request);

    let response = http_request.send().await.map_err(|err| {
        io::Error::other(format!(
            "failed to start remote control pairing at `{pairing_start_url}`: {err}"
        ))
    })?;
    let headers = response.headers().clone();
    let status = response.status();
    let body = response.bytes().await.map_err(|err| {
        io::Error::other(format!(
            "failed to read remote control pairing response from `{pairing_start_url}`: {err}"
        ))
    })?;
    let body_preview = preview_remote_control_response_body(&body);
    if !status.is_success() {
        let headers_str = format_headers(&headers);
        let error_kind = if matches!(status.as_u16(), 401 | 403) {
            ErrorKind::PermissionDenied
        } else {
            ErrorKind::Other
        };
        return Err(io::Error::new(
            error_kind,
            format!(
                "remote control pairing failed at `{pairing_start_url}`: HTTP {status}, {headers_str}, body: {body_preview}"
            ),
        ));
    }

    let response =
        serde_json::from_slice::<StartRemoteControlPairingResponse>(&body).map_err(|err| {
            let headers_str = format_headers(&headers);
            io::Error::other(format!(
                "failed to parse remote control pairing response from `{pairing_start_url}`: HTTP {status}, {headers_str}, body: {body_preview}, decode error: {err}"
            ))
        })?;
    if response.server_id != enrollment.server_id
        || response.environment_id != enrollment.environment_id
        || response.mode != mode
    {
        return Err(io::Error::new(
            ErrorKind::InvalidData,
            "remote control pairing response did not match the active server enrollment",
        ));
    }
    let expires_at = OffsetDateTime::parse(&response.expires_at, &Rfc3339)
        .map_err(|err| io::Error::other(format!("invalid remote control pairing expiry: {err}")))?
        .unix_timestamp();

    Ok(RemoteControlPairing {
        pairing_id: response.pairing_id,
        pairing_code: response.pairing_code,
        expires_at,
        server_id: response.server_id,
        environment_id: response.environment_id,
        mode: response.mode,
        prompt: response.prompt,
    })
}
