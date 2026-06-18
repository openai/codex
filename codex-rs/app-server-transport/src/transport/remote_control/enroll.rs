use super::auth::RemoteControlConnectionAuth;
use super::pairing_unavailable_error;
use super::protocol::EnrollRemoteServerRequest;
use super::protocol::EnrollRemoteServerResponse;
use super::protocol::RefreshRemoteServerRequest;
use super::protocol::RemoteControlPairingStatusRequest;
use super::protocol::RemoteControlPairingStatusResponse as BackendRemoteControlPairingStatusResponse;
use super::protocol::RemoteControlTarget;
use super::protocol::StartRemoteControlPairingRequest;
use super::protocol::StartRemoteControlPairingResponse;
use axum::http::HeaderMap;
use codex_app_server_protocol::RemoteControlPairingStartResponse;
use codex_app_server_protocol::RemoteControlPairingStatusResponse;
use codex_login::default_client::build_reqwest_client;
use codex_state::RemoteControlEnrollmentRecord;
use codex_state::StateRuntime;
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::io;
use std::io::ErrorKind;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use tracing::info;
use tracing::warn;

const REMOTE_CONTROL_ENROLL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);
const REMOTE_CONTROL_PAIRING_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);
const REMOTE_CONTROL_RESPONSE_BODY_MAX_BYTES: usize = 4096;
const REMOTE_CONTROL_SERVER_TOKEN_REFRESH_SKEW_SECS: i64 = 30;

const REQUEST_ID_HEADER: &str = "x-request-id";
const OAI_REQUEST_ID_HEADER: &str = "x-oai-request-id";
const CF_RAY_HEADER: &str = "cf-ray";
pub(super) const REMOTE_CONTROL_ACCOUNT_ID_HEADER: &str = "chatgpt-account-id";
pub(super) const REMOTE_CONTROL_INSTALLATION_ID_HEADER: &str = "x-codex-installation-id";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RemoteControlEnrollment {
    pub(super) remote_control_target: RemoteControlTarget,
    pub(super) account_id: String,
    pub(super) environment_id: String,
    pub(super) server_id: String,
    pub(super) server_name: String,
    pub(super) remote_control_token: Option<String>,
    pub(super) expires_at: Option<OffsetDateTime>,
}

impl RemoteControlEnrollment {
    pub(super) async fn start_pairing(
        &self,
        request: StartRemoteControlPairingRequest,
    ) -> io::Result<RemoteControlPairingStartResponse> {
        if self.should_refresh_server_token() {
            return Err(pairing_unavailable_error());
        }
        let remote_control_token = self
            .remote_control_token
            .as_deref()
            .ok_or_else(pairing_unavailable_error)?;

        let response = build_reqwest_client()
            .post(&self.remote_control_target.pair_url)
            .timeout(REMOTE_CONTROL_PAIRING_TIMEOUT)
            .bearer_auth(remote_control_token)
            .json(&request)
            .send()
            .await
            .map_err(|err| {
                io::Error::other(format!(
                    "failed to start remote control pairing at `{}`: {err}",
                    self.remote_control_target.pair_url
                ))
            })?;
        let headers = response.headers().clone();
        let status = response.status();
        let body = response.bytes().await.map_err(|err| {
            io::Error::other(format!(
                "failed to read remote control pairing response from `{}`: {err}",
                self.remote_control_target.pair_url
            ))
        })?;
        let body_preview = preview_remote_control_response_body(&body);
        if !status.is_success() {
            let error_kind = match status.as_u16() {
                401 | 403 => ErrorKind::PermissionDenied,
                404 => ErrorKind::NotFound,
                _ => ErrorKind::Other,
            };
            return Err(io::Error::new(
                error_kind,
                format!(
                    "remote control pairing failed at `{}`: HTTP {status}, {}, body: {body_preview}",
                    self.remote_control_target.pair_url,
                    format_headers(&headers)
                ),
            ));
        }

        let pairing = serde_json::from_slice::<StartRemoteControlPairingResponse>(&body).map_err(
            |err| {
                io::Error::other(format!(
                    "failed to parse remote control pairing response from `{}`: HTTP {status}, {}, body: {body_preview}, decode error: {err}",
                    self.remote_control_target.pair_url,
                    format_headers(&headers)
                ))
            },
        )?;
        let StartRemoteControlPairingResponse {
            pairing_code,
            manual_pairing_code,
            server_id,
            environment_id,
            expires_at,
        } = pairing;
        if server_id != self.server_id || environment_id != self.environment_id {
            return Err(io::Error::other(format!(
                "remote control pairing returned mismatched enrollment: expected server_id={}, environment_id={}; got server_id={}, environment_id={}",
                self.server_id, self.environment_id, server_id, environment_id
            )));
        }
        let expires_at = OffsetDateTime::parse(&expires_at, &Rfc3339)
            .map_err(|err| {
                io::Error::new(
                    ErrorKind::InvalidData,
                    format!(
                        "failed to parse remote control pairing response from `{}`: HTTP {status}, {}, body: {body_preview}, expires_at parse error: {err}",
                        self.remote_control_target.pair_url,
                        format_headers(&headers)
                    ),
                )
            })?
            .unix_timestamp();

        Ok(RemoteControlPairingStartResponse {
            pairing_code,
            manual_pairing_code,
            environment_id,
            expires_at,
        })
    }

