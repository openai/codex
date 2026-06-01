use super::protocol::EnrollRemoteServerRequest;
use super::protocol::EnrollRemoteServerResponse;
use super::protocol::RefreshRemoteServerRequest;
use super::protocol::RemoteControlTarget;
use super::protocol::StartRemoteControlPairingRequest;
use super::protocol::StartRemoteControlPairingResponse;
use super::remote_control_pairing_unavailable_error;
use axum::http::HeaderMap;
use codex_api::SharedAuthProvider;
use codex_app_server_protocol::RemoteControlPairingStartResponse;
use codex_login::AuthManager;
use codex_login::default_client::build_reqwest_client;
use codex_state::RemoteControlEnrollmentRecord;
use codex_state::StateRuntime;
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::io;
use std::io::ErrorKind;
use std::sync::Arc;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use tokio::sync::Mutex;
use tokio::sync::OwnedSemaphorePermit;
use tokio::sync::Semaphore;
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
pub(super) struct RemoteControlServer {
    pub(super) account_id: String,
    pub(super) environment_id: String,
    pub(super) server_id: String,
    pub(super) server_name: String,
    pub(super) remote_control_token: Option<String>,
    pub(super) expires_at: Option<OffsetDateTime>,
}

impl RemoteControlServer {
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

pub(super) struct RemoteControlConnectionAuth {
    pub(super) auth_provider: SharedAuthProvider,
    pub(super) account_id: String,
}

pub(super) async fn load_persisted_remote_control_enrollment(
    state_db: Option<&StateRuntime>,
    remote_control_target: &RemoteControlTarget,
    account_id: &str,
    app_server_client_name: Option<&str>,
) -> io::Result<Option<RemoteControlServer>> {
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
            Ok(Some(RemoteControlServer {
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
    enrollment: Option<&RemoteControlServer>,
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

impl RemoteControlServer {
    pub(super) async fn enroll(
        remote_control_target: &RemoteControlTarget,
        auth: &RemoteControlConnectionAuth,
        installation_id: &str,
        server_name: &str,
    ) -> io::Result<Self> {
        let enroll_url = &remote_control_target.enroll_url;
        let request = EnrollRemoteServerRequest {
            name: server_name.to_string(),
            os: std::env::consts::OS,
            arch: std::env::consts::ARCH,
            app_server_version: env!("CARGO_PKG_VERSION"),
            installation_id: installation_id.to_string(),
        };
        let enrollment_response =
            send_remote_control_server_request::<_, EnrollRemoteServerResponse>(
                enroll_url,
                auth,
                installation_id,
                &request,
                "enroll",
                "server enrollment",
            )
            .await?;
        let mut server = Self {
            account_id: auth.account_id.clone(),
            environment_id: enrollment_response.environment_id,
            server_id: enrollment_response.server_id,
            server_name: server_name.to_string(),
            remote_control_token: None,
            expires_at: None,
        };
        update_remote_control_server_token(
            &mut server,
            enroll_url,
            enrollment_response.remote_control_token,
            enrollment_response.expires_at,
        )?;
        Ok(server)
    }

    pub(super) async fn refresh_server_token(
        &mut self,
        remote_control_target: &RemoteControlTarget,
        auth: &RemoteControlConnectionAuth,
        installation_id: &str,
    ) -> io::Result<()> {
        let refresh_url = &remote_control_target.refresh_url;
        let request = RefreshRemoteServerRequest {
            server_id: self.server_id.clone(),
            installation_id: installation_id.to_string(),
        };
        let refreshed = send_remote_control_server_request::<_, EnrollRemoteServerResponse>(
            refresh_url,
            auth,
            installation_id,
            &request,
            "refresh",
            "server refresh",
        )
        .await?;
        if refreshed.server_id != self.server_id || refreshed.environment_id != self.environment_id
        {
            return Err(io::Error::other(format!(
                "remote control server refresh returned mismatched enrollment: expected server_id={}, environment_id={}; got server_id={}, environment_id={}",
                self.server_id, self.environment_id, refreshed.server_id, refreshed.environment_id
            )));
        }

        update_remote_control_server_token(
            self,
            refresh_url,
            refreshed.remote_control_token,
            refreshed.expires_at,
        )
    }

    pub(super) async fn start_pairing(
        &self,
        remote_control_target: &RemoteControlTarget,
        request: StartRemoteControlPairingRequest,
    ) -> io::Result<RemoteControlPairingStartResponse> {
        let remote_control_token = self
            .remote_control_token
            .as_deref()
            .ok_or_else(remote_control_pairing_unavailable_error)?;
        let expires_at = self
            .expires_at
            .ok_or_else(remote_control_pairing_unavailable_error)?;
        if expires_at <= OffsetDateTime::now_utc() {
            return Err(io::Error::new(
                ErrorKind::InvalidInput,
                "remote control pairing is unavailable because the server token expired",
            ));
        }

        let pairing_url = &remote_control_target.pair_url;
        let response = build_reqwest_client()
            .post(pairing_url)
            .timeout(REMOTE_CONTROL_PAIRING_TIMEOUT)
            .bearer_auth(remote_control_token)
            .json(&request)
            .send()
            .await
            .map_err(|err| {
                io::Error::other(format!(
                    "failed to start remote control pairing at `{pairing_url}`: {err}"
                ))
            })?;
        let headers = response.headers().clone();
        let status = response.status();
        let body = response.bytes().await.map_err(|err| {
            io::Error::other(format!(
                "failed to read remote control pairing response from `{pairing_url}`: {err}"
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
                    "remote control pairing failed at `{pairing_url}`: HTTP {status}, {}, body: {body_preview}",
                    format_headers(&headers)
                ),
            ));
        }

        let pairing =
            serde_json::from_slice::<StartRemoteControlPairingResponse>(&body).map_err(|err| {
                io::Error::other(format!(
                    "failed to parse remote control pairing response from `{pairing_url}`: HTTP {status}, {}, body: {body_preview}, decode error: {err}",
                    format_headers(&headers)
                ))
            })?;
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
                    format!("invalid remote control pairing expires_at: {err}"),
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
}

#[derive(Clone)]
pub(super) struct SharedRemoteControlServer {
    remote_control_url: String,
    installation_id: String,
    server_name: String,
    state_db: Option<Arc<StateRuntime>>,
    auth_manager: Arc<AuthManager>,
    app_server_client_name: Arc<std::sync::Mutex<Option<String>>>,
    server: Arc<Mutex<Option<RemoteControlServer>>>,
    server_mutations: Arc<Semaphore>,
}

impl SharedRemoteControlServer {
    pub(super) fn new(
        remote_control_url: String,
        installation_id: String,
        server_name: String,
        state_db: Option<Arc<StateRuntime>>,
        auth_manager: Arc<AuthManager>,
    ) -> Self {
        Self {
            remote_control_url,
            installation_id,
            server_name,
            state_db,
            auth_manager,
            app_server_client_name: Arc::new(std::sync::Mutex::new(None)),
            server: Arc::new(Mutex::new(None)),
            server_mutations: Arc::new(Semaphore::new(1)),
        }
    }

    pub(super) fn set_app_server_client_name(&self, app_server_client_name: Option<String>) {
        *self
            .app_server_client_name
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = app_server_client_name;
    }

    pub(super) async fn snapshot(&self) -> Option<RemoteControlServer> {
        self.server.lock().await.clone()
    }

    pub(super) async fn clear(&self) {
        let Ok(_server_mutation) = self.server_mutation().await else {
            return;
        };
        *self.server.lock().await = None;
    }

    #[cfg(test)]
    pub(super) async fn replace(&self, server: Option<RemoteControlServer>) {
        let Ok(_server_mutation) = self.server_mutation().await else {
            return;
        };
        *self.server.lock().await = server;
    }

    pub(super) async fn prepare(
        &self,
        auth: &RemoteControlConnectionAuth,
    ) -> io::Result<(RemoteControlTarget, RemoteControlServer)> {
        self.prepare_with_environment_updates(auth, |_| {}).await
    }

    pub(super) async fn prepare_with_environment_updates(
        &self,
        auth: &RemoteControlConnectionAuth,
        mut publish_environment_id: impl FnMut(Option<String>),
    ) -> io::Result<(RemoteControlTarget, RemoteControlServer)> {
        let remote_control_target =
            super::protocol::normalize_remote_control_url(&self.remote_control_url)?;
        let app_server_client_name = self
            .app_server_client_name
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        let _server_mutation = self.server_mutation().await?;
        let Some(state_db) = self.state_db.as_deref() else {
            *self.server.lock().await = None;
            return Err(io::Error::new(
                ErrorKind::NotFound,
                "remote control requires sqlite state db",
            ));
        };
        let server = self.snapshot().await;
        let server = prepare_remote_control_server(
            &RemoteControlServerPrepareContext {
                state_db,
                remote_control_target: &remote_control_target,
                auth,
                installation_id: &self.installation_id,
                server_name: &self.server_name,
                app_server_client_name: app_server_client_name.as_deref(),
            },
            server,
            &mut publish_environment_id,
        )
        .await?;
        *self.server.lock().await = server.clone();
        Ok((
            remote_control_target,
            server
                .ok_or_else(|| io::Error::other("missing remote control server after prepare"))?,
        ))
    }

    pub(super) async fn start_pairing(
        &self,
        params: codex_app_server_protocol::RemoteControlPairingStartParams,
    ) -> io::Result<RemoteControlPairingStartResponse> {
        let auth_change_rx = self.auth_manager.auth_change_receiver();
        let auth_change_revision = *auth_change_rx.borrow();
        let auth = load_remote_control_auth(&self.auth_manager)
            .await
            .map_err(|err| {
                if matches!(
                    err.kind(),
                    ErrorKind::PermissionDenied | ErrorKind::WouldBlock
                ) {
                    super::remote_control_pairing_unavailable_error()
                } else {
                    err
                }
            })?;
        let (remote_control_target, server) = self.prepare(&auth).await?;
        let pairing_response = server
            .start_pairing(
                &remote_control_target,
                StartRemoteControlPairingRequest {
                    manual_code: params.manual_code,
                },
            )
            .await;
        if *auth_change_rx.borrow() != auth_change_revision {
            return Err(super::remote_control_pairing_unavailable_error());
        }
        if pairing_response
            .as_ref()
            .is_err_and(|err| err.kind() == ErrorKind::PermissionDenied)
        {
            self.clear_server_token_if_current(&server).await;
        }
        if pairing_response
            .as_ref()
            .is_err_and(|err| err.kind() == ErrorKind::NotFound)
        {
            self.clear_server_if_current(&remote_control_target, &server)
                .await;
        }
        pairing_response
    }

    pub(super) async fn clear_server_token_if_current(
        &self,
        expected_server: &RemoteControlServer,
    ) {
        let Ok(_server_mutation) = self.server_mutation().await else {
            return;
        };
        let mut server = self.server.lock().await;
        if server.as_ref() != Some(expected_server) {
            return;
        }
        if let Some(server) = server.as_mut() {
            server.clear_server_token();
        }
    }

    pub(super) async fn clear_server_if_current(
        &self,
        remote_control_target: &RemoteControlTarget,
        expected_server: &RemoteControlServer,
    ) {
        let app_server_client_name = self
            .app_server_client_name
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        let Ok(_server_mutation) = self.server_mutation().await else {
            return;
        };
        if self.snapshot().await.as_ref() != Some(expected_server) {
            return;
        }
        if update_persisted_remote_control_enrollment(
            self.state_db.as_deref(),
            remote_control_target,
            &expected_server.account_id,
            app_server_client_name.as_deref(),
            None,
        )
        .await
        .is_ok()
        {
            *self.server.lock().await = None;
        }
    }

    async fn server_mutation(&self) -> io::Result<OwnedSemaphorePermit> {
        self.server_mutations
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| io::Error::other("remote control server mutation semaphore closed"))
    }
}

pub(super) async fn load_remote_control_auth(
    auth_manager: &AuthManager,
) -> io::Result<RemoteControlConnectionAuth> {
    let mut reloaded = false;
    let auth = loop {
        let Some(auth) = auth_manager.auth().await else {
            if reloaded {
                return Err(io::Error::new(
                    ErrorKind::PermissionDenied,
                    "remote control requires ChatGPT authentication",
                ));
            }
            auth_manager.reload().await;
            reloaded = true;
            continue;
        };
        if !auth.uses_codex_backend() {
            break auth;
        }
        if auth.get_account_id().is_none() && !reloaded {
            auth_manager.reload().await;
            reloaded = true;
            continue;
        }
        break auth;
    };

    if !auth.uses_codex_backend() {
        return Err(io::Error::new(
            ErrorKind::PermissionDenied,
            "remote control requires ChatGPT authentication; API key auth is not supported",
        ));
    }

    Ok(RemoteControlConnectionAuth {
        auth_provider: codex_model_provider::auth_provider_from_auth(&auth),
        account_id: auth.get_account_id().ok_or_else(|| {
            io::Error::new(
                ErrorKind::WouldBlock,
                "remote control enrollment is waiting for a ChatGPT account id",
            )
        })?,
    })
}

struct RemoteControlServerPrepareContext<'a> {
    state_db: &'a StateRuntime,
    remote_control_target: &'a RemoteControlTarget,
    auth: &'a RemoteControlConnectionAuth,
    installation_id: &'a str,
    server_name: &'a str,
    app_server_client_name: Option<&'a str>,
}

async fn prepare_remote_control_server(
    context: &RemoteControlServerPrepareContext<'_>,
    mut server: Option<RemoteControlServer>,
    publish_environment_id: &mut impl FnMut(Option<String>),
) -> io::Result<Option<RemoteControlServer>> {
    if server
        .as_ref()
        .is_some_and(|server| server.account_id != context.auth.account_id)
    {
        server = None;
        publish_environment_id(None);
    }
    if server.is_none() {
        server = load_persisted_remote_control_enrollment(
            Some(context.state_db),
            context.remote_control_target,
            &context.auth.account_id,
            context.app_server_client_name,
        )
        .await?
        .map(|mut server| {
            server.server_name = context.server_name.to_string();
            server
        });
    }
    if let Some(server) = server.as_ref() {
        publish_environment_id(Some(server.environment_id.clone()));
    }
    if server.is_none() {
        let new_server = RemoteControlServer::enroll(
            context.remote_control_target,
            context.auth,
            context.installation_id,
            context.server_name,
        )
        .await?;
        update_persisted_remote_control_enrollment(
            Some(context.state_db),
            context.remote_control_target,
            &context.auth.account_id,
            context.app_server_client_name,
            Some(&new_server),
        )
        .await?;
        publish_environment_id(Some(new_server.environment_id.clone()));
        server = Some(new_server);
    }
    if server
        .as_ref()
        .is_some_and(RemoteControlServer::should_refresh_server_token)
    {
        let refresh_result = server
            .as_mut()
            .ok_or_else(|| io::Error::other("missing remote control server before refresh"))?
            .refresh_server_token(
                context.remote_control_target,
                context.auth,
                context.installation_id,
            )
            .await;
        if let Err(err) = refresh_result {
            if err.kind() != ErrorKind::NotFound {
                return Err(err);
            }
            update_persisted_remote_control_enrollment(
                Some(context.state_db),
                context.remote_control_target,
                &context.auth.account_id,
                context.app_server_client_name,
                None,
            )
            .await?;
            publish_environment_id(None);
            let new_server = RemoteControlServer::enroll(
                context.remote_control_target,
                context.auth,
                context.installation_id,
                context.server_name,
            )
            .await?;
            update_persisted_remote_control_enrollment(
                Some(context.state_db),
                context.remote_control_target,
                &context.auth.account_id,
                context.app_server_client_name,
                Some(&new_server),
            )
            .await?;
            publish_environment_id(Some(new_server.environment_id.clone()));
            server = Some(new_server);
        }
    }
    Ok(server)
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
    server: &mut RemoteControlServer,
    url: &str,
    token: String,
    expires_at: String,
) -> io::Result<()> {
    let expires_at = OffsetDateTime::parse(&expires_at, &Rfc3339).map_err(|err| {
        io::Error::other(format!(
            "failed to parse remote control server token expiry from `{url}`: {err}"
        ))
    })?;
    server.remote_control_token = Some(token);
    server.expires_at = Some(expires_at);
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
        let expires_soon = RemoteControlServer {
            account_id: "account-a".to_string(),
            environment_id: "env_first".to_string(),
            server_id: "srv_e_first".to_string(),
            server_name: "first-server".to_string(),
            remote_control_token: Some("expires-soon".to_string()),
            expires_at: Some(OffsetDateTime::now_utc() + time::Duration::seconds(29)),
        };
        let expires_later = RemoteControlServer {
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
                br#"{"server_id":"srv_e_test","remote_control_token":"secret"}"#
            ))
            .expect("redacted response preview should stay valid json"),
            json!({
                "server_id": "srv_e_test",
                "remote_control_token": "<redacted>",
            })
        );
    }

    #[test]
    fn preview_remote_control_response_body_redacts_pairing_codes() {
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&preview_remote_control_response_body(
                br#"{"pairing_code":"pairing-code","manual_pairing_code":"ABCD-EFGH"}"#
            ))
            .expect("redacted response preview should stay valid json"),
            json!({
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
        let first_enrollment = RemoteControlServer {
            account_id: "account-a".to_string(),
            environment_id: "env_first".to_string(),
            server_id: "srv_e_first".to_string(),
            server_name: "first-server".to_string(),
            remote_control_token: None,
            expires_at: None,
        };
        let second_enrollment = RemoteControlServer {
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
        let first_enrollment = RemoteControlServer {
            account_id: "account-a".to_string(),
            environment_id: "env_first".to_string(),
            server_id: "srv_e_first".to_string(),
            server_name: "first-server".to_string(),
            remote_control_token: None,
            expires_at: None,
        };
        let second_enrollment = RemoteControlServer {
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

        let err = RemoteControlServer::enroll(
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
