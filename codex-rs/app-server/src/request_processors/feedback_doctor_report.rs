//! Builds a redacted doctor report attachment for feedback uploads.
//!
//! Feedback upload should never depend on doctor succeeding. This module runs
//! the configured Codex executable as a subprocess, accepts only valid JSON from
//! `codex doctor --json`, derives a small set of Sentry tags, and otherwise
//! skips the attachment with a warning. Keeping the report generation out of the
//! app-server process avoids sharing doctor internals across crates while still
//! attaching exactly the same JSON a user could copy from the CLI.

use std::collections::BTreeMap;
use std::time::Duration;

use codex_core::config::Config;
use codex_feedback::DOCTOR_REPORT_ATTACHMENT_FILENAME;
use codex_feedback::FeedbackAttachment;
use serde_json::Value;
use tokio::process::Command;
use tokio::time::timeout;
use tracing::warn;

const DOCTOR_FEEDBACK_REPORT_TIMEOUT: Duration = Duration::from_secs(25);
const MAX_DOCTOR_TAG_VALUE_LEN: usize = 256;

/// Redacted doctor report data that can be merged into a feedback upload.
pub(crate) struct DoctorFeedbackReport {
    /// JSON support report to upload as `codex-doctor-report.json`.
    pub(crate) attachment: FeedbackAttachment,
    /// Low-cardinality Sentry tags derived from the report status and check ids.
    pub(crate) tags: BTreeMap<String, String>,
}

/// Runs `codex doctor --json` and returns a best-effort feedback attachment.
///
/// Failure to spawn Codex, finish before the timeout, or parse JSON means the
/// feedback upload proceeds without the doctor report. Callers should merge the
/// returned tags without overriding explicit client-provided tags.
pub(crate) async fn doctor_feedback_report(config: &Config) -> Option<DoctorFeedbackReport> {
    let executable = config
        .codex_self_exe
        .clone()
        .or_else(|| std::env::current_exe().ok())?;

    let mut command = Command::new(&executable);
    command.arg("doctor").arg("--json");
    command.kill_on_drop(/*kill_on_drop*/ true);
    let output = match timeout(DOCTOR_FEEDBACK_REPORT_TIMEOUT, command.output()).await {
        Ok(Ok(output)) => output,
        Ok(Err(err)) => {
            warn!(
                executable = %executable.display(),
                error = %err,
                "failed to run doctor report for feedback; skipping attachment"
            );
            return None;
        }
        Err(_) => {
            warn!(
                executable = %executable.display(),
                "timed out running doctor report for feedback; skipping attachment"
            );
            return None;
        }
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let Some(json_start) = stdout.find('{') else {
        warn!(
            executable = %executable.display(),
            status = %output.status,
            stderr = %String::from_utf8_lossy(&output.stderr),
            "doctor report for feedback did not produce JSON; skipping attachment"
        );
        return None;
    };
    let json = stdout[json_start..].trim();
    let report: Value = match serde_json::from_str(json) {
        Ok(report) => report,
        Err(err) => {
            warn!(
                executable = %executable.display(),
                status = %output.status,
                error = %err,
                "doctor report for feedback was not valid JSON; skipping attachment"
            );
            return None;
        }
    };

    let pretty = serde_json::to_vec_pretty(&report).unwrap_or_else(|_| json.as_bytes().to_vec());
    Some(DoctorFeedbackReport {
        tags: doctor_report_tags(&report),
        attachment: FeedbackAttachment {
            filename: DOCTOR_REPORT_ATTACHMENT_FILENAME.to_string(),
            content_type: Some("application/json".to_string()),
            buffer: pretty,
        },
    })
}

fn doctor_report_tags(report: &Value) -> BTreeMap<String, String> {
    let mut tags = BTreeMap::new();
    if let Some(overall_status) = report.get("overallStatus").and_then(Value::as_str) {
        tags.insert(
            "doctor_overall_status".to_string(),
            truncate_tag_value(overall_status),
        );
    }

    let mut ok_count = 0usize;
    let mut warning_count = 0usize;
    let mut fail_count = 0usize;
    let mut failed_checks = Vec::new();
    let mut warning_checks = Vec::new();
    if let Some(checks) = report.get("checks") {
        for check in check_values(checks) {
            let status = check.get("status").and_then(Value::as_str);
            let id = check.get("id").and_then(Value::as_str).unwrap_or("unknown");
            match status {
                Some("ok") => ok_count += 1,
                Some("warning") => {
                    warning_count += 1;
                    warning_checks.push(id.to_string());
                }
                Some("fail") => {
                    fail_count += 1;
                    failed_checks.push(id.to_string());
                }
                _ => {}
            }
        }
    }
    tags.insert("doctor_ok_count".to_string(), ok_count.to_string());
    tags.insert(
        "doctor_warning_count".to_string(),
        warning_count.to_string(),
    );
    tags.insert("doctor_fail_count".to_string(), fail_count.to_string());
    if !failed_checks.is_empty() {
        tags.insert(
            "doctor_failed_checks".to_string(),
            truncate_tag_value(&failed_checks.join(",")),
        );
    }
    if !warning_checks.is_empty() {
        tags.insert(
            "doctor_warning_checks".to_string(),
            truncate_tag_value(&warning_checks.join(",")),
        );
    }

    tags
}

/// Iterates checks from both the current keyed JSON shape and older array reports.
fn check_values(checks: &Value) -> Box<dyn Iterator<Item = &Value> + '_> {
    match checks {
        Value::Array(values) => Box::new(values.iter()),
        Value::Object(values) => Box::new(values.values()),
        _ => Box::new(std::iter::empty()),
    }
}

fn truncate_tag_value(value: &str) -> String {
    if value.chars().count() <= MAX_DOCTOR_TAG_VALUE_LEN {
        return value.to_string();
    }
    let prefix = value
        .chars()
        .take(MAX_DOCTOR_TAG_VALUE_LEN.saturating_sub(3))
        .collect::<String>();
    format!("{prefix}...")
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    #[test]
    fn doctor_report_tags_summarize_status_counts() {
        let report = json!({
            "overallStatus": "fail",
            "checks": {
                "runtime.provenance": {"id": "runtime.provenance", "status": "ok"},
                "websocket.reachability": {
                    "id": "websocket.reachability",
                    "status": "warning"
                },
                "auth.credentials": {"id": "auth.credentials", "status": "fail"}
            }
        });

        let tags = doctor_report_tags(&report);

        let expected = BTreeMap::from([
            ("doctor_fail_count".to_string(), "1".to_string()),
            (
                "doctor_failed_checks".to_string(),
                "auth.credentials".to_string(),
            ),
            ("doctor_ok_count".to_string(), "1".to_string()),
            ("doctor_overall_status".to_string(), "fail".to_string()),
            (
                "doctor_warning_checks".to_string(),
                "websocket.reachability".to_string(),
            ),
            ("doctor_warning_count".to_string(), "1".to_string()),
        ]);
        assert_eq!(tags, expected);
    }
}
