//! Runs one local HTTP headers helper with bounded output, time and process-tree lifetime.
//! Callers choose the command environment, working directory and refresh policy.

use anyhow::Result;
use anyhow::anyhow;
use http::HeaderMap;
use http::HeaderName;
use http::HeaderValue;
use serde::de::Deserializer as _;
use serde::de::MapAccess;
use serde::de::Visitor;
use std::fmt;
#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::process::Command;
use std::process::Stdio;
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::process::Child;

const HELPER_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_HELPER_OUTPUT_BYTES: usize = 64 * 1024;

struct HelperProcess {
    child: Child,
    #[cfg(unix)]
    process_group_id: u32,
    #[cfg(windows)]
    job: codex_utils_pty::JobObject,
}

impl Drop for HelperProcess {
    fn drop(&mut self) {
        #[cfg(unix)]
        let _ = codex_utils_pty::process_group::kill_process_group(self.process_group_id);
        #[cfg(windows)]
        let _ = self.job.terminate();
        let _ = self.child.start_kill();
    }
}

/// Builds a background shell command using the platform's command interpreter.
pub fn shell_command(command: &str) -> Command {
    #[cfg(windows)]
    let shell = std::env::var_os("COMSPEC").unwrap_or_else(|| "cmd.exe".into());
    #[cfg(not(windows))]
    let shell = "sh";

    // Match the repository's existing shell-command convention. The command is ordinary
    // configuration and may be visible in local process metadata; credentials belong in the
    // JSON output rather than in the command text.
    let mut process = codex_utils_process::background_command(shell);
    #[cfg(windows)]
    {
        process.args(["/Q", "/D", "/C"]);
        process.raw_arg(format!(r#""{command}""#));
    }
    #[cfg(not(windows))]
    process.args(["-c", command]);
    process
}

/// Executes a prepared command and validates its JSON headers without exposing output in errors.
pub async fn run(command: Command) -> Result<HeaderMap> {
    let mut process = tokio::process::Command::from(command);
    #[cfg(unix)]
    process.process_group(0);
    process
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);

    #[cfg(windows)]
    let (child, job) = {
        let job = codex_utils_pty::JobObject::create_without_breakaway()
            .map_err(|error| anyhow!("headers helper containment failed: {error}"))?;
        let child = job
            .spawn_contained(&mut process)
            .map_err(|error| anyhow!("headers helper failed to start: {error}"))?;
        (child, job)
    };
    #[cfg(not(windows))]
    let child = process
        .spawn()
        .map_err(|error| anyhow!("headers helper failed to start: {error}"))?;
    let mut process = HelperProcess {
        #[cfg(unix)]
        process_group_id: child
            .id()
            .ok_or_else(|| anyhow!("headers helper process id was unavailable"))?,
        child,
        #[cfg(windows)]
        job,
    };
    let output = tokio::time::timeout(HELPER_TIMEOUT, async {
        let stdout = process
            .child
            .stdout
            .take()
            .ok_or_else(|| anyhow!("headers helper stdout was unavailable"))?;
        let mut output = Vec::new();
        stdout
            .take((MAX_HELPER_OUTPUT_BYTES + 1) as u64)
            .read_to_end(&mut output)
            .await?;
        if output.len() > MAX_HELPER_OUTPUT_BYTES {
            return Err(anyhow!("headers helper output exceeds 64 KiB"));
        }
        let status = process.child.wait().await?;
        if !status.success() {
            return Err(anyhow!("headers helper exited with status {status}"));
        }
        Ok(output)
    })
    .await
    .map_err(|_| anyhow!("headers helper timed out after 10 seconds"))??;

    parse_headers(&output)
}

// Deserialize entries in wire order. Deserializing into a map would silently
// overwrite exact duplicates before HTTP's case-insensitive name check.
struct HeaderEntries;

impl<'de> Visitor<'de> for HeaderEntries {
    type Value = Vec<(String, String)>;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a JSON object of string header names and values")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        let mut entries = Vec::new();
        while let Some(entry) = map.next_entry()? {
            entries.push(entry);
        }
        Ok(entries)
    }
}

fn parse_headers(output: &[u8]) -> Result<HeaderMap> {
    let text =
        std::str::from_utf8(output).map_err(|_| anyhow!("headers helper wrote non-UTF-8 data"))?;
    let mut json = serde_json::Deserializer::from_str(text.trim());
    let entries = json
        .deserialize_map(HeaderEntries)
        .and_then(|entries| {
            json.end()?;
            Ok(entries)
        })
        .map_err(|_| anyhow!("headers helper must output a JSON object of strings"))?;
    let mut parsed = HeaderMap::new();
    for (name, value) in entries {
        let name = HeaderName::from_bytes(name.as_bytes())
            .map_err(|_| anyhow!("headers helper returned an invalid header name"))?;
        let mut value = HeaderValue::from_str(&value)
            .map_err(|_| anyhow!("headers helper returned an invalid header value"))?;
        value.set_sensitive(true);
        if parsed.insert(name, value).is_some() {
            return Err(anyhow!("headers helper returned duplicate header names"));
        }
    }
    Ok(parsed)
}

#[cfg(test)]
#[path = "headers_helper_tests.rs"]
mod tests;