    pub(super) async fn pairing_status(
        &self,
        request: RemoteControlPairingStatusRequest,
    ) -> io::Result<RemoteControlPairingStatusResponse> {
        if self.should_refresh_server_token() {
            return Err(pairing_unavailable_error());
        }
        let remote_control_token = self
            .remote_control_token
            .as_deref()
            .ok_or_else(pairing_unavailable_error)?;

        let response = build_reqwest_client()
            .post(&self.remote_control_target.pair_status_url)
            .timeout(REMOTE_CONTROL_PAIRING_TIMEOUT)
            .bearer_auth(remote_control_token)
            .json(&request)
            .send()
            .await
            .map_err(|err| {
                io::Error::other(format!(
                    "failed to check remote control pairing status at `{}`: {err}",
                    self.remote_control_target.pair_status_url
                ))
            })?;
        let headers = response.headers().clone();
        let status = response.status();
        let body = response.bytes().await.map_err(|err| {
            io::Error::other(format!(
                "failed to read remote control pairing status response from `{}`: {err}",
                self.remote_control_target.pair_status_url
            ))
        })?;
        let body_preview = preview_remote_control_response_body(&body);
        if !status.is_success() {
            let error_kind = match status.as_u16() {
                401 | 403 => ErrorKind::PermissionDenied,
                404 | 410 => ErrorKind::InvalidInput,
                _ => ErrorKind::Other,
            };
            return Err(io::Error::new(
                error_kind,
                format!(
                    "remote control pairing status failed at `{}`: HTTP {status}, {}, body: {body_preview}",
                    self.remote_control_target.pair_status_url,
                    format_headers(&headers)
                ),
            ));
        }

        let response = serde_json::from_slice::<BackendRemoteControlPairingStatusResponse>(&body)
            .map_err(|err| {
                io::Error::other(format!(
                    "failed to parse remote control pairing status response from `{}`: HTTP {status}, {}, body: {body_preview}, decode error: {err}",
                    self.remote_control_target.pair_status_url,
                    format_headers(&headers)
                ))
            })?;
        Ok(RemoteControlPairingStatusResponse {
            claimed: response.claimed,
        })
    }

    pub(super) fn should_refresh_server_token(&self) -> bool {
        self.remote_control_token.is_none()
            || self.expires_at.is_none_or(|expires_at| {
                expires_at.unix_timestamp()
                    <= OffsetDateTime::now_utc().unix_timestamp()
                        + REMOTE_CONTROL_SERVER_TOKEN_REFRESH_SKEW_SECS
            })
    }

    pub(super) fn clear_server_token(&mut self) {
        self.remote_control_token = None;
        self.expires_at = None;
    }
}

