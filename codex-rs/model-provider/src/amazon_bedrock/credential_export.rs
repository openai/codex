//! Caches Bedrock signing credentials and provides an exclusive refresh guard.
//! The caller holds the guard across login and export to coalesce auth recovery.
//! Exported secrets remain in memory; command output is bounded and never logged.

use std::io;
use std::ops::Deref;
use std::path::Component;
use std::path::Path;
use std::process::Stdio;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::SystemTime;

use chrono::DateTime;
use chrono::Utc;
use codex_aws_auth::AwsAccessKeys;
use codex_aws_auth::AwsCredentialsProvider;
use codex_model_provider_info::AwsCredentialExportConfig;
use serde::Deserialize;
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio::sync::Mutex;
use tokio::sync::MutexGuard;

const DEFAULT_CREDENTIAL_LIFETIME: Duration = Duration::from_secs(60 * 60);
const CREDENTIAL_REFRESH_WINDOW: Duration = Duration::from_secs(5 * 60);
const MAX_CREDENTIAL_OUTPUT_BYTES: u64 = 64 * 1024;

#[derive(Debug)]
pub(crate) struct AwsCredentialExport {
    config: AwsCredentialExportConfig,
    cached: Mutex<Option<CachedCredentials>>,
    recovery_generation: AtomicU64,
}

#[derive(Debug)]
struct CachedCredentials {
    access_keys: AwsAccessKeys,
    refresh_at: SystemTime,
}

/// Keeps cached credentials invalidated while the caller reauthenticates and exports.
pub(super) struct CredentialExportRefresh<'a> {
    exporter: &'a AwsCredentialExport,
    cached: MutexGuard<'a, Option<CachedCredentials>>,
}

impl CredentialExportRefresh<'_> {
    pub(super) async fn refresh(mut self) -> io::Result<()> {
        *self.cached = Some(self.exporter.export_credentials().await?);
        self.exporter
            .recovery_generation
            .fetch_add(1, Ordering::Release);
        Ok(())
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum CredentialOutput {
    Sts {
        #[serde(rename = "Credentials")]
        credentials: ExportedCredentials,
    },
    Process(ExportedCredentials),
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct ExportedCredentials {
    access_key_id: String,
    secret_access_key: String,
    session_token: Option<String>,
    expiration: Option<String>,
}

impl CachedCredentials {
    fn from_output(output: &[u8], now: SystemTime) -> io::Result<Self> {
        let output: CredentialOutput = serde_json::from_slice(output).map_err(|_| {
            // Serde's detailed errors may quote credential values.
            io::Error::new(
                io::ErrorKind::InvalidData,
                "AWS credential export returned invalid credentials JSON",
            )
        })?;
        let credentials = match output {
            CredentialOutput::Sts { credentials } | CredentialOutput::Process(credentials) => {
                credentials
            }
        };
        if credentials.access_key_id.trim().is_empty()
            || credentials.secret_access_key.trim().is_empty()
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "AWS credential export returned empty access keys",
            ));
        }
        let refresh_at = if let Some(expiration) = credentials.expiration {
            let expiration = DateTime::parse_from_rfc3339(&expiration).map_err(|_| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "AWS credential export Expiration must be an RFC 3339 timestamp",
                )
            })?;
            let lifetime = expiration
                .signed_duration_since(DateTime::<Utc>::from(now))
                .to_std()
                .ok()
                .filter(|lifetime| !lifetime.is_zero())
                .ok_or_else(|| {
                    io::Error::other("AWS credential export returned expired credentials")
                })?;
            now.checked_add(lifetime.saturating_sub(CREDENTIAL_REFRESH_WINDOW))
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        "AWS credential export Expiration is out of range",
                    )
                })?
        } else {
            now + DEFAULT_CREDENTIAL_LIFETIME
        };
        Ok(Self {
            access_keys: AwsAccessKeys {
                access_key_id: credentials.access_key_id,
                secret_access_key: credentials.secret_access_key,
                session_token: credentials.session_token,
            },
            refresh_at,
        })
    }
}

impl AwsCredentialExport {
    pub(crate) fn new(config: AwsCredentialExportConfig) -> Self {
        Self {
            config,
            cached: Mutex::new(None),
            recovery_generation: AtomicU64::new(0),
        }
    }

    /// Returns None when another overlapping recovery has already completed.
    /// Dropping the guard before refresh succeeds leaves the cache empty.
    pub(super) async fn begin_refresh(&self) -> Option<CredentialExportRefresh<'_>> {
        let generation = self.recovery_generation.load(Ordering::Acquire);
        let mut cached = self.cached.lock().await;
        if self.recovery_generation.load(Ordering::Acquire) != generation {
            return None;
        }
        *cached = None;
        Some(CredentialExportRefresh {
            exporter: self,
            cached,
        })
    }

    async fn export_credentials(&self) -> io::Result<CachedCredentials> {
        let program = Path::new(&self.config.command);
        let mut components = program.components();
        let is_bare_command = matches!(
            (components.next(), components.next()),
            (Some(Component::Normal(name)), None) if name == program.as_os_str()
        );
        if self.config.command.trim().is_empty() || (!program.is_absolute() && !is_bare_command) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "AWS credential export command must be an absolute path or a bare executable name",
            ));
        }

        let mut command = Command::from(codex_utils_process::background_command(program));
        command
            .args(self.config.args.iter().map(Deref::deref))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true);

        let mut child = command.spawn().map_err(|error| {
            io::Error::new(
                error.kind(),
                format!(
                    "failed to run AWS credential export command `{}`: {error}",
                    self.config.command
                ),
            )
        })?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| io::Error::other("AWS credential export stdout is unavailable"))?;
        let output = tokio::time::timeout(self.config.timeout(), async {
            let mut output = Vec::new();
            stdout
                .take(MAX_CREDENTIAL_OUTPUT_BYTES + 1)
                .read_to_end(&mut output)
                .await?;
            if output.len() as u64 > MAX_CREDENTIAL_OUTPUT_BYTES {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "AWS credential export output exceeds 64 KiB",
                ));
            }
            let status = child.wait().await?;
            if !status.success() {
                return Err(io::Error::other(format!(
                    "AWS credential export command exited with {status}"
                )));
            }
            Ok(output)
        })
        .await
        .map_err(|_| {
            io::Error::new(
                io::ErrorKind::TimedOut,
                format!(
                    "AWS credential export command timed out after {} ms",
                    self.config.timeout_ms
                ),
            )
        })??;
        CachedCredentials::from_output(&output, SystemTime::now())
    }
}

impl AwsCredentialsProvider for AwsCredentialExport {
    #[expect(
        clippy::await_holding_invalid_type,
        reason = "cache misses serialize command execution to avoid duplicate credential exports"
    )]
    async fn credentials(&self) -> std::io::Result<AwsAccessKeys> {
        let mut cached = self.cached.lock().await;
        if let Some(credentials) = cached.as_ref()
            && SystemTime::now() < credentials.refresh_at
        {
            return Ok(credentials.access_keys.clone());
        }
        let credentials = self.export_credentials().await?;
        let access_keys = credentials.access_keys.clone();
        *cached = Some(credentials);
        Ok(access_keys)
    }
}

#[cfg(test)]
#[path = "credential_export_tests.rs"]
mod tests;
