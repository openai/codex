use std::path::Path;
use std::process::Command;
use std::process::Stdio;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use serde_json::Value;
use serde_json::json;
use tokio::sync::Notify;

#[derive(Clone, Default)]
pub(crate) struct JsonLogCapture {
    lines: Arc<Mutex<Vec<String>>>,
    updated: Arc<Notify>,
}

impl JsonLogCapture {
    pub(crate) fn record(&self, line: String) {
        self.lines
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(line);
        self.updated.notify_one();
    }

    pub(crate) async fn wait_for_event(&self, event_name: &str) -> Result<Value> {
        let mut events = self.wait_for_events(event_name, /*count*/ 1).await?;
        Ok(events.remove(0))
    }

    pub(crate) async fn wait_for_events(
        &self,
        event_name: &str,
        count: usize,
    ) -> Result<Vec<Value>> {
        let result = tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                let updated = self.updated.notified();
                let events = self
                    .events()?
                    .into_iter()
                    .filter(|event| event["fields"]["event.name"].as_str() == Some(event_name))
                    .collect::<Vec<_>>();
                if events.len() >= count {
                    return Ok(events);
                }
                updated.await;
            }
        })
        .await;
        match result {
            Ok(result) => result,
            Err(_) => {
                let lines = self
                    .lines
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .join("\n");
                anyhow::bail!(
                    "timed out waiting for {count} JSON log event(s) named `{event_name}`; captured stderr:\n{lines}"
                )
            }
        }
    }

    pub(crate) fn events(&self) -> Result<Vec<Value>> {
        let lines = self
            .lines
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        json_log_events(lines.iter().map(String::as_str))
    }
}

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

    let events = json_log_events(stderr.lines())
        .with_context(|| format!("app-server stderr was not valid JSONL: {stderr}"))?;
    let event = events
        .iter()
        .find(|event| event["fields"]["message"] == "processor task exited")
        .context("missing INFO shutdown event in app-server JSON logs")?;
    Ok(json!({
        "level": event["level"],
        "fields": event["fields"],
        "target": event["target"],
    }))
}

fn json_log_events<'a>(lines: impl IntoIterator<Item = &'a str>) -> Result<Vec<Value>> {
    lines
        .into_iter()
        .filter(|line| !line.is_empty())
        .map(|line| {
            let event = serde_json::from_str::<Value>(line)
                .with_context(|| format!("log line was not JSON: {line}"))?;
            anyhow::ensure!(
                event["level"].is_string()
                    && event["fields"].is_object()
                    && event["target"].is_string(),
                "JSON log event did not include level, fields, and target: {line}"
            );
            let timestamp = event["timestamp"]
                .as_str()
                .with_context(|| format!("JSON log event did not include a timestamp: {line}"))?;
            chrono::DateTime::parse_from_rfc3339(timestamp).with_context(|| {
                format!("JSON log event timestamp was not RFC 3339: {timestamp}")
            })?;
            Ok(event)
        })
        .collect()
}