pub(super) async fn load_persisted_remote_control_enrollment(
    state_db: Option<&StateRuntime>,
    remote_control_target: &RemoteControlTarget,
    account_id: &str,
    app_server_client_name: Option<&str>,
) -> io::Result<Option<RemoteControlEnrollment>> {
    let Some(state_db) = state_db else {
        return Err(io::Error::new(
            ErrorKind::NotFound,
            format!(
                "remote control enrollment cache unavailable because sqlite state db is disabled: websocket_url={}, account_id={}, app_server_client_name={:?}",
                remote_control_target.websocket_url, account_id, app_server_client_name
            ),
        ));
    };
    let enrollment = match state_db
        .get_remote_control_enrollment(
            &remote_control_target.websocket_url,
            account_id,
            app_server_client_name,
        )
        .await
    {
        Ok(enrollment) => enrollment,
        Err(err) => {
            warn!(
                "failed to load persisted remote control enrollment: websocket_url={}, account_id={}, app_server_client_name={:?}, err={err}",
                remote_control_target.websocket_url, account_id, app_server_client_name
            );
            return Err(io::Error::other(err));
        }
    };

    match enrollment {
        Some(enrollment) => {
            info!(
                "reusing persisted remote control enrollment: websocket_url={}, account_id={}, app_server_client_name={:?}, server_id={}, environment_id={}",
                remote_control_target.websocket_url,
                account_id,
                app_server_client_name,
                enrollment.server_id,
                enrollment.environment_id
            );
            Ok(Some(RemoteControlEnrollment {
                remote_control_target: remote_control_target.clone(),
                account_id: enrollment.account_id,
                environment_id: enrollment.environment_id,
                server_id: enrollment.server_id,
                server_name: enrollment.server_name,
                remote_control_token: None,
                expires_at: None,
            }))
        }
        None => {
            info!(
                "no persisted remote control enrollment found: websocket_url={}, account_id={}, app_server_client_name={:?}",
                remote_control_target.websocket_url, account_id, app_server_client_name
            );
            Ok(None)
        }
    }
}

pub(super) async fn update_persisted_remote_control_enrollment(
    state_db: Option<&StateRuntime>,
    remote_control_target: &RemoteControlTarget,
    account_id: &str,
    app_server_client_name: Option<&str>,
    enrollment: Option<&RemoteControlEnrollment>,
    remote_control_enabled: Option<bool>,
) -> io::Result<()> {
    let Some(state_db) = state_db else {
        return Err(io::Error::new(
            ErrorKind::NotFound,
            format!(
                "remote control enrollment persistence unavailable because sqlite state db is disabled: websocket_url={}, account_id={}, app_server_client_name={:?}, has_enrollment={}",
                remote_control_target.websocket_url,
                account_id,
                app_server_client_name,
                enrollment.is_some()
            ),
        ));
    };
    if let &Some(enrollment) = &enrollment
        && enrollment.account_id != account_id
    {
        return Err(io::Error::other(format!(
            "enrollment account_id does not match expected account_id `{account_id}`"
        )));
    }

    if let Some(enrollment) = enrollment {
        state_db
            .upsert_remote_control_enrollment(&RemoteControlEnrollmentRecord {
                websocket_url: remote_control_target.websocket_url.clone(),
                account_id: account_id.to_string(),
                app_server_client_name: app_server_client_name.map(str::to_string),
                server_id: enrollment.server_id.clone(),
                environment_id: enrollment.environment_id.clone(),
                server_name: enrollment.server_name.clone(),
                remote_control_enabled,
            })
            .await
            .map_err(io::Error::other)?;
        info!(
            "persisted remote control enrollment: websocket_url={}, account_id={}, app_server_client_name={:?}, server_id={}, environment_id={}",
            remote_control_target.websocket_url,
            account_id,
            app_server_client_name,
            enrollment.server_id,
            enrollment.environment_id
        );
        Ok(())
    } else {
        let rows_affected = state_db
            .delete_remote_control_enrollment(
                &remote_control_target.websocket_url,
                account_id,
                app_server_client_name,
            )
            .await
            .map_err(io::Error::other)?;
        info!(
            "cleared persisted remote control enrollment: websocket_url={}, account_id={}, app_server_client_name={:?}, rows_affected={rows_affected}",
            remote_control_target.websocket_url, account_id, app_server_client_name
        );
        Ok(())
    }
}

