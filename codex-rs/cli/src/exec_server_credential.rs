//! Reads scoped registry credentials from a trusted parent's stdin.
//!
//! The parent sends newline-delimited JSON frames with `version: 1`,
//! `environment_id`, `token`, and `expires_at` (Unix seconds). A frame, including
//! its newline, must be at most 16 KiB; a valid first frame is required within
//! 30 seconds. Each valid frame replaces the current credential. Frames for another
//! environment or otherwise invalid input clear it and stop the executor, as does
//! EOF. Requests reject expired credentials, so the parent must renew before expiry.
//! Only registry requests carry the credential, as an Authorization bearer.

use std::fmt;
use std::io::BufRead;
use std::sync::Arc;
use std::time::Duration;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use codex_api::AuthError;
use codex_api::AuthProvider;
use http::HeaderMap;
use http::HeaderValue;
use http::header::AUTHORIZATION;
use serde::Deserialize;
use tokio::sync::oneshot;
use tokio::sync::watch;

const MAX_FRAME_BYTES: usize = 16 * 1024;
const INITIAL_CREDENTIAL_TIMEOUT: Duration = Duration::from_secs(30);

pub(super) struct ExecutorCredentialPipe {
    pub(super) provider: Arc<ExecutorCredentialProvider>,
    pub(super) parent_closed: oneshot::Receiver<()>,
}

impl ExecutorCredentialPipe {
    pub(super) fn from_stdin(environment_id: String) -> Self {
        Self::spawn_with_reader(environment_id, std::io::stdin)
    }

    fn spawn_with_reader<R: std::io::Read, F: FnOnce() -> R + Send + 'static>(
        environment_id: String,
        open_reader: F,
    ) -> Self {
        let (updates, current) = watch::channel(CredentialState::Pending);
        let (closed, parent_closed) = oneshot::channel();
        std::thread::spawn(move || {
            let mut reader = std::io::BufReader::new(open_reader());
            let result = read_frames(&mut reader, &environment_id, &updates);
            updates.send_replace(match result {
                Ok(()) => CredentialState::Closed,
                Err(error) => CredentialState::Failed(error),
            });
            let _ = closed.send(());
        });
        Self {
            provider: Arc::new(ExecutorCredentialProvider { current }),
            parent_closed,
        }
    }
}

pub(super) struct ExecutorCredentialProvider {
    current: watch::Receiver<CredentialState>,
}

impl fmt::Debug for ExecutorCredentialProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ExecutorCredentialProvider")
            .finish_non_exhaustive()
    }
}

impl ExecutorCredentialProvider {
    pub(super) async fn wait_for_initial(&self) -> Result<(), CredentialError> {
        self.wait_for_initial_with_timeout(INITIAL_CREDENTIAL_TIMEOUT)
            .await
    }

    async fn wait_for_initial_with_timeout(
        &self,
        timeout: Duration,
    ) -> Result<(), CredentialError> {
        let mut current = self.current.clone();
        tokio::time::timeout(timeout, async move {
            loop {
                match &*current.borrow_and_update() {
                    CredentialState::Pending => {}
                    state => return state.header().map(|_| ()),
                }
                current
                    .changed()
                    .await
                    .map_err(|_| CredentialError::Closed)?;
            }
        })
        .await
        .map_err(|_| CredentialError::InitialTimeout)?
    }

    pub(super) fn check_pipe_failure(&self) -> Result<(), CredentialError> {
        if let CredentialState::Failed(error) = &*self.current.borrow() {
            return Err(*error);
        }
        Ok(())
    }
}

impl AuthProvider for ExecutorCredentialProvider {
    fn add_auth_headers(&self, headers: &mut HeaderMap) -> Result<(), AuthError> {
        let token = self
            .current
            .borrow()
            .header()
            .map_err(|error| AuthError::Build(error.to_string()))?;
        let token = token
            .to_str()
            .map_err(|_| AuthError::Build(CredentialError::InvalidFrame.to_string()))?;
        let mut authorization = HeaderValue::from_str(&format!("Bearer {token}"))
            .map_err(|_| AuthError::Build(CredentialError::InvalidFrame.to_string()))?;
        authorization.set_sensitive(true);
        headers.insert(AUTHORIZATION, authorization);
        Ok(())
    }

