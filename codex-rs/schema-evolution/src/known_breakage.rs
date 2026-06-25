use crate::ApiSchema;
use crate::SchemaBreakage;
use crate::ViolationKind;
use crate::find_request_narrowing;
use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

const KNOWN_BREAKAGE_LOG_VERSION: u32 = 1;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct KnownBreakage {
    pub id: u64,
    pub kind: ViolationKind,
    pub method: String,
    pub path: String,
    pub before_json: String,
    pub after_json: String,
    pub justification: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct KnownBreakageLog {
    version: u32,
    #[serde(default)]
    breakages: Vec<KnownBreakage>,
}

#[derive(Serialize)]
struct KnownBreakageTemplate<'a> {
    breakages: &'a [KnownBreakage],
}

impl KnownBreakage {
    fn from_breakage(id: u64, breakage: &SchemaBreakage) -> Self {
        Self {
            id,
            kind: breakage.kind.clone(),
            method: breakage.method.clone(),
            path: breakage.path.clone(),
            before_json: canonical_json(&breakage.before),
            after_json: canonical_json(&breakage.after),
            justification: String::new(),
        }
    }

    fn matches(&self, breakage: &SchemaBreakage) -> bool {
        self.kind == breakage.kind
            && self.method == breakage.method
            && self.path == breakage.path
            && self.before_json == canonical_json(&breakage.before)
            && self.after_json == canonical_json(&breakage.after)
    }
}

impl KnownBreakageLog {
    /// Parses one versioned, append-only known-breakage log.
    pub fn parse(contents: &str, label: &str) -> Result<Self> {
        toml::from_str(contents).with_context(|| format!("parse known-breakage log {label}"))
    }
}

/// Checks request-schema narrowing and verifies that each breakage was newly recorded.
///
/// The earlier log must be an exact prefix of the current log. This makes the log an
/// append-only history and prevents an older entry from acknowledging a new change.
pub fn check_request_narrowing(
    before: &ApiSchema,
    after: &ApiSchema,
    before_log: &KnownBreakageLog,
    after_log: &KnownBreakageLog,
) -> Result<Vec<SchemaBreakage>> {
    let breakages = find_request_narrowing(before, after)?;
    let problems = known_breakage_problems(before_log, after_log, &breakages);
    if !problems.is_empty() {
        bail!(failure_report(
            &problems, &breakages, before_log, after_log
        )?);
    }
    Ok(breakages)
}

fn known_breakage_problems(
    before_log: &KnownBreakageLog,
    after_log: &KnownBreakageLog,
    breakages: &[SchemaBreakage],
) -> Vec<String> {
    let mut problems = validate_log("before", before_log);
    problems.extend(validate_log("after", after_log));

    for (index, previous) in before_log.breakages.iter().enumerate() {
        match after_log.breakages.get(index) {
            Some(current) if current == previous => {}
            Some(_) => problems.push(format!(
                "known breakage {} was edited or reordered; existing entries are append-only",
                previous.id
            )),
            None => problems.push(format!(
                "known breakage {} was deleted; existing entries are append-only",
                previous.id
            )),
        }
    }

    let appended = after_log
        .breakages
        .get(before_log.breakages.len()..)
        .unwrap_or_default();
    for breakage in breakages {
        match appended
            .iter()
            .filter(|entry| entry.matches(breakage))
            .count()
        {
            0 => problems.push(format!(
                "missing a new known-breakage entry for {} {:?} at {}",
                breakage.method, breakage.kind, breakage.path
            )),
            1 => {}
            _ => problems.push(format!(
                "multiple new known-breakage entries match {} at {}",
                breakage.method, breakage.path
            )),
        }
    }
    for entry in appended {
        if !breakages.iter().any(|breakage| entry.matches(breakage)) {
            problems.push(format!(
                "new known breakage {} does not match a detected request-schema breakage",
                entry.id
            ));
        }
    }
    problems
}