pub(crate) fn preview_remote_control_response_body(body: &[u8]) -> String {
    let body = String::from_utf8_lossy(body);
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return "<empty>".to_string();
    }
    let redacted = redact_remote_control_response_body(trimmed);
    if redacted.len() <= REMOTE_CONTROL_RESPONSE_BODY_MAX_BYTES {
        return redacted;
    }

    let mut cut = REMOTE_CONTROL_RESPONSE_BODY_MAX_BYTES;
    while !redacted.is_char_boundary(cut) {
        cut = cut.saturating_sub(1);
    }
    let mut truncated = redacted[..cut].to_string();
    truncated.push_str("...");
    truncated
}

fn redact_remote_control_response_body(body: &str) -> String {
    let Ok(mut body_json) = serde_json::from_str::<serde_json::Value>(body) else {
        return body.to_string();
    };
    let Some(body_object) = body_json.as_object_mut() else {
        return body.to_string();
    };
    for sensitive_field in [
        "remote_control_token",
        "pairing_code",
        "manual_pairing_code",
    ] {
        if let Some(value) = body_object.get_mut(sensitive_field) {
            *value = serde_json::Value::String("<redacted>".to_string());
        }
    }
    body_json.to_string()
}

pub(crate) fn format_headers(headers: &HeaderMap) -> String {
    let request_id_str = headers
        .get(REQUEST_ID_HEADER)
        .or_else(|| headers.get(OAI_REQUEST_ID_HEADER))
        .map(|value| value.to_str().unwrap_or("<invalid utf-8>").to_owned())
        .unwrap_or_else(|| "<none>".to_owned());
    let cf_ray_str = headers
        .get(CF_RAY_HEADER)
        .map(|value| value.to_str().unwrap_or("<invalid utf-8>").to_owned())
        .unwrap_or_else(|| "<none>".to_owned());
    format!("request-id: {request_id_str}, cf-ray: {cf_ray_str}")
}

pub(super) async fn enroll_remote_control_server(
    remote_control_target: &RemoteControlTarget,
    auth: &RemoteControlConnectionAuth,
    installation_id: &str,
    server_name: &str,
) -> io::Result<RemoteControlEnrollment> {
    let enroll_url = &remote_control_target.enroll_url;
    let request = EnrollRemoteServerRequest {
        name: server_name.to_string(),
        os: std::env::consts::OS,
        arch: std::env::consts::ARCH,
        app_server_version: env!("CARGO_PKG_VERSION"),
        installation_id: installation_id.to_string(),
    };
    let enrollment_response = send_remote_control_server_request::<_, EnrollRemoteServerResponse>(
        enroll_url,
        auth,
        installation_id,
        &request,
        "enroll",
        "server enrollment",
    )
    .await?;
    let mut enrollment = RemoteControlEnrollment {
        remote_control_target: remote_control_target.clone(),
        account_id: auth.account_id.clone(),
        environment_id: enrollment_response.environment_id,
        server_id: enrollment_response.server_id,
        server_name: server_name.to_string(),
        remote_control_token: None,
        expires_at: None,
    };
    update_remote_control_server_token(
        &mut enrollment,
        enroll_url,
        enrollment_response.remote_control_token,
        enrollment_response.expires_at,
    )?;
    Ok(enrollment)
}

pub(super) async fn refresh_remote_control_server(
    auth: &RemoteControlConnectionAuth,
    installation_id: &str,
    enrollment: &mut RemoteControlEnrollment,
) -> io::Result<()> {
    let refresh_url = enrollment.remote_control_target.refresh_url.clone();
    let request = RefreshRemoteServerRequest {
        server_id: enrollment.server_id.clone(),
        installation_id: installation_id.to_string(),
    };
    let refreshed = send_remote_control_server_request::<_, EnrollRemoteServerResponse>(
        &refresh_url,
        auth,
        installation_id,
        &request,
        "refresh",
        "server refresh",
    )
    .await?;
    if refreshed.server_id != enrollment.server_id
        || refreshed.environment_id != enrollment.environment_id
    {
        return Err(io::Error::other(format!(
            "remote control server refresh returned mismatched enrollment: expected server_id={}, environment_id={}; got server_id={}, environment_id={}",
            enrollment.server_id,
            enrollment.environment_id,
            refreshed.server_id,
            refreshed.environment_id
        )));
    }

    update_remote_control_server_token(
        enrollment,
        &refresh_url,
        refreshed.remote_control_token,
        refreshed.expires_at,
    )
}

