use super::protocol::EnrollRemoteServerRequest;
use super::protocol::EnrollRemoteServerResponse;
use super::protocol::RemoteControlTarget;
use axum::http::HeaderMap;
use chrono::DateTime;
use chrono::Duration;
use chrono::Utc;
use codex_login::default_client::build_reqwest_client;
use codex_state::RemoteControlEnrollmentRecord;
use codex_state::StateRuntime;
use gethostname::gethostname;
use std::io;
use std::io::ErrorKind;
use tracing::info;
use tracing::warn;

const REMOTE_CONTROL_ENROLL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);
const REMOTE_CONTROL_RESPONSE_BODY_MAX_BYTES: usize = 4096;
const REMOTE_CONTROL_SERVER_WEBSOCKET_SCOPE: &str = "remote_control_server_websocket";
const REMOTE_CONTROL_SERVER_TOKEN_REFRESH_SKEW_SECONDS: i64 = 60;

const REQUEST_ID_HEADER: &str = "x-request-id";
const OAI_REQUEST_ID_HEADER: &str = "x-oai-request-id";
const CF_RAY_HEADER: &str = "cf-ray";
const REMOTE_CONTROL_FEDRAMP_HEADER: &str = "X-OpenAI-Fedramp";
pub(super) const REMOTE_CONTROL_ACCOUNT_ID_HEADER: &str = "chatgpt-account-id";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RemoteControlEnrollment {
    pub(super) account_id: String,
    pub(super) environment_id: String,
    pub(super) server_id: String,
    pub(super) server_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RemoteControlServerToken {
    pub(super) bearer_token: String,
    pub(super) expires_at: DateTime<Utc>,
}

impl RemoteControlServerToken {
    pub(super) fn expires_soon(&self, now: DateTime<Utc>) -> bool {
        self.expires_at <= now + Duration::seconds(REMOTE_CONTROL_SERVER_TOKEN_REFRESH_SKEW_SECONDS)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RemoteControlEnrollmentResult {
    pub(super) enrollment: RemoteControlEnrollment,
    pub(super) server_token: Option<RemoteControlServerToken>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RemoteControlConnectionAuth {
    pub(super) authorization_header_value: String,
    pub(super) account_id: String,
    pub(super) is_fedramp_account: bool,
}

pub(super) async fn load_persisted_remote_control_enrollment(
    state_db: Option<&StateRuntime>,
    remote_control_target: &RemoteControlTarget,
    account_id: &str,
    app_server_client_name: Option<&str>,
) -> Option<RemoteControlEnrollment> {
    let Some(state_db) = state_db else {
        info!(
            "remote control enrollment cache unavailable because sqlite state db is disabled: websocket_url={}, account_id={}, app_server_client_name={:?}",
            remote_control_target.websocket_url, account_id, app_server_client_name
        );
        return None;
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
            return None;
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
            Some(RemoteControlEnrollment {
                account_id: enrollment.account_id,
                environment_id: enrollment.environment_id,
                server_id: enrollment.server_id,
                server_name: enrollment.server_name,
            })
        }
        None => {
            info!(
                "no persisted remote control enrollment found: websocket_url={}, account_id={}, app_server_client_name={:?}",
                remote_control_target.websocket_url, account_id, app_server_client_name
            );
            None
        }
    }
}

pub(super) async fn update_persisted_remote_control_enrollment(
    state_db: Option<&StateRuntime>,
    remote_control_target: &RemoteControlTarget,
    account_id: &str,
    app_server_client_name: Option<&str>,
    enrollment: Option<&RemoteControlEnrollment>,
) -> io::Result<()> {
    let Some(state_db) = state_db else {
        info!(
            "skipping remote control enrollment persistence because sqlite state db is disabled: websocket_url={}, account_id={}, app_server_client_name={:?}, has_enrollment={}",
            remote_control_target.websocket_url,
            account_id,
            app_server_client_name,
            enrollment.is_some()
        );
        return Ok(());
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
    if trimmed.len() <= REMOTE_CONTROL_RESPONSE_BODY_MAX_BYTES {
        return trimmed.to_string();
    }

    let mut cut = REMOTE_CONTROL_RESPONSE_BODY_MAX_BYTES;
    while !trimmed.is_char_boundary(cut) {
        cut = cut.saturating_sub(1);
    }
    let mut truncated = trimmed[..cut].to_string();
    truncated.push_str("...");
    truncated
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
) -> io::Result<RemoteControlEnrollmentResult> {
    let enroll_url = &remote_control_target.enroll_url;
    let server_name = gethostname().to_string_lossy().trim().to_string();
    let request = EnrollRemoteServerRequest {
        name: server_name.clone(),
        os: std::env::consts::OS,
        arch: std::env::consts::ARCH,
        app_server_version: env!("CARGO_PKG_VERSION"),
    };
    let client = build_reqwest_client();
    let mut http_request = client
        .post(enroll_url)
        .timeout(REMOTE_CONTROL_ENROLL_TIMEOUT)
        .header("authorization", &auth.authorization_header_value)
        .header(REMOTE_CONTROL_ACCOUNT_ID_HEADER, &auth.account_id);
    if auth.is_fedramp_account {
        http_request = http_request.header(REMOTE_CONTROL_FEDRAMP_HEADER, "true");
    }
    let http_request = http_request.json(&request);

    let response = http_request.send().await.map_err(|err| {
        io::Error::other(format!(
            "failed to enroll remote control server at `{enroll_url}`: {err}"
        ))
    })?;
    let headers = response.headers().clone();
    let status = response.status();
    let body = response.bytes().await.map_err(|err| {
        io::Error::other(format!(
            "failed to read remote control enrollment response from `{enroll_url}`: {err}"
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
                "remote control server enrollment failed at `{enroll_url}`: HTTP {status}, {headers_str}, body: {body_preview}"
            ),
        ));
    }

    let enrollment = serde_json::from_slice::<EnrollRemoteServerResponse>(&body).map_err(|err| {
        let headers_str = format_headers(&headers);
        io::Error::other(format!(
            "failed to parse remote control enrollment response from `{enroll_url}`: HTTP {status}, {headers_str}, body: {body_preview}, decode error: {err}"
        ))
    })?;

    let server_token = remote_control_server_token_from_response(&enrollment)?;
    Ok(RemoteControlEnrollmentResult {
        enrollment: RemoteControlEnrollment {
            account_id: auth.account_id.clone(),
            environment_id: enrollment.environment_id,
            server_id: enrollment.server_id,
            server_name,
        },
        server_token,
    })
}

fn remote_control_server_token_from_response(
    enrollment: &EnrollRemoteServerResponse,
) -> io::Result<Option<RemoteControlServerToken>> {
    let Some(remote_control_token) = enrollment.remote_control_token.as_ref() else {
        return Ok(None);
    };
    let expires_at = enrollment.expires_at.as_ref().ok_or_else(|| {
        io::Error::new(
            ErrorKind::InvalidData,
            "remote control enrollment response included a token without expires_at",
        )
    })?;
    let scopes = enrollment.scopes.as_ref().ok_or_else(|| {
        io::Error::new(
            ErrorKind::InvalidData,
            "remote control enrollment response included a token without scopes",
        )
    })?;
    if !scopes
        .iter()
        .any(|scope| scope == REMOTE_CONTROL_SERVER_WEBSOCKET_SCOPE)
    {
        return Err(io::Error::new(
            ErrorKind::InvalidData,
            "remote control enrollment response token is missing server websocket scope",
        ));
    }
    let expires_at = DateTime::parse_from_rfc3339(expires_at)
        .map_err(|err| {
            io::Error::new(
                ErrorKind::InvalidData,
                format!("invalid remote control token expires_at: {err}"),
            )
        })?
        .with_timezone(&Utc);

    Ok(Some(RemoteControlServerToken {
        bearer_token: remote_control_token.clone(),
        expires_at,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::remote_control::protocol::normalize_remote_control_url;
    use chrono::DateTime;
    use chrono::Utc;
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
            account_id: "account-a".to_string(),
            environment_id: "env_first".to_string(),
            server_id: "srv_e_first".to_string(),
            server_name: "first-server".to_string(),
        };
        let second_enrollment = RemoteControlEnrollment {
            account_id: "account-a".to_string(),
            environment_id: "env_second".to_string(),
            server_id: "srv_e_second".to_string(),
            server_name: "second-server".to_string(),
        };

        update_persisted_remote_control_enrollment(
            Some(state_db.as_ref()),
            &first_target,
            "account-a",
            Some("desktop-client"),
            Some(&first_enrollment),
        )
        .await
        .expect("first enrollment should persist");
        update_persisted_remote_control_enrollment(
            Some(state_db.as_ref()),
            &second_target,
            "account-a",
            Some("desktop-client"),
            Some(&second_enrollment),
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
            .await,
            Some(first_enrollment.clone())
        );
        assert_eq!(
            load_persisted_remote_control_enrollment(
                Some(state_db.as_ref()),
                &first_target,
                "account-b",
                Some("desktop-client"),
            )
            .await,
            None
        );
        assert_eq!(
            load_persisted_remote_control_enrollment(
                Some(state_db.as_ref()),
                &second_target,
                "account-a",
                Some("desktop-client"),
            )
            .await,
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
            account_id: "account-a".to_string(),
            environment_id: "env_first".to_string(),
            server_id: "srv_e_first".to_string(),
            server_name: "first-server".to_string(),
        };
        let second_enrollment = RemoteControlEnrollment {
            account_id: "account-a".to_string(),
            environment_id: "env_second".to_string(),
            server_id: "srv_e_second".to_string(),
            server_name: "second-server".to_string(),
        };

        update_persisted_remote_control_enrollment(
            Some(state_db.as_ref()),
            &first_target,
            "account-a",
            /*app_server_client_name*/ None,
            Some(&first_enrollment),
        )
        .await
        .expect("first enrollment should persist");
        update_persisted_remote_control_enrollment(
            Some(state_db.as_ref()),
            &second_target,
            "account-a",
            /*app_server_client_name*/ None,
            Some(&second_enrollment),
        )
        .await
        .expect("second enrollment should persist");

        update_persisted_remote_control_enrollment(
            Some(state_db.as_ref()),
            &first_target,
            "account-a",
            /*app_server_client_name*/ None,
            /*enrollment*/ None,
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
            .await,
            None
        );
        assert_eq!(
            load_persisted_remote_control_enrollment(
                Some(state_db.as_ref()),
                &second_target,
                "account-a",
                /*app_server_client_name*/ None,
            )
            .await,
            Some(second_enrollment)
        );
    }

    #[test]
    fn remote_control_server_token_from_response_accepts_legacy_response_without_token() {
        assert_eq!(
            remote_control_server_token_from_response(&EnrollRemoteServerResponse {
                server_id: "srv_e_test".to_string(),
                environment_id: "env_test".to_string(),
                remote_control_token: None,
                expires_at: None,
                scopes: None,
            })
            .expect("legacy response should parse"),
            None
        );
    }

    #[test]
    fn remote_control_server_token_from_response_parses_scoped_token() {
        let server_token = remote_control_server_token_from_response(&EnrollRemoteServerResponse {
            server_id: "srv_e_test".to_string(),
            environment_id: "env_test".to_string(),
            remote_control_token: Some("remote-control-token".to_string()),
            expires_at: Some("2026-04-09T12:00:00Z".to_string()),
            scopes: Some(vec!["remote_control_server_websocket".to_string()]),
        })
        .expect("token response should parse")
        .expect("token should be present");

        assert_eq!(server_token.bearer_token, "remote-control-token");
        assert!(
            !server_token.expires_soon(
                DateTime::parse_from_rfc3339("2026-04-09T11:58:30Z")
                    .expect("timestamp should parse")
                    .with_timezone(&Utc)
            )
        );
        assert!(
            server_token.expires_soon(
                DateTime::parse_from_rfc3339("2026-04-09T11:59:30Z")
                    .expect("timestamp should parse")
                    .with_timezone(&Utc)
            )
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
            "error": "not enrolled",
        });
        let expected_body = response_body.to_string();
        let server_task = tokio::spawn(async move {
            let stream = accept_http_request(&listener).await;
            respond_with_json(stream, response_body).await;
        });

        let err = enroll_remote_control_server(
            &remote_control_target,
            &RemoteControlConnectionAuth {
                authorization_header_value: "Bearer Access Token".to_string(),
                account_id: "account_id".to_string(),
                is_fedramp_account: false,
            },
        )
        .await
        .expect_err("invalid response should fail to parse");

        server_task.await.expect("server task should succeed");
        assert_eq!(
            err.to_string(),
            format!(
                "failed to parse remote control enrollment response from `{enroll_url}`: HTTP 200 OK, request-id: <none>, cf-ray: <none>, body: {expected_body}, decode error: missing field `server_id` at line 1 column {}",
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
