use super::auth::RemoteControlConnectionAuth;
use super::enroll::RemoteControlEnrollment;
use super::enroll::RemoteControlServerTokenRefreshRequirement;
use super::enroll::format_headers;
use super::enroll::preview_remote_control_response_body;
use super::protocol::EnrollRemoteServerRequest;
use super::protocol::EnrollRemoteServerResponse;
use super::protocol::RefreshRemoteServerRequest;
use super::protocol::RemoteControlTarget;
use axum::http::HeaderMap;
use axum::http::StatusCode;
use codex_login::default_client::create_client_without_request_logging;
use rand::Rng;
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::fmt;
use std::io;
use std::io::ErrorKind;
use std::time::Duration;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use tracing::warn;

const REMOTE_CONTROL_ENROLL_TIMEOUT: Duration = Duration::from_secs(30);
const REMOTE_CONTROL_SERVER_TOKEN_REFRESH_BACKOFF_MIN_SECS: u64 = 24;
const REMOTE_CONTROL_SERVER_TOKEN_REFRESH_BACKOFF_MAX_SECS: u64 = 36;

pub(super) const REMOTE_CONTROL_INSTALLATION_ID_HEADER: &str = "x-codex-installation-id";

#[derive(Debug)]
struct RemoteControlServerRequestError {
    message: String,
    status: Option<StatusCode>,
    retry_at: Option<OffsetDateTime>,
}

impl RemoteControlServerRequestError {
    fn io_error(
        message: String,
        status: Option<StatusCode>,
        retry_at: Option<OffsetDateTime>,
        timed_out: bool,
    ) -> io::Error {
        let kind = match status {
            Some(StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN) => ErrorKind::PermissionDenied,
            Some(StatusCode::NOT_FOUND) => ErrorKind::NotFound,
            Some(status) if timed_out && !status.is_client_error() => ErrorKind::TimedOut,
            None if timed_out => ErrorKind::TimedOut,
            Some(_) | None => ErrorKind::Other,
        };
        io::Error::new(
            kind,
            Self {
                message,
                status,
                retry_at,
            },
        )
    }

    fn is_transient(&self, kind: ErrorKind) -> bool {
        kind == ErrorKind::TimedOut
            || self.status.is_none()
            || self.status.is_some_and(|status| {
                status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error()
            })
    }
}

