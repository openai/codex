//! Runs credential exporters directly with bounded output, timeout, and sanitized diagnostics.

use std::io;
use std::ops::Deref;
use std::process::Stdio;

use codex_model_provider_info::CredentialExportConfig;
use tokio::io::AsyncReadExt;
use tokio::process::Command;

const MAX_CREDENTIAL_OUTPUT_BYTES: u64 = 64 * 1024;

/// Captures stdout for the caller to parse in memory. Stderr is discarded.
pub async fn run_credential_export(
    config: &CredentialExportConfig,
    provider: &str,
) -> io::Result<Vec<u8>> {
    config
        .validate("credential_export")
        .map_err(|message| io::Error::new(io::ErrorKind::InvalidInput, message))?;
    let program = &config.command;
    let mut command = Command::from(codex_utils_process::background_command(program));
    command
        .args(config.args.iter().map(Deref::deref))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);

    let mut child = command.spawn().map_err(|error| {
        io::Error::new(
            error.kind(),
            format!(
                "failed to run {provider} credential export command `{}`: {error}",
                config.command
            ),
        )
    })?;
    let stdout = child.stdout.take().ok_or_else(|| {
        io::Error::other(format!(
            "{provider} credential export stdout is unavailable"
        ))
    })?;
    tokio::time::timeout(config.timeout(), async {
        let mut output = Vec::new();
        stdout
            .take(MAX_CREDENTIAL_OUTPUT_BYTES + 1)
            .read_to_end(&mut output)
            .await?;
        if output.len() as u64 > MAX_CREDENTIAL_OUTPUT_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{provider} credential export output exceeds 64 KiB"),
            ));
        }
        let status = child.wait().await?;
        if !status.success() {
            return Err(io::Error::other(format!(
                "{provider} credential export command exited with {status}"
            )));
        }
        Ok(output)
    })
    .await
    .map_err(|_| {
        io::Error::new(
            io::ErrorKind::TimedOut,
            format!(
                "{provider} credential export command timed out after {} ms",
                config.timeout_ms
            ),
        )
    })?
}
