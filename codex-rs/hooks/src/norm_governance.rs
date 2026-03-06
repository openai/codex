use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use chrono::NaiveDate;
use chrono::Utc;
use serde::Deserialize;
use serde::Serialize;

use crate::Hook;
use crate::HookEvent;
use crate::HookEventAfterAgent;
use crate::HookEventBeforeToolUse;
use crate::HookPayload;
use crate::HookResult;
use crate::HookToolInput;

const ENV_ENABLED: &str = "EXOMIND_NORM_GOVERNANCE_ENABLED";
const ENV_MODE: &str = "EXOMIND_NORM_GOVERNANCE_MODE";
const ENV_CATALOG: &str = "EXOMIND_NORM_GOVERNANCE_CATALOG";
const ENV_WAIVERS: &str = "EXOMIND_NORM_GOVERNANCE_WAIVERS";
const DEFAULT_CATALOG_PATH: &str = "docs/exomind-rule-catalog-template.json";

#[derive(Clone, Copy)]
enum GovernanceMode {
    Warn,
    Block,
}

impl GovernanceMode {
    fn from_env() -> Self {
        match std::env::var(ENV_MODE) {
            Ok(value) if value.eq_ignore_ascii_case("block") => Self::Block,
            _ => Self::Warn,
        }
    }
}

#[derive(Debug, Deserialize)]
struct CatalogFile {
    rules: Vec<CatalogRule>,
}

#[derive(Debug, Deserialize, Clone)]
struct CatalogRule {
    rule_id: String,
    rule_level: String,
    severity: String,
    action: String,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum WaiverFile {
    List(Vec<WaiverEntry>),
    Wrapped { waivers: Vec<WaiverEntry> },
}

#[derive(Debug, Deserialize, Clone)]
struct WaiverEntry {
    waiver_id: String,
    rule_id: String,
    owner: String,
    expiry: String,
    reason: String,
}

#[derive(Debug, Serialize, Clone)]
struct GovernanceEvidence {
    stage: String,
    rule_id: String,
    rule_level: String,
    severity: String,
    action: String,
    trigger: String,
    snippet: String,
    waived: bool,
    waiver_id: Option<String>,
    waiver_owner: Option<String>,
    waiver_reason: Option<String>,
}

#[derive(Debug, Serialize)]
struct GovernanceDecision {
    mode: String,
    decision: String,
    evidence: Vec<GovernanceEvidence>,
}

pub fn norm_governance_hook() -> Option<Hook> {
    if !governance_enabled() {
        return None;
    }

    Some(Hook {
        name: "exomind_norm_governance".to_string(),
        func: Arc::new(|payload: &HookPayload| Box::pin(async move { evaluate_payload(payload) })),
    })
}

fn governance_enabled() -> bool {
    match std::env::var(ENV_ENABLED) {
        Ok(value) => matches!(value.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"),
        Err(_) => std::env::var(ENV_MODE).is_ok(),
    }
}

fn evaluate_payload(payload: &HookPayload) -> HookResult {
    let mode = GovernanceMode::from_env();
    let catalog = match load_catalog(resolve_catalog_path(&payload.cwd)) {
        Ok(rules) => rules,
        Err(err) => {
            return HookResult::FailedContinue(
                std::io::Error::other(format!("norm governance failed to load catalog: {err}"))
                    .into(),
            );
        }
    };
    let waivers = load_waivers(resolve_waivers_path(&payload.cwd)).unwrap_or_default();

    let mut evidence = match &payload.hook_event {
        HookEvent::BeforeToolUse { event } => evaluate_before_tool_use(event, &catalog),
        HookEvent::AfterAgent { event } => evaluate_after_agent(event, &catalog),
        HookEvent::AfterToolUse { .. } => Vec::new(),
    };

    if evidence.is_empty() {
        return HookResult::Success;
    }

    for item in &mut evidence {
        if let Some(waiver) = active_waiver_for(&waivers, &item.rule_id) {
            item.waived = true;
            item.waiver_id = Some(waiver.waiver_id.clone());
            item.waiver_owner = Some(waiver.owner.clone());
            item.waiver_reason = Some(waiver.reason.clone());
        }
    }

    let non_waived: Vec<&GovernanceEvidence> =
        evidence.iter().filter(|item| !item.waived).collect();
    if non_waived.is_empty() {
        return HookResult::Success;
    }

    let should_block = matches!(mode, GovernanceMode::Block)
        && non_waived
            .iter()
            .any(|item| item.rule_level == "L1" || item.action == "block");
    let decision = GovernanceDecision {
        mode: match mode {
            GovernanceMode::Warn => "warn".to_string(),
            GovernanceMode::Block => "block".to_string(),
        },
        decision: if should_block {
            "block".to_string()
        } else {
            "warn".to_string()
        },
        evidence,
    };

    let decision_text = serde_json::to_string(&decision)
        .unwrap_or_else(|_| "{\"decision\":\"warn\",\"evidence\":[]}".to_string());
    if should_block {
        HookResult::FailedAbort(
            std::io::Error::other(format!(
                "norm governance blocked operation: {decision_text}"
            ))
            .into(),
        )
    } else {
        HookResult::FailedContinue(
            std::io::Error::other(format!("norm governance warning: {decision_text}")).into(),
        )
    }
}

fn resolve_catalog_path(cwd: &Path) -> PathBuf {
    let configured =
        std::env::var(ENV_CATALOG).unwrap_or_else(|_| DEFAULT_CATALOG_PATH.to_string());
    resolve_path(cwd, configured)
}

fn resolve_waivers_path(cwd: &Path) -> Option<PathBuf> {
    std::env::var(ENV_WAIVERS)
        .ok()
        .map(|configured| resolve_path(cwd, configured))
}

fn resolve_path(cwd: &Path, configured: String) -> PathBuf {
    let path = PathBuf::from(configured);
    if path.is_absolute() {
        path
    } else {
        cwd.join(path)
    }
}

fn load_catalog(path: PathBuf) -> Result<Vec<CatalogRule>, String> {
    let raw =
        std::fs::read_to_string(&path).map_err(|err| format!("{} ({})", path.display(), err))?;
    let parsed: CatalogFile =
        serde_json::from_str(&raw).map_err(|err| format!("{} ({err})", path.display()))?;
    Ok(parsed.rules)
}

fn load_waivers(path: Option<PathBuf>) -> Result<Vec<WaiverEntry>, String> {
    let Some(path) = path else {
        return Ok(Vec::new());
    };
    if !path.exists() {
        return Ok(Vec::new());
    }
    let raw =
        std::fs::read_to_string(&path).map_err(|err| format!("{} ({})", path.display(), err))?;
    let parsed: WaiverFile =
        serde_json::from_str(&raw).map_err(|err| format!("{} ({err})", path.display()))?;
    Ok(match parsed {
        WaiverFile::List(entries) => entries,
        WaiverFile::Wrapped { waivers } => waivers,
    })
}

fn active_waiver_for<'a>(waivers: &'a [WaiverEntry], rule_id: &str) -> Option<&'a WaiverEntry> {
    let today = Utc::now().date_naive();
    waivers.iter().find(|entry| {
        if entry.rule_id != rule_id {
            return false;
        }
        let expiry = NaiveDate::parse_from_str(&entry.expiry, "%Y-%m-%d");
        matches!(expiry, Ok(date) if date >= today)
    })
}