fn validate_log(label: &str, log: &KnownBreakageLog) -> Vec<String> {
    let mut problems = Vec::new();
    if log.version != KNOWN_BREAKAGE_LOG_VERSION {
        problems.push(format!(
            "{label} known-breakage log must use version {KNOWN_BREAKAGE_LOG_VERSION}"
        ));
    }
    for (index, entry) in log.breakages.iter().enumerate() {
        let expected_id = index as u64 + 1;
        if entry.id != expected_id {
            problems.push(format!(
                "{label} known-breakage entry {} must have id {expected_id}",
                entry.id
            ));
        }
        if entry.justification.trim().is_empty() {
            problems.push(format!(
                "{label} known breakage {} needs a justification",
                entry.id
            ));
        }
        if entry.method.trim().is_empty() {
            problems.push(format!(
                "{label} known breakage {} needs a method",
                entry.id
            ));
        }
        if entry.path.trim().is_empty() {
            problems.push(format!("{label} known breakage {} needs a path", entry.id));
        }
        validate_snapshot(
            label,
            entry,
            "before_json",
            &entry.before_json,
            &mut problems,
        );
        validate_snapshot(label, entry, "after_json", &entry.after_json, &mut problems);
    }
    problems
}

fn validate_snapshot(
    label: &str,
    entry: &KnownBreakage,
    field: &str,
    snapshot: &str,
    problems: &mut Vec<String>,
) {
    if serde_json::from_str::<Value>(snapshot).is_err() {
        problems.push(format!(
            "{label} known breakage {} has invalid JSON in {field}",
            entry.id
        ));
    }
}

fn failure_report(
    problems: &[String],
    breakages: &[SchemaBreakage],
    before_log: &KnownBreakageLog,
    after_log: &KnownBreakageLog,
) -> Result<String> {
    let mut report = String::from("request schema compatibility lint failed:\n");
    for problem in problems {
        report.push_str(&format!("- {problem}\n"));
    }
    for breakage in breakages {
        report.push_str(&format!(
            "- {} {:?} at {}: {} -> {}\n",
            breakage.method, breakage.kind, breakage.path, breakage.before, breakage.after
        ));
    }

    let missing = if log_can_be_extended(before_log, after_log, breakages) {
        let appended = &after_log.breakages[before_log.breakages.len()..];
        breakages
            .iter()
            .filter(|breakage| !appended.iter().any(|entry| entry.matches(breakage)))
            .enumerate()
            .map(|(index, breakage)| {
                KnownBreakage::from_breakage(
                    after_log.breakages.len() as u64 + index as u64 + 1,
                    breakage,
                )
            })
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    if !missing.is_empty() {
        report.push_str(&format!(
            "\nappend one entry per breakage to the known-breakage log:\n{}\n",
            toml::to_string_pretty(&KnownBreakageTemplate {
                breakages: &missing
            })?
        ));
    }
    Ok(report)
}

fn log_can_be_extended(
    before_log: &KnownBreakageLog,
    after_log: &KnownBreakageLog,
    breakages: &[SchemaBreakage],
) -> bool {
    if !validate_log("before", before_log).is_empty()
        || !validate_log("after", after_log).is_empty()
        || !after_log.breakages.starts_with(&before_log.breakages)
    {
        return false;
    }
    let appended = &after_log.breakages[before_log.breakages.len()..];
    appended.iter().all(|entry| {
        breakages
            .iter()
            .filter(|breakage| entry.matches(breakage))
            .count()
            == 1
    }) && breakages.iter().all(|breakage| {
        appended
            .iter()
            .filter(|entry| entry.matches(breakage))
            .count()
            <= 1
    })
}

fn canonical_json(value: &Value) -> String {
    serde_json::to_string(value).unwrap_or_default()
}

#[cfg(test)]
#[path = "known_breakage_tests.rs"]
mod tests;
