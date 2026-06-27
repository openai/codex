use std::path::Path;
use std::process::Command;
use std::process::Stdio;

use anyhow::Context;
use anyhow::Result;
use serde_json::Value;
use serde_json::json;

pub fn app_server_json_shutdown_event(
    binary: &str,
    args: &[&str],
    codex_home: &Path,
) -> Result<Value> {
    std::fs::write(
        codex_home.join("config.toml"),
        "[features]\nplugins = false\n",
    )?;
    let output = Command::new(codex_utils_cargo_bin::cargo_bin(binary)?)
        .stdin(Stdio::null())
        .env("CODEX_HOME", codex_home)
        .env(
            "CODEX_APP_SERVER_MANAGED_CONFIG_PATH",
            codex_home.join("managed_config.toml"),
        )
        .env("LOG_FORMAT", "json")
        .env("RUST_LOG", "codex_app_server=info")
        .args(args)
        .output()?;

    let stderr = String::from_utf8(output.stderr)?;
    anyhow::ensure!(output.status.success(), "app-server failed: {stderr}");

    let events = stderr
        .lines()
        .filter(|line| !line.is_empty())
        .map(serde_json::from_str::<Value>)
        .collect::<serde_json::Result<Vec<_>>>()
        .with_context(|| format!("app-server stderr was not JSONL: {stderr}"))?;
    let event = events
        .iter()
        .find(|event| event["fields"]["message"] == "processor task exited")
        .context("missing INFO shutdown event in app-server JSON logs")?;
    let timestamp = event["timestamp"]
        .as_str()
        .context("shutdown event did not include a timestamp")?;
    chrono::DateTime::parse_from_rfc3339(timestamp)
        .with_context(|| format!("shutdown event timestamp was not RFC 3339: {timestamp}"))?;

    Ok(json!({
        "level": event["level"],
        "fields": event["fields"],
        "target": event["target"],
    }))
}
