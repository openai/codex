use super::enroll::format_headers;
use super::enroll::preview_remote_control_response_body;
use super::protocol::RemoteControlTarget;
use super::protocol::StartRemoteControlPairingRequest;
use super::protocol::StartRemoteControlPairingResponse;
use codex_app_server_protocol::RemoteControlPairingStartResponse;
use codex_login::default_client::build_reqwest_client;
use std::io;
use std::io::ErrorKind;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

const REMOTE_CONTROL_PAIRING_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

#[derive(Debug, Clone)]
pub(super) struct RemoteControlPairingClient {
    pairing_url: String,
    remote_control_token: String,
    server_id: String,
    environment_id: String,
    expires_at: OffsetDateTime,
    auth_change_revision: u64,
    generation: u64,
}

impl RemoteControlPairingClient {
    pub(super) fn new(
        remote_control_target: &RemoteControlTarget,
        remote_control_token: String,
        server_id: String,
        environment_id: String,
        expires_at: OffsetDateTime,
        auth_change_revision: u64,
        generation: u64,
    ) -> Self {
        Self {
            pairing_url: remote_control_target.pair_url.clone(),
            remote_control_token,
            server_id,
            environment_id,
            expires_at,
            auth_change_revision,
            generation,
        }
    }

    pub(super) fn matches_auth_change_revision(&self, auth_change_revision: u64) -> bool {
        self.auth_change_revision == auth_change_revision
    }

    pub(super) fn matches_pairing_auth(&self, other: &Self) -> bool {
        self.pairing_url == other.pairing_url
            && self.remote_control_token == other.remote_control_token
            && self.server_id == other.server_id
            && self.environment_id == other.environment_id
            && self.auth_change_revision == other.auth_change_revision
            && self.generation == other.generation
    }

    pub(super) async fn start(
        &self,
        request: StartRemoteControlPairingRequest,
    ) -> io::Result<RemoteControlPairingStartResponse> {
        if self.expires_at <= OffsetDateTime::now_utc() {
            return Err(io::Error::new(
                ErrorKind::InvalidInput,
                "remote control pairing is unavailable because the server token expired",
            ));
        }

        let response = build_reqwest_client()
            .post(&self.pairing_url)
            .timeout(REMOTE_CONTROL_PAIRING_TIMEOUT)
            .bearer_auth(&self.remote_control_token)
            .json(&request)
            .send()
            .await
            .map_err(|err| {
                io::Error::other(format!(
                    "failed to start remote control pairing at `{}`: {err}",
                    self.pairing_url
                ))
            })?;
        let headers = response.headers().clone();
        let status = response.status();
        let body = response.bytes().await.map_err(|err| {
            io::Error::other(format!(
                "failed to read remote control pairing response from `{}`: {err}",
                self.pairing_url
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
                    self.pairing_url,
                    format_headers(&headers)
                ),
            ));
        }

        let pairing = serde_json::from_slice::<StartRemoteControlPairingResponse>(&body).map_err(
            |err| {
                io::Error::other(format!(
                    "failed to parse remote control pairing response from `{}`: HTTP {status}, {}, body: {body_preview}, decode error: {err}",
                    self.pairing_url,
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
            return Err(io::Error::new(
                ErrorKind::InvalidData,
                format!(
                    "remote control pairing returned mismatched enrollment: expected server_id={}, environment_id={}; got server_id={}, environment_id={}",
                    self.server_id, self.environment_id, server_id, environment_id
                ),
            ));
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
