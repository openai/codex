use super::protocol::EnrollRemoteServerRequest;
use super::protocol::EnrollRemoteServerResponse;
use super::protocol::RemoteControlTarget;
use axum::http::HeaderMap;
use codex_core::default_client::build_reqwest_client;
use codex_state::StateRuntime;
use gethostname::gethostname;
use std::io;
use std::io::ErrorKind;
use tracing::warn;

const REMOTE_CONTROL_ENROLL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);
const REMOTE_CONTROL_RESPONSE_BODY_MAX_BYTES: usize = 4096;

const REQUEST_ID_HEADER: &str = "x-request-id";
const OAI_REQUEST_ID_HEADER: &str = "x-oai-request-id";
const CF_RAY_HEADER: &str = "cf-ray";
pub(super) const REMOTE_CONTROL_ACCOUNT_ID_HEADER: &str = "chatgpt-account-id";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RemoteControlEnrollment {
    pub(super) account_id: Option<String>,
    pub(super) server_id: String,
    pub(super) server_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RemoteControlConnectionAuth {
    pub(super) bearer_token: String,
    pub(super) account_id: Option<String>,
}

pub(super) async fn load_persisted_remote_control_enrollment(
    state_db: Option<&StateRuntime>,
    remote_control_target: &RemoteControlTarget,
    account_id: Option<&str>,
) -> Option<RemoteControlEnrollment> {
    let state_db = state_db?;
    let enrollment = match state_db
        .get_remote_control_enrollment(&remote_control_target.websocket_url, account_id)
        .await
    {
        Ok(enrollment) => enrollment,
        Err(err) => {
            warn!("{err}");
            return None;
        }
    };

    enrollment.map(|(server_id, server_name)| RemoteControlEnrollment {
        account_id: account_id.map(&str::to_string),
        server_id,
        server_name,
    })
}

pub(super) async fn update_persisted_remote_control_enrollment(
    state_db: Option<&StateRuntime>,
    remote_control_target: &RemoteControlTarget,
    account_id: Option<&str>,
    enrollment: Option<&RemoteControlEnrollment>,
) -> io::Result<()> {
    let Some(state_db) = state_db else {
        return Ok(());
    };
    if let &Some(enrollment) = &enrollment
        && enrollment.account_id.as_deref() != account_id
    {
        return Err(io::Error::other(format!(
            "enrollment account_id does not match expected account_id `{account_id:?}`"
        )));
    }

    if let Some(enrollment) = enrollment {
        state_db
            .upsert_remote_control_enrollment(
                &remote_control_target.websocket_url,
                account_id,
                &enrollment.server_id,
                &enrollment.server_name,
            )
            .await
            .map_err(io::Error::other)
    } else {
        state_db
            .delete_remote_control_enrollment(&remote_control_target.websocket_url, account_id)
            .await
            .map(|_| ())
            .map_err(io::Error::other)
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
) -> io::Result<RemoteControlEnrollment> {
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
        .bearer_auth(&auth.bearer_token)
        .json(&request);
    let account_id = auth.account_id.as_deref();
    if let Some(account_id) = account_id {
        http_request = http_request.header(REMOTE_CONTROL_ACCOUNT_ID_HEADER, account_id);
    }

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

    Ok(RemoteControlEnrollment {
        account_id: account_id.map(&str::to_string),
        server_id: enrollment.server_id,
        server_name,
    })
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
            account_id: Some("account-a".to_string()),
            server_id: "srv_e_first".to_string(),
            server_name: "first-server".to_string(),
        };
        let second_enrollment = RemoteControlEnrollment {
            account_id: Some("account-a".to_string()),
            server_id: "srv_e_second".to_string(),
            server_name: "second-server".to_string(),
        };

        update_persisted_remote_control_enrollment(
            Some(state_db.as_ref()),
            &first_target,
            Some("account-a"),
            Some(&first_enrollment),
        )
        .await
        .expect("first enrollment should persist");
        update_persisted_remote_control_enrollment(
            Some(state_db.as_ref()),
            &second_target,
            Some("account-a"),
            Some(&second_enrollment),
        )
        .await
        .expect("second enrollment should persist");

        assert_eq!(
            load_persisted_remote_control_enrollment(
                Some(state_db.as_ref()),
                &first_target,
                Some("account-a"),
            )
            .await,
            Some(first_enrollment.clone())
        );
        assert_eq!(
            load_persisted_remote_control_enrollment(
                Some(state_db.as_ref()),
                &first_target,
                Some("account-b"),
            )
            .await,
            None
        );
        assert_eq!(
            load_persisted_remote_control_enrollment(
                Some(state_db.as_ref()),
                &second_target,
                Some("account-a"),
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
            account_id: Some("account-a".to_string()),
            server_id: "srv_e_first".to_string(),
            server_name: "first-server".to_string(),
        };
        let second_enrollment = RemoteControlEnrollment {
            account_id: Some("account-a".to_string()),
            server_id: "srv_e_second".to_string(),
            server_name: "second-server".to_string(),
        };

        update_persisted_remote_control_enrollment(
            Some(state_db.as_ref()),
            &first_target,
            Some("account-a"),
            Some(&first_enrollment),
        )
        .await
        .expect("first enrollment should persist");
        update_persisted_remote_control_enrollment(
            Some(state_db.as_ref()),
            &second_target,
            Some("account-a"),
            Some(&second_enrollment),
        )
        .await
        .expect("second enrollment should persist");

        update_persisted_remote_control_enrollment(
            Some(state_db.as_ref()),
            &first_target,
            Some("account-a"),
            None,
        )
        .await
        .expect("matching enrollment should clear");

        assert_eq!(
            load_persisted_remote_control_enrollment(
                Some(state_db.as_ref()),
                &first_target,
                Some("account-a"),
            )
            .await,
            None
        );
        assert_eq!(
            load_persisted_remote_control_enrollment(
                Some(state_db.as_ref()),
                &second_target,
                Some("account-a"),
            )
            .await,
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
                bearer_token: "Access Token".to_string(),
                account_id: Some("account_id".to_string()),
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