impl fmt::Display for RemoteControlServerRequestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for RemoteControlServerRequestError {}

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
        REMOTE_CONTROL_ENROLL_TIMEOUT,
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
        next_refresh_at: None,
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
    let now = OffsetDateTime::now_utc();
    let refresh_requirement = enrollment.server_token_refresh_requirement_at(now);
    if refresh_requirement == RemoteControlServerTokenRefreshRequirement::NotNeeded {
        return Ok(());
    }
    if refresh_requirement == RemoteControlServerTokenRefreshRequirement::Required
        && let Some(next_refresh_at) = enrollment.next_refresh_at
        && next_refresh_at > now
    {
        return Err(io::Error::new(
            ErrorKind::WouldBlock,
            format!("remote control server token refresh deferred until {next_refresh_at}"),
        ));
    }
    let refresh_url = enrollment.remote_control_target.refresh_url.clone();
    let request = RefreshRemoteServerRequest {
        server_id: enrollment.server_id.clone(),
        installation_id: installation_id.to_string(),
    };
    let refreshed = match send_remote_control_server_request::<_, EnrollRemoteServerResponse>(
        &refresh_url,
        auth,
        installation_id,
        &request,
        "refresh",
        "server refresh",
        REMOTE_CONTROL_ENROLL_TIMEOUT,
    )
    .await
    {
        Ok(refreshed) => refreshed,
        Err(err) => {
            let Some(refresh_error) = remote_control_server_request_error(&err) else {
                return Err(err);
            };
            if !refresh_error.is_transient(err.kind()) {
                return Err(err);
            }
            let now = OffsetDateTime::now_utc();
            let refresh_is_required = enrollment.server_token_refresh_requirement_at(now)
                == RemoteControlServerTokenRefreshRequirement::Required;
            let (refresh_delay, next_refresh_at) = refresh_deferral(refresh_error.retry_at, now);
            enrollment.next_refresh_at = Some(next_refresh_at);
            if refresh_is_required {
                warn!(
                    refresh_url,
                    server_id = %enrollment.server_id,
                    environment_id = %enrollment.environment_id,
                    error = %err,
                    ?refresh_delay,
                    %next_refresh_at,
                    "required remote control server token refresh failed; deferring next attempt"
                );
                return Err(err);
            }
            warn!(
                refresh_url,
                server_id = %enrollment.server_id,
                environment_id = %enrollment.environment_id,
                error = %err,
                ?refresh_delay,
                %next_refresh_at,
                "proactive remote control server token refresh failed; continuing with valid token"
            );
            return Ok(());
        }
    };
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
    timeout: Duration,
) -> io::Result<Response>
where
    Request: Serialize,
    Response: DeserializeOwned,
{
    let client = create_client_without_request_logging();
    let auth_headers = auth.request_headers()?;
    let response = client
        .post(url)
        .timeout(timeout)
        .headers(auth_headers)
        .header(REMOTE_CONTROL_INSTALLATION_ID_HEADER, installation_id)
        .json(request)
        .send()
        .await
        .map_err(|err| {
            let timed_out = err.is_timeout();
            RemoteControlServerRequestError::io_error(
                format!("failed to {action} remote control server at `{url}`: {err}"),
                /*status*/ None,
                /*retry_at*/ None,
                timed_out,
            )
        })?;
    let headers = response.headers().clone();
    let status = response.status();
    let retry_at = parse_retry_after(&headers, OffsetDateTime::now_utc());
    let body = response.bytes().await.map_err(|err| {
        let timed_out = err.is_timeout();
        RemoteControlServerRequestError::io_error(
            format!("failed to read remote control {response_kind} response from `{url}`: {err}"),
            Some(status),
            retry_at,
            timed_out,
        )
    })?;
    let body_preview = preview_remote_control_response_body(&body);
    if !status.is_success() {
        let headers_str = format_headers(&headers);
        return Err(RemoteControlServerRequestError::io_error(
            format!(
                "remote control {response_kind} failed at `{url}`: HTTP {status}, {headers_str}, body: {body_preview}"
            ),
            Some(status),
            retry_at,
            /*timed_out*/ false,
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
    enrollment.next_refresh_at = None;
    Ok(())
}

fn remote_control_server_request_error(
    err: &io::Error,
) -> Option<&RemoteControlServerRequestError> {
    err.get_ref()?.downcast_ref()
}

fn parse_retry_after(headers: &HeaderMap, received_at: OffsetDateTime) -> Option<OffsetDateTime> {
    let retry_after = headers
        .get(axum::http::header::RETRY_AFTER)?
        .to_str()
        .ok()?;
    let retry_at = if let Ok(seconds) = retry_after.parse::<u64>() {
        let seconds = i64::try_from(seconds).ok()?;
        received_at.checked_add(time::Duration::seconds(seconds))?
    } else {
        OffsetDateTime::from(httpdate::parse_http_date(retry_after).ok()?)
    };
    (retry_at > received_at).then_some(retry_at)
}

fn refresh_deferral(
    retry_at: Option<OffsetDateTime>,
    now: OffsetDateTime,
) -> (Duration, OffsetDateTime) {
    if let Some(retry_at) = retry_at
        && let Ok(delay) = Duration::try_from(retry_at - now)
        && !delay.is_zero()
    {
        return (delay, retry_at);
    }
    let delay = remote_control_server_token_refresh_backoff();
    let next_refresh_at = now + time::Duration::seconds(delay.as_secs() as i64);
    (delay, next_refresh_at)
}

fn remote_control_server_token_refresh_backoff() -> Duration {
    Duration::from_secs(rand::rng().random_range(
        REMOTE_CONTROL_SERVER_TOKEN_REFRESH_BACKOFF_MIN_SECS
            ..=REMOTE_CONTROL_SERVER_TOKEN_REFRESH_BACKOFF_MAX_SECS,
    ))
}

#[cfg(test)]
#[path = "server_api_tests.rs"]
mod tests;
