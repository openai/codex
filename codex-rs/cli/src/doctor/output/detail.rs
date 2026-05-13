//! Converts raw doctor detail strings into human-oriented rows.
//!
//! Checks intentionally store details as simple redacted `label: value` strings
//! so JSON serialization and human rendering share the same source data. This
//! module owns the presentation-only transformations: collapsing noisy booleans,
//! truncating long paths for terminal output, grouping repeated values, and
//! keeping the `--all` expansion behavior out of check construction.

use std::collections::BTreeSet;
use std::env;

use super::DoctorCheck;
use super::HumanOutputOptions;
use super::redact_detail;

const LIST_LIMIT: usize = 7;
const PATH_LIMIT: usize = 48;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum HumanDetail {
    Row {
        label: String,
        value: String,
        expected: Option<String>,
    },
    Continuation(String),
    Bullet(String),
    Remedy(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ParsedDetail {
    label: String,
    value: String,
}

pub(super) fn detail_lines(check: &DoctorCheck, options: HumanOutputOptions) -> Vec<HumanDetail> {
    let parsed = parsed_details(check);
    let details = match check.category.as_str() {
        "runtime" => runtime_details(&parsed),
        "install" => install_details(&parsed, options),
        "config" => config_details(&parsed, options),
        "state" => state_details(&parsed),
        _ => generic_details(&parsed),
    };
    let mut details = details
        .into_iter()
        .map(|detail| attach_issue_metadata(detail, check))
        .map(|detail| humanize_detail(detail, options))
        .collect::<Vec<_>>();
    details.extend(issue_remedies(check));
    details
}

pub(super) fn detail_value(check: &DoctorCheck, label: &str) -> Option<String> {
    parsed_details(check)
        .into_iter()
        .find(|detail| detail.label == label)
        .map(|detail| detail.value)
}

pub(super) fn rollout_summary(value: &str) -> Option<String> {
    let (files, rest) = value.split_once(" files, ")?;
    let (total_bytes, rest) = rest.split_once(" total bytes, ")?;
    let (average_bytes, _) = rest.split_once(" average bytes")?;
    let files = files.trim().parse::<u64>().ok()?;
    let total_bytes = total_bytes.trim().parse::<u64>().ok()?;
    let average_bytes = average_bytes.trim().parse::<u64>().ok()?;
    Some(format!(
        "{} files · {} (avg {})",
        format_count(files),
        format_bytes(total_bytes),
        format_bytes(average_bytes)
    ))
}

pub(super) fn rollout_files_and_bytes(value: &str) -> Option<(u64, u64)> {
    let (files, rest) = value.split_once(" files, ")?;
    let (total_bytes, _) = rest.split_once(" total bytes, ")?;
    Some((
        files.trim().parse::<u64>().ok()?,
        total_bytes.trim().parse::<u64>().ok()?,
    ))
}

pub(super) fn format_bytes(bytes: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = KIB * 1024.0;
    const GIB: f64 = MIB * 1024.0;

    let bytes = bytes as f64;
    if bytes >= GIB {
        format!("{:.2} GB", bytes / GIB)
    } else if bytes >= MIB {
        format!("{:.2} MB", bytes / MIB)
    } else if bytes >= KIB {
        format!("{:.2} KB", bytes / KIB)
    } else {
        format!("{} B", bytes as u64)
    }
}

pub(super) fn format_count(count: u64) -> String {
    let mut digits = count.to_string();
    let mut out = String::new();
    while digits.len() > 3 {
        let tail = digits.split_off(digits.len() - 3);
        if out.is_empty() {
            out = tail;
        } else {
            out = format!("{tail},{out}");
        }
    }
    if out.is_empty() {
        digits
    } else {
        format!("{digits},{out}")
    }
}

fn parsed_details(check: &DoctorCheck) -> Vec<ParsedDetail> {
    check
        .details
        .iter()
        .map(|detail| redact_detail(detail))
        .map(|detail| {
            detail
                .split_once(": ")
                .map(|(label, value)| ParsedDetail {
                    label: label.to_string(),
                    value: value.to_string(),
                })
                .unwrap_or_else(|| ParsedDetail {
                    label: String::new(),
                    value: detail,
                })
        })
        .collect()
}

fn runtime_details(parsed: &[ParsedDetail]) -> Vec<HumanDetail> {
    let mut out = Vec::new();
    push_row_if_present(&mut out, parsed, "version", "version");
    push_row_if_present(&mut out, parsed, "install method", "install method");
    push_row_if_present(&mut out, parsed, "commit", "commit");
    push_row_if_present(&mut out, parsed, "current executable", "executable");
    push_remaining(
        &mut out,
        parsed,
        &[
            "version",
            "platform",
            "install method",
            "commit",
            "current executable",
        ],
        &[],
    );
    out
}

fn install_details(parsed: &[ParsedDetail], options: HumanOutputOptions) -> Vec<HumanDetail> {
    let mut out = Vec::new();
    push_row_if_present(&mut out, parsed, "install context", "context");
    if parsed.iter().any(|detail| {
        detail.value == "ignored inherited package-manager launch env for cargo-built binary"
    }) {
        out.push(HumanDetail::Bullet(
            "ignored inherited package-manager launch env for cargo-built binary".to_string(),
        ));
    }

    let managed_by_npm = value(parsed, "managed by npm").unwrap_or("false");
    let managed_by_bun = value(parsed, "managed by bun").unwrap_or("false");
    let package_root = value(parsed, "managed package root").unwrap_or("not set");
    out.push(HumanDetail::Row {
        label: "managed by".to_string(),
        value: format!(
            "npm: {} · bun: {} · package root {}",
            yes_no(managed_by_npm),
            yes_no(managed_by_bun),
            if is_falsy(package_root) {
                "—".to_string()
            } else {
                package_root.to_string()
            }
        ),
        expected: None,
    });

    let path_entries = numbered_values(parsed, "PATH codex #");
    if !path_entries.is_empty() {
        let total = path_entries.len();
        let shown = if options.show_all {
            total
        } else {
            total.min(3)
        };
        out.push(HumanDetail::Row {
            label: format!("PATH entries ({total})"),
            value: path_entries[0].clone(),
            expected: None,
        });
        out.extend(
            path_entries
                .iter()
                .skip(1)
                .take(shown.saturating_sub(1))
                .cloned()
                .map(HumanDetail::Continuation),
        );
        if shown < total {
            out.push(HumanDetail::Continuation(
                "… (full list with --all)".to_string(),
            ));
        }
    }

    push_remaining(
        &mut out,
        parsed,
        &[
            "current executable",
            "install context",
            "managed by npm",
            "managed by bun",
            "managed package root",
            "PATH codex entries",
        ],
        &["PATH codex #"],
    );
    out
}

fn config_details(parsed: &[ParsedDetail], options: HumanOutputOptions) -> Vec<HumanDetail> {
    let mut out = Vec::new();
    if let Some(model) = value(parsed, "model") {
        let value = value(parsed, "model provider").map_or_else(
            || model.to_string(),
            |provider| format!("{model} · {provider}"),
        );
        out.push(HumanDetail::Row {
            label: "model".to_string(),
            value,
            expected: None,
        });
    }
    push_row_if_present(&mut out, parsed, "cwd", "cwd");
    push_row_if_present(&mut out, parsed, "config.toml", "config.toml");
    push_row_if_present(&mut out, parsed, "config.toml parse", "config.toml parse");
    push_row_if_present(&mut out, parsed, "config.toml read", "config.toml read");
    push_row_if_present(&mut out, parsed, "mcp servers", "MCP servers");
    push_feature_flags(&mut out, parsed, options);

    for detail in parsed
        .iter()
        .filter(|detail| detail.label == "legacy feature flag")
    {
        out.push(HumanDetail::Row {
            label: "legacy alias".to_string(),
            value: detail.value.clone(),
            expected: None,
        });
    }

    push_remaining(
        &mut out,
        parsed,
        &[
            "CODEX_HOME",
            "cwd",
            "model",
            "model provider",
            "log dir",
            "sqlite home",
            "mcp servers",
            "feature flags enabled",
            "enabled feature flags",
            "feature flag overrides",
            "legacy feature flag",
            "config.toml",
            "config.toml parse",
            "config.toml read",
        ],
        &[],
    );
    out
}

fn state_details(parsed: &[ParsedDetail]) -> Vec<HumanDetail> {
    let mut out = Vec::new();
    push_row_if_present(&mut out, parsed, "CODEX_HOME", "CODEX_HOME");
    push_row_if_present(&mut out, parsed, "log dir", "log dir");
    push_row_if_present(&mut out, parsed, "sqlite home", "sqlite home");
    push_database_row(&mut out, parsed, "state DB");
    push_database_row(&mut out, parsed, "log DB");

    for (source, label) in [
        ("active rollout files", "active rollouts"),
        ("archived rollout files", "archived rollouts"),
    ] {
        if let Some(value) = value(parsed, source) {
            out.push(HumanDetail::Row {
                label: label.to_string(),
                value: rollout_summary(value).unwrap_or_else(|| value.to_string()),
                expected: None,
            });
        }
    }

    push_remaining(
        &mut out,
        parsed,
        &[
            "CODEX_HOME",
            "log dir",
            "sqlite home",
            "state DB",
            "log DB",
            "state DB integrity",
            "log DB integrity",
            "active rollout files",
            "archived rollout files",
        ],
        &[],
    );
    out
}

fn generic_details(parsed: &[ParsedDetail]) -> Vec<HumanDetail> {
    parsed
        .iter()
        .map(|detail| {
            if detail.label.is_empty() {
                HumanDetail::Bullet(detail.value.clone())
            } else {
                HumanDetail::Row {
                    label: display_label(&detail.label),
                    value: detail.value.clone(),
                    expected: None,
                }
            }
        })
        .collect()
}

fn push_feature_flags(
    out: &mut Vec<HumanDetail>,
    parsed: &[ParsedDetail],
    options: HumanOutputOptions,
) {
    let enabled_count = value(parsed, "feature flags enabled")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or_default();
    let overrides = list_items(value(parsed, "feature flag overrides").unwrap_or("none"));
    let override_count = overrides.len();
    let hint = if !options.show_all && enabled_count > 0 {
        " (full list with --all)"
    } else {
        ""
    };
    out.push(HumanDetail::Row {
        label: "feature flags".to_string(),
        value: format!("{enabled_count} enabled · {override_count} overridden{hint}"),
        expected: None,
    });

    if !overrides.is_empty() {
        push_list_row(out, "overrides", &override_names(&overrides), options);
    }
    if options.show_all {
        let enabled = list_items(value(parsed, "enabled feature flags").unwrap_or("none"));
        if !enabled.is_empty() {
            push_list_row(out, "enabled flags", &enabled, options);
        }
    }
}

fn push_list_row(
    out: &mut Vec<HumanDetail>,
    label: &str,
    items: &[String],
    options: HumanOutputOptions,
) {
    let limit = if options.show_all {
        items.len()
    } else {
        items.len().min(LIST_LIMIT)
    };
    let mut value = items
        .iter()
        .take(limit)
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    if limit < items.len() {
        value.push_str(", … (full list with --all)");
    }
    out.push(HumanDetail::Row {
        label: label.to_string(),
        value,
        expected: None,
    });
}

fn push_database_row(out: &mut Vec<HumanDetail>, parsed: &[ParsedDetail], label: &str) {
    let Some(path) = value(parsed, label) else {
        return;
    };
    let integrity = value(parsed, &format!("{label} integrity"));
    let value = integrity.map_or_else(
        || path.to_string(),
        |integrity| format!("{path} · integrity {integrity}"),
    );
    out.push(HumanDetail::Row {
        label: label.to_string(),
        value,
        expected: None,
    });
}

fn push_row_if_present(
    out: &mut Vec<HumanDetail>,
    parsed: &[ParsedDetail],
    source_label: &str,
    display_label: &str,
) {
    if let Some(value) = value(parsed, source_label) {
        out.push(HumanDetail::Row {
            label: display_label.to_string(),
            value: value.to_string(),
            expected: None,
        });
    }
}

fn push_remaining(
    out: &mut Vec<HumanDetail>,
    parsed: &[ParsedDetail],
    consumed_labels: &[&str],
    consumed_prefixes: &[&str],
) {
    for detail in parsed {
        if detail.value == "ignored inherited package-manager launch env for cargo-built binary" {
            continue;
        }
        if consumed_labels.contains(&detail.label.as_str())
            || consumed_prefixes
                .iter()
                .any(|prefix| detail.label.starts_with(prefix))
        {
            continue;
        }
        if detail.label.is_empty() {
            out.push(HumanDetail::Bullet(detail.value.clone()));
        } else {
            out.push(HumanDetail::Row {
                label: display_label(&detail.label),
                value: detail.value.clone(),
                expected: None,
            });
        }
    }
}

fn humanize_detail(detail: HumanDetail, options: HumanOutputOptions) -> HumanDetail {
    match detail {
        HumanDetail::Row {
            label,
            value,
            expected,
        } => HumanDetail::Row {
            label,
            value: humanize_value(&value, options),
            expected,
        },
        HumanDetail::Continuation(value) => {
            HumanDetail::Continuation(humanize_value(&value, options))
        }
        HumanDetail::Bullet(value) => HumanDetail::Bullet(humanize_value(&value, options)),
        HumanDetail::Remedy(value) => HumanDetail::Remedy(value),
    }
}

fn attach_issue_metadata(detail: HumanDetail, check: &DoctorCheck) -> HumanDetail {
    let HumanDetail::Row {
        label,
        value,
        expected,
    } = detail
    else {
        return detail;
    };
    let expected = expected.or_else(|| issue_expected_for_label(check, &label));
    HumanDetail::Row {
        label,
        value,
        expected,
    }
}

fn issue_expected_for_label(check: &DoctorCheck, label: &str) -> Option<String> {
    check
        .issues
        .iter()
        .find(|issue| {
            issue
                .fields
                .iter()
                .any(|field| display_label(field) == label || field == label)
        })
        .and_then(|issue| issue.expected.clone())
}

fn issue_remedies(check: &DoctorCheck) -> Vec<HumanDetail> {
    let mut seen = BTreeSet::new();
    check
        .issues
        .iter()
        .filter_map(|issue| issue.remedy.as_ref())
        .filter(|remedy| seen.insert((*remedy).clone()))
        .cloned()
        .map(HumanDetail::Remedy)
        .collect()
}

fn humanize_value(value: &str, _options: HumanOutputOptions) -> String {
    if looks_like_path(value) {
        return shorten_path_prefix(value);
    }
    if let Some(timestamp) = humanize_timestamp(value) {
        return timestamp;
    }
    value.to_string()
}

fn humanize_timestamp(value: &str) -> Option<String> {
    if value.len() < 17 || !value.ends_with('Z') {
        return None;
    }
    let (date, time) = value.split_once('T')?;
    let hour_minute = time.get(..5)?;
    Some(format!("{date} {hour_minute} UTC"))
}

fn shorten_path_prefix(value: &str) -> String {
    let (path, suffix) = value.split_once(" (").map_or_else(
        || (value, String::new()),
        |(path, suffix)| (path, format!(" ({suffix}")),
    );
    let home_shortened = home_shortened_path(path);
    let shortened = middle_truncate(&home_shortened, PATH_LIMIT);
    format!("{shortened}{suffix}")
}

fn home_shortened_path(path: &str) -> String {
    let Some(home) = env::var_os("HOME").and_then(|home| home.into_string().ok()) else {
        return path.to_string();
    };
    if path == home {
        "~".to_string()
    } else {
        path.strip_prefix(&format!("{home}/"))
            .map_or_else(|| path.to_string(), |tail| format!("~/{tail}"))
    }
}

fn middle_truncate(value: &str, max_chars: usize) -> String {
    let char_count = value.chars().count();
    if char_count <= max_chars {
        return value.to_string();
    }
    let head_len = max_chars / 2;
    let tail_len = max_chars.saturating_sub(head_len + 1);
    let head = value.chars().take(head_len).collect::<String>();
    let tail = value
        .chars()
        .rev()
        .take(tail_len)
        .collect::<String>()
        .chars()
        .rev()
        .collect::<String>();
    format!("{head}…{tail}")
}

fn looks_like_path(value: &str) -> bool {
    value.starts_with('/')
        || value.starts_with("~/")
        || value.starts_with("./")
        || value.starts_with("../")
}

fn numbered_values(parsed: &[ParsedDetail], prefix: &str) -> Vec<String> {
    parsed
        .iter()
        .filter(|detail| detail.label.starts_with(prefix))
        .map(|detail| detail.value.clone())
        .collect()
}

fn value<'a>(parsed: &'a [ParsedDetail], label: &str) -> Option<&'a str> {
    parsed
        .iter()
        .find(|detail| detail.label == label)
        .map(|detail| detail.value.as_str())
}

fn display_label(label: &str) -> String {
    match label {
        "codex-linux-sandbox helper" => "linux helper",
        "optional reachability failed" => "optional reachability",
        "check for update on startup" => "startup update check",
        other => other,
    }
    .to_string()
}

fn list_items(value: &str) -> Vec<String> {
    if is_falsy(value) {
        return Vec::new();
    }
    value
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_string)
        .collect()
}

fn override_names(items: &[String]) -> Vec<String> {
    items
        .iter()
        .map(|item| item.split_once('=').map_or(item.as_str(), |(name, _)| name))
        .map(str::to_string)
        .collect()
}

fn yes_no(value: &str) -> &'static str {
    if value == "true" { "yes" } else { "no" }
}

pub(super) fn is_falsy(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "" | "false" | "none" | "not set" | "unknown" | "missing" | "absent" | "no" | "—" | "-"
    )
}