    fn refresh_after_unauthorized(
        &self,
        rejected_headers: HeaderMap,
    ) -> codex_api::AuthRefreshFuture<'_> {
        Box::pin(async move {
            // The parent owns refresh; only retry if it already supplied a valid replacement.
            self.apply_auth_headers(HeaderMap::new())
                .await
                .is_ok_and(|headers| {
                    headers.get(AUTHORIZATION) != rejected_headers.get(AUTHORIZATION)
                })
        })
    }
}

enum CredentialState {
    Pending,
    Active { token: HeaderValue, expires_at: u64 },
    Failed(CredentialError),
    Closed,
}

impl CredentialState {
    fn header(&self) -> Result<HeaderValue, CredentialError> {
        match self {
            Self::Active { token, expires_at } if *expires_at > unix_timestamp()? => {
                Ok(token.clone())
            }
            Self::Active { .. } => Err(CredentialError::Expired),
            Self::Failed(error) => Err(*error),
            Self::Pending => Err(CredentialError::Missing),
            Self::Closed => Err(CredentialError::Closed),
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CredentialFrame {
    version: u8,
    environment_id: String,
    token: String,
    expires_at: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum CredentialError {
    InvalidFrame,
    FrameTooLarge,
    WrongEnvironment,
    Expired,
    Missing,
    Closed,
    Read,
    Clock,
    InitialTimeout,
}

impl fmt::Display for CredentialError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidFrame => "invalid exec-server executor credential frame",
            Self::FrameTooLarge => "exec-server executor credential frame exceeds the size limit",
            Self::WrongEnvironment => {
                "exec-server executor credential is for a different environment"
            }
            Self::Expired => "exec-server executor credential is expired",
            Self::Missing => "exec-server executor credential is unavailable",
            Self::Closed => "exec-server executor credential pipe closed",
            Self::Read => "could not read exec-server executor credential pipe",
            Self::Clock => "could not check exec-server executor credential expiration",
            Self::InitialTimeout => {
                "timed out waiting for the initial exec-server executor credential"
            }
        })
    }
}

impl std::error::Error for CredentialError {}

fn read_frames(
    reader: &mut impl BufRead,
    environment_id: &str,
    updates: &watch::Sender<CredentialState>,
) -> Result<(), CredentialError> {
    let mut frame = Vec::new();
    loop {
        let chunk = reader.fill_buf().map_err(|_| CredentialError::Read)?;
        if chunk.is_empty() {
            return if frame.is_empty() {
                Ok(())
            } else {
                Err(CredentialError::InvalidFrame)
            };
        }
        let newline = chunk.iter().position(|byte| *byte == b'\n');
        let consumed = newline.map_or(chunk.len(), |index| index + 1);
        if frame.len() + consumed > MAX_FRAME_BYTES {
            return Err(CredentialError::FrameTooLarge);
        }
        frame.extend_from_slice(&chunk[..consumed]);
        reader.consume(consumed);
        if newline.is_some() {
            let parsed = parse_frame(&frame, environment_id)?;
            updates.send_replace(parsed);
            frame.clear();
        }
    }
}

fn parse_frame(frame: &[u8], environment_id: &str) -> Result<CredentialState, CredentialError> {
    let frame: CredentialFrame =
        serde_json::from_slice(frame).map_err(|_| CredentialError::InvalidFrame)?;
    if frame.version != 1
        || frame.token.is_empty()
        || !frame.token.bytes().all(|byte| byte.is_ascii_graphic())
    {
        return Err(CredentialError::InvalidFrame);
    }
    if environment_id.is_empty() || frame.environment_id != environment_id {
        return Err(CredentialError::WrongEnvironment);
    }
    if frame.expires_at <= unix_timestamp()? {
        return Err(CredentialError::Expired);
    }
    let mut token =
        HeaderValue::from_str(&frame.token).map_err(|_| CredentialError::InvalidFrame)?;
    token.set_sensitive(true);
    Ok(CredentialState::Active {
        token,
        expires_at: frame.expires_at,
    })
}

fn unix_timestamp() -> Result<u64, CredentialError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .map_err(|_| CredentialError::Clock)
}

#[cfg(test)]
#[path = "exec_server_credential_tests.rs"]
mod tests;
