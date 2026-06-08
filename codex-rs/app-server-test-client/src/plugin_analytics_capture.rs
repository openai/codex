use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use serde_json::Value;
use std::fs;
use std::io;
use std::path::Path;
use std::thread;
use std::time::Duration;
use std::time::Instant;

const CAPTURE_READY_TIMEOUT: Duration = Duration::from_secs(5);
const CAPTURE_READY_POLL_INTERVAL: Duration = Duration::from_millis(50);

pub(super) fn wait_until_capture_is_ready(path: &Path) -> Result<()> {
    let deadline = Instant::now() + CAPTURE_READY_TIMEOUT;
    loop {
        match fs::metadata(path) {
            Ok(_) => return Ok(()),
            Err(err) if err.kind() == io::ErrorKind::NotFound => {}
            Err(err) => {
                return Err(err)
                    .with_context(|| format!("inspect capture file {}", path.display()));
            }
        }
        if Instant::now() >= deadline {
            anyhow::bail!(
                "analytics capture did not become ready at {}; use an authenticated profile and a debug Codex binary",
                path.display()
            );
        }
        thread::sleep(CAPTURE_READY_POLL_INTERVAL);
    }
}

pub(super) fn read_events_for_remote_plugin(
    path: &Path,
    remote_plugin_id: &str,
) -> Result<Vec<Value>> {
    let contents = match fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => {
            return Err(err).with_context(|| format!("read capture file {}", path.display()));
        }
    };
    let mut matching = Vec::new();
    for (index, line) in contents.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let payload: Value = serde_json::from_str(line).with_context(|| {
            format!(
                "parse analytics capture line {} from {}",
                index + 1,
                path.display()
            )
        })?;
        let events = payload["events"]
            .as_array()
            .context("analytics capture payload is missing events")?;
        matching.extend(
            events
                .iter()
                .filter(|event| event["event_params"]["remote_plugin_id"] == remote_plugin_id)
                .cloned(),
        );
    }
    Ok(matching)
}

pub(super) struct PluginEventIdentity<'a> {
    pub(super) plugin_id: &'a str,
    pub(super) remote_plugin_id: &'a str,
    pub(super) plugin_name: &'a str,
    pub(super) marketplace_name: &'a str,
}

pub(super) fn validate_mutation_events(
    events: Vec<Value>,
    expected: PluginEventIdentity<'_>,
) -> Result<Vec<Value>> {
    let mut validated = Vec::new();
    for event_type in ["codex_plugin_installed", "codex_plugin_uninstalled"] {
        let matching = events
            .iter()
            .filter(|event| event["event_type"] == event_type)
            .collect::<Vec<_>>();
        let [event] = matching.as_slice() else {
            bail!(
                "expected exactly one `{event_type}` event for `{}`, found {}",
                expected.remote_plugin_id,
                matching.len()
            );
        };
        validate_event(event, &expected)?;
        validated.push((*event).clone());
    }
    Ok(validated)
}

fn validate_event(event: &Value, expected: &PluginEventIdentity<'_>) -> Result<()> {
    let params = &event["event_params"];
    require_string(params, "plugin_id", expected.plugin_id)?;
    require_string(params, "remote_plugin_id", expected.remote_plugin_id)?;
    require_string(params, "plugin_name", expected.plugin_name)?;
    require_string(params, "marketplace_name", expected.marketplace_name)?;
    for field in [
        "has_skills",
        "mcp_server_count",
        "connector_ids",
        "product_client_id",
    ] {
        if params.get(field).is_none_or(Value::is_null) {
            bail!(
                "{} event has null or missing `{field}`",
                event["event_type"]
            );
        }
    }
    Ok(())
}

fn require_string(params: &Value, field: &str, expected: &str) -> Result<()> {
    let actual = params.get(field).and_then(Value::as_str);
    if actual != Some(expected) {
        bail!("expected `{field}` to be `{expected}`, got {actual:?}");
    }
    Ok(())
}

#[cfg(test)]
#[path = "plugin_analytics_capture_tests.rs"]
mod tests;