async fn send_remote_control_server_request<Request, Response>(
    url: &str,
    auth: &RemoteControlConnectionAuth,
    installation_id: &str,
    request: &Request,
    action: &str,
    response_kind: &str,
) -> io::Result<Response>
where
    Request: Serialize,
    Response: DeserializeOwned,
{
    let client = build_reqwest_client();
    let mut auth_headers = HeaderMap::new();
    auth.auth_provider.add_auth_headers(&mut auth_headers);
    let response = client
        .post(url)
        .timeout(REMOTE_CONTROL_ENROLL_TIMEOUT)
        .headers(auth_headers)
        .header(REMOTE_CONTROL_ACCOUNT_ID_HEADER, &auth.account_id)
        .header(REMOTE_CONTROL_INSTALLATION_ID_HEADER, installation_id)
        .json(request)
        .send()
        .await
        .map_err(|err| {
            io::Error::other(format!(
                "failed to {action} remote control server at `{url}`: {err}"
            ))
        })?;
    let headers = response.headers().clone();
    let status = response.status();
    let body = response.bytes().await.map_err(|err| {
        io::Error::other(format!(
            "failed to read remote control {response_kind} response from `{url}`: {err}"
        ))
    })?;
    let body_preview = preview_remote_control_response_body(&body);
    if !status.is_success() {
        let headers_str = format_headers(&headers);
        let error_kind = match status.as_u16() {
            401 | 403 => ErrorKind::PermissionDenied,
            404 => ErrorKind::NotFound,
            _ => ErrorKind::Other,
        };
        return Err(io::Error::new(
            error_kind,
            format!(
                "remote control {response_kind} failed at `{url}`: HTTP {status}, {headers_str}, body: {body_preview}"
            ),
        ));
    }

    serde_json::from_slice::<Response>(&body).map_err(|err| {
        let headers_str = format_headers(&headers);
        io::Error::other(format!(
            "failed to parse remote control {response_kind} response from `{url}`: HTTP {status}, {headers_str}, body: {body_preview}, decode error: {err}"
        ))
    })
}