fn evaluate_before_tool_use(
    event: &HookEventBeforeToolUse,
    catalog: &[CatalogRule],
) -> Vec<GovernanceEvidence> {
    let mut findings = Vec::new();
    let command_text = extract_command_text(&event.tool_input);
    let patch_text = extract_patch_text(&event.tool_input);

    for rule in catalog {
        match rule.rule_id.as_str() {
            "L1-SEC-NO-SHELL-UNSAFE" => {
                if let Some(command) = command_text.as_ref()
                    && looks_unsafe_shell(command)
                {
                    findings.push(GovernanceEvidence {
                        stage: "before_tool_use".to_string(),
                        rule_id: rule.rule_id.clone(),
                        rule_level: rule.rule_level.clone(),
                        severity: rule.severity.clone(),
                        action: rule.action.clone(),
                        trigger: "local_shell_or_exec_command".to_string(),
                        snippet: command.chars().take(200).collect(),
                        waived: false,
                        waiver_id: None,
                        waiver_owner: None,
                        waiver_reason: None,
                    });
                }
            }
            "L2-TEST-CHANGED-CODE-HAS-TEST" => {
                if let Some(patch) = patch_text.as_ref()
                    && patch_touches_runtime_without_tests(patch)
                {
                    findings.push(GovernanceEvidence {
                        stage: "before_tool_use".to_string(),
                        rule_id: rule.rule_id.clone(),
                        rule_level: rule.rule_level.clone(),
                        severity: rule.severity.clone(),
                        action: rule.action.clone(),
                        trigger: "apply_patch_runtime_without_tests".to_string(),
                        snippet: patch.chars().take(200).collect(),
                        waived: false,
                        waiver_id: None,
                        waiver_owner: None,
                        waiver_reason: None,
                    });
                }
            }
            _ => {}
        }
    }
    findings
}