fn update_remote_control_server_token(
    enrollment: &mut RemoteControlEnrollment,
    url: &str,
    token: String,
    expires_at: String,
) -> io::Result<()> {
    let expires_at = OffsetDateTime::parse(&expires_at, &Rfc3339).map_err(|err| {
        io::Error::other(format!(
            "failed to parse remote control server token expiry from `{url}`: {err}"
        ))
    })?;
    enrollment.remote_control_token = Some(token);
    enrollment.expires_at = Some(expires_at);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::remote_control::protocol::normalize_remote_control_url;
    use codex_state::StateRuntime;
    use pretty_assertions::assert_eq;
    use serde_json::json;
    use std::sync::Arc;
    use tempfile::TempDir;
    use tokio::io::AsyncBufReadExt;
    use tokio::io::AsyncWriteExt;
    use tokio::io::BufReader;
    use tokio::net::TcpListener;
    use tokio::net::TcpStream;
    use tokio::time::Duration;
    use tokio::time::timeout;

    async fn remote_control_state_runtime(codex_home: &TempDir) -> Arc<StateRuntime> {
        StateRuntime::init(codex_home.path().to_path_buf(), "test-provider".to_string())
            .await
            .expect("state runtime should initialize")
    }

    #[test]
    fn remote_control_enrollment_refreshes_server_token_before_expiry() {
        let expires_soon = RemoteControlEnrollment {
            remote_control_target: normalize_remote_control_url("http://localhost/backend-api/")
                .expect("target should normalize"),
            account_id: "account-a".to_string(),
            environment_id: "env_first".to_string(),
            server_id: "srv_e_first".to_string(),
            server_name: "first-server".to_string(),
            remote_control_token: Some("expires-soon".to_string()),
            expires_at: Some(OffsetDateTime::now_utc() + time::Duration::seconds(29)),
        };
        let expires_later = RemoteControlEnrollment {
            expires_at: Some(OffsetDateTime::now_utc() + time::Duration::seconds(31)),
            remote_control_token: Some("expires-later".to_string()),
            ..expires_soon.clone()
        };

        assert!(expires_soon.should_refresh_server_token());
        assert!(!expires_later.should_refresh_server_token());
    }

    #[test]
    fn preview_remote_control_response_body_redacts_server_token() {
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&preview_remote_control_response_body(
                br#"{"server_id":"srv_e_test","remote_control_token":"secret","pairing_code":"pairing-code","manual_pairing_code":"ABCD-EFGH"}"#
            ))
            .expect("redacted response preview should stay valid json"),
            json!({
                "server_id": "srv_e_test",
                "remote_control_token": "<redacted>",
                "pairing_code": "<redacted>",
                "manual_pairing_code": "<redacted>",
            })
        );
    }

    #[tokio::test]
    async fn persisted_remote_control_enrollment_round_trips_by_target_and_account() {
        let codex_home = TempDir::new().expect("temp dir should create");
        let state_db = remote_control_state_runtime(&codex_home).await;
        let first_target = normalize_remote_control_url("https://chatgpt.com/remote/control")
            .expect("first target should parse");
        let second_target =
            normalize_remote_control_url("https://api.chatgpt-staging.com/other/control")
                .expect("second target should parse");
        let first_enrollment = RemoteControlEnrollment {
            remote_control_target: first_target.clone(),
            account_id: "account-a".to_string(),
            environment_id: "env_first".to_string(),
            server_id: "srv_e_first".to_string(),
            server_name: "first-server".to_string(),
            remote_control_token: None,
            expires_at: None,
        };
        let second_enrollment = RemoteControlEnrollment {
            remote_control_target: second_target.clone(),
            account_id: "account-a".to_string(),
            environment_id: "env_second".to_string(),
            server_id: "srv_e_second".to_string(),
            server_name: "second-server".to_string(),
            remote_control_token: None,
            expires_at: None,
        };

        update_persisted_remote_control_enrollment(
            Some(state_db.as_ref()),
            &first_target,
            "account-a",
            Some("desktop-client"),
            Some(&first_enrollment),
            /*remote_control_enabled*/ None,
        )
        .await
        .expect("first enrollment should persist");
        update_persisted_remote_control_enrollment(
            Some(state_db.as_ref()),
            &second_target,
            "account-a",
            Some("desktop-client"),
            Some(&second_enrollment),
            /*remote_control_enabled*/ None,
        )
        .await
        .expect("second enrollment should persist");

        assert_eq!(
            load_persisted_remote_control_enrollment(
                Some(state_db.as_ref()),
                &first_target,
                "account-a",
                Some("desktop-client"),
            )
            .await
            .expect("first enrollment should load"),
            Some(first_enrollment.clone())
        );
        assert_eq!(
            load_persisted_remote_control_enrollment(
                Some(state_db.as_ref()),
                &first_target,
                "account-b",
                Some("desktop-client"),
            )
            .await
            .expect("missing account should load"),
            None
        );
        assert_eq!(
            load_persisted_remote_control_enrollment(
                Some(state_db.as_ref()),
                &second_target,
                "account-a",
                Some("desktop-client"),
            )
            .await
            .expect("second enrollment should load"),
            Some(second_enrollment)
        );
    }

    #[tokio::test]
    async fn clearing_persisted_remote_control_enrollment_removes_only_matching_entry() {
        let codex_home = TempDir::new().expect("temp dir should create");
        let state_db = remote_control_state_runtime(&codex_home).await;
        let first_target = normalize_remote_control_url("https://chatgpt.com/remote/control")
            .expect("first target should parse");
        let second_target =
            normalize_remote_control_url("https://api.chatgpt-staging.com/other/control")
                .expect("second target should parse");
        let first_enrollment = RemoteControlEnrollment {
            remote_control_target: first_target.clone(),
            account_id: "account-a".to_string(),
            environment_id: "env_first".to_string(),
            server_id: "srv_e_first".to_string(),
            server_name: "first-server".to_string(),
            remote_control_token: None,
            expires_at: None,
        };
        let second_enrollment = RemoteControlEnrollment {
            remote_control_target: second_target.clone(),
            account_id: "account-a".to_string(),
            environment_id: "env_second".to_string(),
            server_id: "srv_e_second".to_string(),
            server_name: "second-server".to_string(),
            remote_control_token: None,
            expires_at: None,
        };

        update_persisted_remote_control_enrollment(
            Some(state_db.as_ref()),
            &first_target,
            "account-a",
            /*app_server_client_name*/ None,
            Some(&first_enrollment),
            /*remote_control_enabled*/ None,
        )
        .await
        .expect("first enrollment should persist");
        update_persisted_remote_control_enrollment(
            Some(state_db.as_ref()),
            &second_target,
            "account-a",
            /*app_server_client_name*/ None,
            Some(&second_enrollment),
            /*remote_control_enabled*/ None,
        )
        .await
        .expect("second enrollment should persist");

        update_persisted_remote_control_enrollment(
            Some(state_db.as_ref()),
            &first_target,
            "account-a",
            /*app_server_client_name*/ None,
            /*enrollment*/ None,
            /*remote_control_enabled*/ None,
        )
        .await
        .expect("matching enrollment should clear");

        assert_eq!(
            load_persisted_remote_control_enrollment(
                Some(state_db.as_ref()),
                &first_target,
                "account-a",
                /*app_server_client_name*/ None,
            )
            .await
            .expect("cleared enrollment should load"),
            None
        );
        assert_eq!(
            load_persisted_remote_control_enrollment(
                Some(state_db.as_ref()),
                &second_target,
                "account-a",
                /*app_server_client_name*/ None,
            )
            .await
            .expect("remaining enrollment should load"),
            Some(second_enrollment)
        );
    }

    #[tokio::test]
    async fn enroll_remote_control_server_parse_failure_includes_response_body() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener should bind");
        let remote_control_url = format!(
            "http://127.0.0.1:{}/backend-api/",
            listener
                .local_addr()
                .expect("listener should have a local addr")
                .port()
        );
        let remote_control_target =
            normalize_remote_control_url(&remote_control_url).expect("target should parse");
        let enroll_url = remote_control_target.enroll_url.clone();
        let response_body = json!({
            "server_id": "srv_e_test",
            "environment_id": "env_test",
        });
        let expected_body = response_body.to_string();
        let server_task = tokio::spawn(async move {
            let stream = accept_http_request(&listener).await;
            respond_with_json(stream, response_body).await;
        });

        let err = enroll_remote_control_server(
            &remote_control_target,
            &RemoteControlConnectionAuth {
                auth_provider: codex_model_provider::unauthenticated_auth_provider(),
                account_id: "account_id".to_string(),
            },
            "11111111-1111-4111-8111-111111111111",
            "test-server",
        )
        .await
        .expect_err("invalid response should fail to parse");

        server_task.await.expect("server task should succeed");
        assert_eq!(
            err.to_string(),
            format!(
                "failed to parse remote control server enrollment response from `{enroll_url}`: HTTP 200 OK, request-id: <none>, cf-ray: <none>, body: {expected_body}, decode error: missing field `remote_control_token` at line 1 column {}",
                expected_body.len()
            )
        );
    }

    async fn accept_http_request(listener: &TcpListener) -> TcpStream {
        let (stream, _) = timeout(Duration::from_secs(5), listener.accept())
            .await
            .expect("HTTP request should arrive in time")
            .expect("listener accept should succeed");
        let mut reader = BufReader::new(stream);

        let mut request_line = String::new();
        reader
            .read_line(&mut request_line)
            .await
            .expect("request line should read");
        loop {
            let mut line = String::new();
            reader
                .read_line(&mut line)
                .await
                .expect("header line should read");
            if line == "\r\n" {
                break;
            }
        }

        reader.into_inner()
    }

    async fn respond_with_json(mut stream: TcpStream, body: serde_json::Value) {
        let body = body.to_string();
        let response = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
            body.len()
        );
        stream
            .write_all(response.as_bytes())
            .await
            .expect("response should write");
        stream.flush().await.expect("response should flush");
    }
}