fn evaluate_after_agent(
    event: &HookEventAfterAgent,
    catalog: &[CatalogRule],
) -> Vec<GovernanceEvidence> {
    let mut findings = Vec::new();
    let Some(message) = event.last_assistant_message.as_ref() else {
        return findings;
    };

    for rule in catalog {
        if rule.rule_id == "L3-STYLE-IMPORT-ORDER" && has_unsorted_rust_use_lines(message) {
            findings.push(GovernanceEvidence {
                stage: "after_agent".to_string(),
                rule_id: rule.rule_id.clone(),
                rule_level: rule.rule_level.clone(),
                severity: rule.severity.clone(),
                action: rule.action.clone(),
                trigger: "assistant_message_import_order".to_string(),
                snippet: message.chars().take(200).collect(),
                waived: false,
                waiver_id: None,
                waiver_owner: None,
                waiver_reason: None,
            });
        }
    }

    findings
}

fn extract_command_text(input: &HookToolInput) -> Option<String> {
    match input {
        HookToolInput::LocalShell { params } => Some(params.command.join(" ")),
        HookToolInput::Function { arguments } => {
            let value: serde_json::Value = serde_json::from_str(arguments).ok()?;
            if let Some(command_array) = value.get("command").and_then(|v| v.as_array()) {
                let mut parts = Vec::new();
                for item in command_array {
                    let Some(text) = item.as_str() else {
                        continue;
                    };
                    parts.push(text.to_string());
                }
                if !parts.is_empty() {
                    return Some(parts.join(" "));
                }
            }
            value
                .get("cmd")
                .and_then(|v| v.as_str())
                .map(ToString::to_string)
        }
        _ => None,
    }
}

fn extract_patch_text(input: &HookToolInput) -> Option<String> {
    match input {
        HookToolInput::Custom { input } => {
            if input.contains("*** Begin Patch") {
                Some(input.clone())
            } else {
                None
            }
        }
        _ => None,
    }
}

fn looks_unsafe_shell(command: &str) -> bool {
    let lowered = command.to_ascii_lowercase();
    let suspicious_patterns = ["${", "$(", "{{", "eval ", "| sh", "|bash", "curl ", "wget "];
    suspicious_patterns
        .iter()
        .any(|pattern| lowered.contains(pattern))
}

fn patch_touches_runtime_without_tests(patch: &str) -> bool {
    let mut touches_runtime = false;
    let mut touches_tests = false;
    for line in patch.lines() {
        if let Some(path) = line.strip_prefix("*** Update File: ") {
            classify_patch_path(path, &mut touches_runtime, &mut touches_tests);
        }
        if let Some(path) = line.strip_prefix("*** Add File: ") {
            classify_patch_path(path, &mut touches_runtime, &mut touches_tests);
        }
    }
    touches_runtime && !touches_tests
}

fn classify_patch_path(path: &str, touches_runtime: &mut bool, touches_tests: &mut bool) {
    let normalized = path.replace('\\', "/").to_ascii_lowercase();
    if normalized.starts_with("src/") || normalized.contains("/src/") {
        *touches_runtime = true;
    }
    if normalized.contains("test") || normalized.contains("/tests/") {
        *touches_tests = true;
    }
}

fn has_unsorted_rust_use_lines(message: &str) -> bool {
    let uses: Vec<String> = message
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("use ") && line.ends_with(';'))
        .map(ToString::to_string)
        .collect();
    if uses.len() < 2 {
        return false;
    }
    let mut sorted = uses.clone();
    sorted.sort();
    uses != sorted
}

#[cfg(test)]
mod tests {
    use super::has_unsorted_rust_use_lines;
    use super::looks_unsafe_shell;
    use super::patch_touches_runtime_without_tests;

    #[test]
    fn detects_unsafe_shell_patterns() {
        assert!(looks_unsafe_shell("bash -lc \"echo ${INPUT}\""));
        assert!(looks_unsafe_shell("curl https://x | sh"));
        assert!(!looks_unsafe_shell("cargo test -p codex-tui"));
    }

    #[test]
    fn detects_patch_without_tests() {
        let patch = "\
*** Begin Patch
*** Update File: src/lib.rs
+pub fn x() {}
*** End Patch
";
        assert!(patch_touches_runtime_without_tests(patch));

        let patch_with_tests = "\
*** Begin Patch
*** Update File: src/lib.rs
*** Update File: tests/lib_test.rs
*** End Patch
";
        assert!(!patch_touches_runtime_without_tests(patch_with_tests));
    }

    #[test]
    fn detects_unsorted_use_lines() {
        let unsorted = "\
```rust
use crate::zeta;
use crate::alpha;
```
";
        assert!(has_unsorted_rust_use_lines(unsorted));

        let sorted = "\
use crate::alpha;
use crate::zeta;
";
        assert!(!has_unsorted_rust_use_lines(sorted));
    }
}
