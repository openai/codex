//! Renders doctor reports for terminal users.
//!
//! The renderer is intentionally separate from check construction so the JSON
//! report can stay stable while the human view optimizes for scanability. It
//! groups checks by concern, colors only status/actionable tokens, and redacts
//! sensitive detail lines before showing them in detailed output.

mod detail;

use std::fmt::Write as _;

use detail::HumanDetail;
use detail::detail_lines;
use owo_colors::OwoColorize;
use owo_colors::XtermColors;

use super::CheckStatus;
use super::DoctorCheck;
use super::DoctorReport;

const NAME_WIDTH: usize = 12;
const DETAIL_LABEL_WIDTH: usize = 24;
const SEPARATOR_WIDTH: usize = 61;

const GROUPS: &[OutputGroup] = &[
    OutputGroup {
        title: "Environment",
        keys: &["runtime", "install", "search", "terminal", "state"],
    },
    OutputGroup {
        title: "Configuration",
        keys: &["config", "auth", "mcp", "sandbox"],
    },
    OutputGroup {
        title: "Updates",
        keys: &["updates"],
    },
    OutputGroup {
        title: "Connectivity",
        keys: &["network", "websocket", "reachability"],
    },
    OutputGroup {
        title: "Background Server",
        keys: &["app-server"],
    },
];

struct OutputGroup {
    title: &'static str,
    keys: &'static [&'static str],
}

/// Rendering controls for human doctor output.
///
/// These options affect presentation only. They must not change which checks
/// run or which fields are present in the underlying JSON report.
#[derive(Clone, Copy, Debug)]
pub(super) struct HumanOutputOptions {
    pub(super) show_details: bool,
    pub(super) show_all: bool,
    pub(super) ascii: bool,
    pub(super) color_enabled: bool,
}

/// Formats a doctor report into the grouped terminal layout.
///
/// The renderer expects checks to carry stable categories, but it owns their
/// display order. Adding a new category without adding it to GROUPS keeps JSON
/// output intact but hides that row from the human view.
pub(super) fn render_human_report(report: &DoctorReport, options: HumanOutputOptions) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "{} {}",
        bold("Codex Doctor", options),
        dim(&header_suffix(report), options)
    );
    out.push('\n');

    let notes = notes_for_report(report);
    if !notes.is_empty() {
        let _ = writeln!(out, "{}", bold("Notes", options));
        for note in &notes {
            write_note_row(&mut out, note, options);
        }
        let _ = writeln!(out, "{}", dim(&separator(options), options));
        out.push('\n');
    }

    let mut wrote_group = false;
    for group in GROUPS {
        let group_checks = checks_for_group(report, group);
        if group_checks.is_empty() {
            continue;
        }

        if wrote_group {
            out.push('\n');
        }
        wrote_group = true;

        let _ = writeln!(out, "{}", bold(group.title, options));
        for check in group_checks {
            write_check_row(&mut out, check, options);
        }
    }

    out.push('\n');
    let _ = writeln!(out, "{}", dim(&separator(options), options));
    let _ = writeln!(out, "{}", summary_line(report, options));
    out.push('\n');
    write_footer(&mut out, options);
    out
}

fn checks_for_group<'a>(report: &'a DoctorReport, group: &OutputGroup) -> Vec<&'a DoctorCheck> {
    group
        .keys
        .iter()
        .flat_map(|key| {
            report
                .checks
                .iter()
                .filter(move |check| check.category == *key)
        })
        .collect()
}

fn write_check_row(out: &mut String, check: &DoctorCheck, options: HumanOutputOptions) {
    let description = row_description(check, options);
    let status = display_status(check);
    let _ = writeln!(
        out,
        "  {}{} {}",
        status_marker_slot(status, options),
        format_args!("{:<NAME_WIDTH$}", check.category),
        style_description(&description, status, options)
    );

    if options.show_details {
        for detail in detail_lines(check, options) {
            write_detail_line(out, detail, options);
        }
    }
}

fn write_note_row(out: &mut String, note: &DoctorNote, options: HumanOutputOptions) {
    let _ = writeln!(
        out,
        "   {}{} {}",
        status_marker_slot(note.status, options),
        format_args!("{:<NAME_WIDTH$}", note.name),
        style_note_summary(note, options)
    );
}

fn write_detail_line(out: &mut String, detail: HumanDetail, options: HumanOutputOptions) {
    match detail {
        HumanDetail::Row {
            label,
            value,
            expected,
        } => {
            let is_issue = expected.is_some();
            let label = format!("{label:<DETAIL_LABEL_WIDTH$}");
            let value = if let Some(expected) = expected {
                format!(
                    "{} {}",
                    detail_value(&value, options),
                    dim(&format!("(expected {expected})"), options)
                )
            } else {
                detail_value(&value, options)
            };
            let _ = writeln!(
                out,
                "    {} {} {}",
                detail_marker(is_issue, options),
                detail_label(&label, options),
                value
            );
        }
        HumanDetail::Continuation(value) => {
            let spacer = " ".repeat(DETAIL_LABEL_WIDTH);
            let _ = writeln!(
                out,
                "      {} {}",
                detail_label(&spacer, options),
                detail_value(&value, options)
            );
        }
        HumanDetail::Bullet(value) => {
            let _ = writeln!(
                out,
                "    {} {}",
                very_dim(if options.ascii { "-" } else { "·" }, options),
                dim(&highlight_actions(&value, options), options)
            );
        }
        HumanDetail::Remedy(value) => {
            let marker = if options.ascii { "->" } else { "→" };
            let _ = writeln!(
                out,
                "    {} {}",
                orange(marker, options),
                highlight_actions(&value, options)
            );
        }
    }
}

fn row_description(check: &DoctorCheck, options: HumanOutputOptions) -> String {
    if matches!(check.status, CheckStatus::Warning | CheckStatus::Fail) && !check.issues.is_empty()
    {
        return issue_summary(check);
    }
    if matches!(check.status, CheckStatus::Warning | CheckStatus::Fail)
        && let Some(remediation) = &check.remediation
    {
        let dash = if options.ascii { " - " } else { " — " };
        let summary = &check.summary;
        return format!("{summary}{dash}{remediation}");
    }

    display_summary(check, options)
}

fn issue_summary(check: &DoctorCheck) -> String {
    match check.issues.as_slice() {
        [] => check.summary.clone(),
        [issue] => issue.cause.clone(),
        issues => format!(
            "{} issues - {}",
            issues.len(),
            issues
                .iter()
                .take(2)
                .map(|issue| issue.cause.as_str())
                .collect::<Vec<_>>()
                .join("; ")
        ),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DisplayStatus {
    Ok,
    Update,
    Note,
    Warning,
    Fail,
    Idle,
}

struct DoctorNote {
    status: DisplayStatus,
    name: String,
    summary: String,
}

fn display_status(check: &DoctorCheck) -> DisplayStatus {
    if check.category == "app-server"
        && check.status == CheckStatus::Ok
        && check
            .details
            .iter()
            .any(|detail| detail == "status: not running")
    {
        return DisplayStatus::Idle;
    }

    match check.status {
        CheckStatus::Ok => DisplayStatus::Ok,
        CheckStatus::Warning => DisplayStatus::Warning,
        CheckStatus::Fail => DisplayStatus::Fail,
    }
}

fn status_marker(status: DisplayStatus, options: HumanOutputOptions) -> String {
    let marker = if options.ascii {
        match status {
            DisplayStatus::Ok => "[ok]",
            DisplayStatus::Update => "[up]",
            DisplayStatus::Note | DisplayStatus::Warning => "[!!]",
            DisplayStatus::Fail => "[XX]",
            DisplayStatus::Idle => "[--]",
        }
    } else {
        match status {
            DisplayStatus::Ok => "✓",
            DisplayStatus::Update => "↑",
            DisplayStatus::Note | DisplayStatus::Warning => "⚠",
            DisplayStatus::Fail => "✗",
            DisplayStatus::Idle => "○",
        }
    };

    match status {
        DisplayStatus::Ok => green(marker, options),
        DisplayStatus::Update => amber(marker, options),
        DisplayStatus::Note | DisplayStatus::Warning => orange(marker, options),
        DisplayStatus::Fail => red(marker, options),
        DisplayStatus::Idle => dim(marker, options),
    }
}

fn status_marker_slot(status: DisplayStatus, options: HumanOutputOptions) -> String {
    let marker = status_marker(status, options);
    format!("{marker} ")
}

fn style_description(
    description: &str,
    status: DisplayStatus,
    options: HumanOutputOptions,
) -> String {
    let highlighted = highlight_actions(description, options);
    match status {
        DisplayStatus::Ok | DisplayStatus::Idle => dim(&highlighted, options),
        DisplayStatus::Update => amber(&highlighted, options),
        DisplayStatus::Note | DisplayStatus::Warning | DisplayStatus::Fail => highlighted,
    }
}

fn detail_marker(is_issue: bool, options: HumanOutputOptions) -> String {
    if !is_issue {
        return " ".to_string();
    }
    orange(if options.ascii { ">" } else { "▸" }, options)
}

fn style_note_summary(note: &DoctorNote, options: HumanOutputOptions) -> String {
    if note.status == DisplayStatus::Update {
        return style_update_note_summary(&note.summary, options);
    }
    style_description(&note.summary, note.status, options)
}

fn style_update_note_summary(summary: &str, options: HumanOutputOptions) -> String {
    if !options.color_enabled {
        return summary.to_string();
    }

    let Some((version, rest)) = summary.split_once(" available") else {
        return amber(summary, options);
    };
    let Some((action, parenthetical)) = rest.split_once(" (") else {
        return format!(
            "{}{}",
            amber(&format!("{version} available"), options),
            amber(rest, options)
        );
    };
    format!(
        "{}{} {}",
        amber(&format!("{version} available"), options),
        amber(action, options),
        dim(&format!("({parenthetical}"), options)
    )
}

fn summary_line(report: &DoctorReport, options: HumanOutputOptions) -> String {
    let notes = notes_for_report(report);
    let counts = StatusCounts::from_report(report, notes.len());
    let separator = dim(if options.ascii { " | " } else { " · " }, options);
    let status = overall_status_label(report.overall_status);
    let mut parts = vec![count_label(counts.ok, "ok", DisplayStatus::Ok, options)];
    if counts.idle > 0 {
        parts.push(count_label(
            counts.idle,
            "idle",
            DisplayStatus::Idle,
            options,
        ));
    }
    if counts.notes > 0 {
        parts.push(count_label(
            counts.notes,
            "notes",
            DisplayStatus::Note,
            options,
        ));
    }
    parts.push(count_label(
        counts.warning,
        "warn",
        DisplayStatus::Warning,
        options,
    ));
    parts.push(count_label(
        counts.fail,
        "fail",
        DisplayStatus::Fail,
        options,
    ));
    format!(
        "{} {}",
        parts.join(&separator),
        styled_overall_status(status, report.overall_status, options)
    )
}

fn count_label(
    count: usize,
    label: &str,
    status: DisplayStatus,
    options: HumanOutputOptions,
) -> String {
    let count = dim(&count.to_string(), options);
    let label = match status {
        DisplayStatus::Ok => green(label, options),
        DisplayStatus::Update => amber(label, options),
        DisplayStatus::Note | DisplayStatus::Warning => orange(label, options),
        DisplayStatus::Fail => red(label, options),
        DisplayStatus::Idle => dim(label, options),
    };
    format!("{count} {label}")
}

fn overall_status_label(status: CheckStatus) -> &'static str {
    match status {
        CheckStatus::Ok => "ok",
        CheckStatus::Warning => "degraded",
        CheckStatus::Fail => "failed",
    }
}

fn styled_overall_status(label: &str, status: CheckStatus, options: HumanOutputOptions) -> String {
    if !options.color_enabled {
        return label.to_string();
    }

    match status {
        CheckStatus::Ok => label.green().bold().to_string(),
        CheckStatus::Warning => label.yellow().bold().to_string(),
        CheckStatus::Fail => label.red().bold().to_string(),
    }
}

fn write_footer(out: &mut String, options: HumanOutputOptions) {
    if options.show_details {
        let _ = writeln!(
            out,
            "{} {:<24} {} {}",
            cyan("--summary", options),
            dim("compact output", options),
            cyan("--all", options),
            dim("expand truncated lists", options)
        );
    } else {
        let _ = writeln!(
            out,
            "{}",
            dim(
                "Run codex doctor without --summary for detailed diagnostics.",
                options
            )
        );
        let _ = writeln!(
            out,
            "{} {:<28} {} {}",
            cyan("--all", options),
            dim("expand truncated lists", options),
            cyan("--json", options),
            dim("redacted report", options)
        );
        return;
    }
    let _ = writeln!(
        out,
        "{} {}",
        cyan("--json", options),
        dim("redacted report", options)
    );
}

fn header_suffix(report: &DoctorReport) -> String {
    let version = format!("v{}", report.codex_version);
    report
        .checks
        .iter()
        .find(|check| check.category == "runtime")
        .and_then(|check| detail::detail_value(check, "platform"))
        .map_or(version.clone(), |platform| {
            format!("{version} · {platform}")
        })
}

fn notes_for_report(report: &DoctorReport) -> Vec<DoctorNote> {
    let mut notes = Vec::new();
    if let Some(check) = find_check(report, "updates") {
        update_note(check, report)
            .into_iter()
            .for_each(|note| notes.push(note));
    }
    if let Some(check) = find_check(report, "state") {
        rollout_note(check)
            .into_iter()
            .for_each(|note| notes.push(note));
    }
    if let Some(check) = find_check(report, "sandbox") {
        sandbox_note(check)
            .into_iter()
            .for_each(|note| notes.push(note));
    }
    non_ok_notes(report)
        .into_iter()
        .for_each(|note| notes.push(note));
    auth_reachability_note(report)
        .into_iter()
        .for_each(|note| notes.push(note));
    notes
}

fn find_check<'a>(report: &'a DoctorReport, category: &str) -> Option<&'a DoctorCheck> {
    report
        .checks
        .iter()
        .find(|check| check.category == category)
}

fn update_note(check: &DoctorCheck, report: &DoctorReport) -> Option<DoctorNote> {
    let status = detail::detail_value(check, "latest version status")?;
    if !status.contains("newer version is available") {
        return None;
    }
    let latest = detail::detail_value(check, "latest version")
        .or_else(|| detail::detail_value(check, "cached latest version"))
        .unwrap_or_else(|| "newer version".to_string());
    let dismissed = detail::detail_value(check, "dismissed version");
    let mut parenthetical = format!("current {}", report.codex_version);
    if let Some(dismissed) = dismissed
        && !detail::is_falsy(&dismissed)
    {
        parenthetical.push_str(&format!(", dismissed {dismissed}"));
    }
    Some(DoctorNote {
        status: DisplayStatus::Update,
        name: "updates".to_string(),
        summary: format!("{latest} available ({parenthetical})"),
    })
}

fn rollout_note(check: &DoctorCheck) -> Option<DoctorNote> {
    let active = detail::detail_value(check, "active rollout files")?;
    let (files, bytes) = detail::rollout_files_and_bytes(&active)?;
    if files < 1000 && bytes < 1024 * 1024 * 1024 {
        return None;
    }
    Some(DoctorNote {
        status: DisplayStatus::Warning,
        name: "rollouts".to_string(),
        summary: format!(
            "{} active files · {} on disk",
            detail::format_count(files),
            detail::format_bytes(bytes)
        ),
    })
}

fn sandbox_note(check: &DoctorCheck) -> Option<DoctorNote> {
    let filesystem = detail::detail_value(check, "filesystem sandbox")?;
    let network = detail::detail_value(check, "network sandbox")?;
    if filesystem == "restricted" && network == "restricted" {
        return None;
    }
    Some(DoctorNote {
        status: DisplayStatus::Warning,
        name: "sandbox".to_string(),
        summary: format!("filesystem {filesystem} · network {network}"),
    })
}

fn non_ok_notes(report: &DoctorReport) -> Vec<DoctorNote> {
    report
        .checks
        .iter()
        .filter(|check| matches!(check.status, CheckStatus::Warning | CheckStatus::Fail))
        .map(|check| DoctorNote {
            status: display_status(check),
            name: check.category.clone(),
            summary: actionable_note_summary(check),
        })
        .collect()
}

fn actionable_note_summary(check: &DoctorCheck) -> String {
    if !check.issues.is_empty() {
        return issue_summary(check);
    }
    if let Some(remediation) = &check.remediation {
        return format!("{} - {remediation}", check.summary);
    }
    check.summary.clone()
}

fn auth_reachability_note(report: &DoctorReport) -> Option<DoctorNote> {
    let websocket = find_check(report, "websocket")?;
    let reachability = find_check(report, "reachability")?;
    let auth_mode = detail::detail_value(websocket, "auth mode")?;
    let reachability_mode = detail::detail_value(reachability, "reachability mode")?;
    let auth_mode_lower = auth_mode.to_ascii_lowercase();
    let reachability_mode_lower = reachability_mode.to_ascii_lowercase();
    if auth_mode_lower.contains("chatgpt") && reachability_mode_lower.contains("api key") {
        return Some(DoctorNote {
            status: DisplayStatus::Warning,
            name: "auth".to_string(),
            summary: "mixed auth signals: ChatGPT login plus API key env var; HTTP reachability uses API-key mode".to_string(),
        });
    }
    None
}

fn display_summary(check: &DoctorCheck, _options: HumanOutputOptions) -> String {
    match check.category.as_str() {
        "runtime" => runtime_summary(check),
        "install" if check.status == CheckStatus::Ok => "consistent".to_string(),
        "search" => search_summary(check),
        "terminal" => terminal_summary(check),
        "state" => state_summary(check),
        "config" if check.status == CheckStatus::Ok => "loaded".to_string(),
        "mcp" => mcp_summary(check),
        "sandbox" => sandbox_summary(check),
        "network" => network_summary(check),
        "websocket" => websocket_summary(check),
        "app-server" => app_server_summary(check),
        _ => check.summary.clone(),
    }
}

fn runtime_summary(check: &DoctorCheck) -> String {
    if detail::detail_value(check, "current executable")
        .is_some_and(|path| path.contains("/target/debug/"))
    {
        return "local debug build".to_string();
    }
    detail::detail_value(check, "install method").unwrap_or_else(|| check.summary.clone())
}

fn search_summary(check: &DoctorCheck) -> String {
    let provider = detail::detail_value(check, "search provider");
    let command = detail::detail_value(check, "search command");
    let readiness = detail::detail_value(check, "search command readiness");
    match (readiness, provider, command) {
        (Some(readiness), Some(provider), Some(command)) if check.status == CheckStatus::Ok => {
            format!("{readiness} ({provider}, `{command}`)")
        }
        _ => check.summary.clone(),
    }
}

fn terminal_summary(check: &DoctorCheck) -> String {
    let mut parts = Vec::new();
    if let Some(terminal) = detail::detail_value(check, "terminal") {
        let version = detail::detail_value(check, "terminal version");
        parts.push(version.map_or(terminal.clone(), |version| format!("{terminal} {version}")));
    }
    if let Some(multiplexer) = detail::detail_value(check, "multiplexer") {
        parts.push(multiplexer);
    }
    if let Some(term) = detail::detail_value(check, "TERM") {
        parts.push(format!("TERM={term}"));
    }
    if parts.is_empty() {
        check.summary.clone()
    } else {
        parts.join(" · ")
    }
}

fn state_summary(check: &DoctorCheck) -> String {
    let state_ok =
        detail::detail_value(check, "state DB integrity").is_some_and(|value| value == "ok");
    let log_ok = detail::detail_value(check, "log DB integrity").is_some_and(|value| value == "ok");
    if state_ok && log_ok {
        "databases healthy".to_string()
    } else {
        check.summary.clone()
    }
}

fn mcp_summary(check: &DoctorCheck) -> String {
    let Some(count) = detail::detail_value(check, "configured servers") else {
        return check.summary.clone();
    };
    let disabled =
        detail::detail_value(check, "disabled servers").unwrap_or_else(|| "0".to_string());
    let transports = check
        .details
        .iter()
        .filter_map(|detail| detail.split_once(" servers: "))
        .filter(|(transport, _)| *transport != "configured" && *transport != "disabled")
        .map(|(transport, count)| format!("{count} {transport}"))
        .collect::<Vec<_>>();
    if transports.is_empty() {
        format!("{count} servers · {disabled} disabled")
    } else {
        format!(
            "{} server ({}) · {} disabled",
            count,
            transports.join(", "),
            disabled
        )
    }
}

fn sandbox_summary(check: &DoctorCheck) -> String {
    let approval = detail::detail_value(check, "approval policy");
    let filesystem = detail::detail_value(check, "filesystem sandbox");
    let network = detail::detail_value(check, "network sandbox");
    match (approval, filesystem, network) {
        (Some(approval), Some(filesystem), Some(network)) => {
            format!("{filesystem} fs + {network} network · approval {approval}")
        }
        _ => check.summary.clone(),
    }
}

fn network_summary(check: &DoctorCheck) -> String {
    detail::detail_value(check, "proxy env vars")
        .map(|value| {
            if value == "none" {
                "no proxy env vars".to_string()
            } else {
                "proxy env vars present".to_string()
            }
        })
        .unwrap_or_else(|| check.summary.clone())
}

fn websocket_summary(check: &DoctorCheck) -> String {
    let status = detail::detail_value(check, "handshake result")
        .or_else(|| detail::detail_value(check, "handshake status"));
    let timeout = detail::detail_value(check, "connect timeout")
        .map(|value| value.replace("000 ms", "s").replace(" ms", "ms"));
    match (status, timeout) {
        (Some(status), Some(timeout)) => format!("connected ({status}) · {timeout} timeout"),
        _ => check.summary.clone(),
    }
}

fn app_server_summary(check: &DoctorCheck) -> String {
    let status = detail::detail_value(check, "status");
    let mode = detail::detail_value(check, "mode");
    match (status, mode) {
        (Some(status), Some(mode)) => format!("{status} ({mode} mode)"),
        _ => check.summary.clone(),
    }
}

fn separator(options: HumanOutputOptions) -> String {
    if options.ascii {
        "-".repeat(SEPARATOR_WIDTH)
    } else {
        "─".repeat(SEPARATOR_WIDTH)
    }
}

fn highlight_actions(text: &str, options: HumanOutputOptions) -> String {
    if !options.color_enabled {
        return text.to_string();
    }

    let mut out = String::new();
    let mut parts = text.split('`');
    if let Some(first) = parts.next() {
        out.push_str(&highlight_flags(first, options));
    }
    let mut in_code = true;
    for part in parts {
        if in_code {
            out.push_str(&cyan(part, options));
        } else {
            out.push_str(&highlight_flags(part, options));
        }
        in_code = !in_code;
    }
    out
}

fn highlight_flags(text: &str, options: HumanOutputOptions) -> String {
    text.split_inclusive(char::is_whitespace)
        .map(|token| {
            let trimmed = token.trim_end();
            let suffix = &token[trimmed.len()..];
            let bare = trimmed.trim_end_matches([',', '.', ':', ';', ')']);
            let punctuation = &trimmed[bare.len()..];
            if bare.starts_with("--") {
                let highlighted = cyan(bare, options);
                format!("{highlighted}{punctuation}{suffix}")
            } else {
                token.to_string()
            }
        })
        .collect()
}

pub(super) fn redact_detail(detail: &str) -> String {
    let lower = detail.to_ascii_lowercase();
    let label = lower.split(':').next().unwrap_or_default();
    if label.contains("env var") {
        return redact_urls(detail);
    }
    if detail
        .split_once(": ")
        .is_some_and(|(_, value)| is_safe_presence_value(value))
    {
        return redact_urls(detail);
    }

    let secret_keys = [
        "openai_api_key",
        "codex_api_key",
        "codex_access_token",
        "authorization",
        "bearer_token",
        "token",
        "secret",
    ];
    if secret_keys.iter().any(|key| lower.contains(key)) {
        let name = detail.split(':').next().unwrap_or(detail);
        format!("{name}: <redacted>")
    } else {
        redact_urls(detail)
    }
}

fn is_safe_presence_value(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "true" | "false" | "yes" | "no" | "present" | "absent" | "missing" | "not set"
    )
}

fn redact_urls(detail: &str) -> String {
    detail
        .split_inclusive(char::is_whitespace)
        .map(redact_url_token)
        .collect()
}

fn redact_url_token(token: &str) -> String {
    let Some(scheme_end) = token.find("://") else {
        return token.to_string();
    };
    let mut suffix_start = token.len();
    while suffix_start > scheme_end + 3
        && matches!(
            token.as_bytes()[suffix_start - 1],
            b' ' | b'\t' | b'\n' | b'\r' | b'.' | b',' | b';' | b':' | b')' | b']'
        )
    {
        suffix_start -= 1;
    }

    let (body, suffix) = token.split_at(suffix_start);
    let scheme_prefix_end = scheme_end + 3;
    let rest = &body[scheme_prefix_end..];
    let authority_end = rest
        .find(['/', '?', '#'])
        .map(|index| scheme_prefix_end + index)
        .unwrap_or(body.len());
    let authority = &body[scheme_prefix_end..authority_end];
    let authority = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host)| host);
    let path = &body[authority_end..];
    let path = path
        .find(['?', '#'])
        .map(|index| &path[..index])
        .unwrap_or(path);
    let path = redact_url_path(path);
    format!(
        "{}{}{}{}",
        &body[..scheme_prefix_end],
        authority,
        path,
        suffix
    )
}

fn redact_url_path(path: &str) -> String {
    let mut segments = path.split('/').filter(|segment| !segment.is_empty());
    let Some(first_segment) = segments.next() else {
        return path.to_string();
    };
    if segments.next().is_some() {
        format!("/{first_segment}/<redacted>")
    } else {
        path.to_string()
    }
}

#[derive(Default)]
struct StatusCounts {
    ok: usize,
    idle: usize,
    notes: usize,
    warning: usize,
    fail: usize,
}

impl StatusCounts {
    fn from_report(report: &DoctorReport, notes: usize) -> Self {
        let mut counts = Self {
            notes,
            ..Self::default()
        };
        for check in &report.checks {
            match display_status(check) {
                DisplayStatus::Ok => counts.ok += 1,
                DisplayStatus::Idle => counts.idle += 1,
                DisplayStatus::Warning => counts.warning += 1,
                DisplayStatus::Fail => counts.fail += 1,
                DisplayStatus::Update | DisplayStatus::Note => {}
            }
        }
        counts
    }
}

fn bold(text: &str, options: HumanOutputOptions) -> String {
    if options.color_enabled {
        text.bold().to_string()
    } else {
        text.to_string()
    }
}

fn dim(text: &str, options: HumanOutputOptions) -> String {
    if options.color_enabled {
        text.dimmed().to_string()
    } else {
        text.to_string()
    }
}

fn very_dim(text: &str, options: HumanOutputOptions) -> String {
    color256(text, /*code*/ 238, options)
}

fn detail_label(text: &str, options: HumanOutputOptions) -> String {
    color256(text, /*code*/ 240, options)
}

fn detail_value(text: &str, options: HumanOutputOptions) -> String {
    if !options.color_enabled {
        return text.to_string();
    }
    style_detail_text(text, options)
}

fn style_detail_text(text: &str, options: HumanOutputOptions) -> String {
    let mut out = String::new();
    let mut parts = text.split('`');
    if let Some(first) = parts.next() {
        out.push_str(&style_detail_plain_text(first, options));
    }
    let mut in_code = true;
    for part in parts {
        if in_code {
            out.push_str(&cyan(part, options));
        } else {
            out.push_str(&style_detail_plain_text(part, options));
        }
        in_code = !in_code;
    }
    out
}

fn style_detail_plain_text(text: &str, options: HumanOutputOptions) -> String {
    text.split_inclusive(char::is_whitespace)
        .map(|token| style_detail_token(token, options))
        .collect()
}

fn style_detail_token(token: &str, options: HumanOutputOptions) -> String {
    let trimmed = token.trim_end();
    let suffix = &token[trimmed.len()..];
    let bare = trimmed.trim_end_matches([',', '.', ':', ';', ')']);
    let punctuation = &trimmed[bare.len()..];
    let styled = style_detail_bare_token(bare, options);
    format!("{styled}{punctuation}{suffix}")
}

fn style_detail_bare_token(bare: &str, options: HumanOutputOptions) -> String {
    if bare.is_empty() {
        return String::new();
    }
    if bare == "<redacted>" {
        return color256(&bare.italic().to_string(), /*code*/ 244, options);
    }
    if bare.contains("(missing)") || detail::is_falsy(bare) {
        return color256(bare, /*code*/ 240, options);
    }
    if let Some((label, value)) = bare.split_once(':')
        && detail::is_falsy(value)
    {
        return format!("{label}:{}", color256(value, /*code*/ 240, options));
    }
    if bare == "ok" {
        return green(bare, options);
    }
    if bare.starts_with("--") || looks_copyable(bare) {
        return cyan(bare, options);
    }
    if matches!(bare, "B" | "KB" | "MB" | "GB" | "TB" | "files" | "file") {
        return dim(bare, options);
    }
    bare.to_string()
}

fn green(text: &str, options: HumanOutputOptions) -> String {
    color256(text, /*code*/ 10, options)
}

fn amber(text: &str, options: HumanOutputOptions) -> String {
    color256(text, /*code*/ 220, options)
}

fn orange(text: &str, options: HumanOutputOptions) -> String {
    color256(text, /*code*/ 214, options)
}

fn red(text: &str, options: HumanOutputOptions) -> String {
    color256(text, /*code*/ 196, options)
}

fn cyan(text: &str, options: HumanOutputOptions) -> String {
    color256(text, /*code*/ 117, options)
}

fn color256(text: &str, code: u8, options: HumanOutputOptions) -> String {
    if options.color_enabled {
        text.color(XtermColors::from(code)).to_string()
    } else {
        text.to_string()
    }
}

fn looks_copyable(text: &str) -> bool {
    text.starts_with("http://")
        || text.starts_with("https://")
        || text.starts_with("wss://")
        || text.starts_with("~/")
        || text.starts_with('/')
        || text.starts_with("./")
        || text.starts_with("../")
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    fn detailed_no_color_unicode_options() -> HumanOutputOptions {
        HumanOutputOptions {
            show_details: true,
            show_all: false,
            ascii: false,
            color_enabled: false,
        }
    }

    fn summary_no_color_unicode_options() -> HumanOutputOptions {
        HumanOutputOptions {
            show_details: false,
            show_all: false,
            ascii: false,
            color_enabled: false,
        }
    }

    fn detailed_all_no_color_unicode_options() -> HumanOutputOptions {
        HumanOutputOptions {
            show_details: true,
            show_all: true,
            ascii: false,
            color_enabled: false,
        }
    }

    fn detailed_color_unicode_options() -> HumanOutputOptions {
        HumanOutputOptions {
            show_details: true,
            show_all: false,
            ascii: false,
            color_enabled: true,
        }
    }

    fn sample_report() -> DoctorReport {
        let checks = vec![
            DoctorCheck::new(
                "runtime.provenance",
                "runtime",
                CheckStatus::Ok,
                "running local build on darwin-arm64",
            ),
            DoctorCheck::new(
                "installation",
                "install",
                CheckStatus::Ok,
                "installation looks consistent",
            ),
            DoctorCheck::new(
                "runtime.search",
                "search",
                CheckStatus::Ok,
                "search is OK (bundled)",
            ),
            DoctorCheck::new(
                "terminal.env",
                "terminal",
                CheckStatus::Warning,
                "narrow terminal",
            ),
            DoctorCheck::new(
                "state.paths",
                "state",
                CheckStatus::Ok,
                "state paths inspectable",
            ),
            DoctorCheck::new(
                "auth.credentials",
                "auth",
                CheckStatus::Fail,
                "token expired",
            )
            .detail("OPENAI_API_KEY: present")
            .remediation("Run `codex login`."),
            DoctorCheck::new(
                "updates.status",
                "updates",
                CheckStatus::Ok,
                "update configuration is locally consistent",
            ),
            DoctorCheck::new(
                "network.env",
                "network",
                CheckStatus::Ok,
                "network environment readable",
            ),
            DoctorCheck::new(
                "network.websocket_reachability",
                "websocket",
                CheckStatus::Ok,
                "Responses WebSocket handshake succeeded",
            ),
            DoctorCheck::new(
                "app_server.status",
                "app-server",
                CheckStatus::Ok,
                "background server is not running",
            ),
            DoctorCheck::new(
                "network.provider_reachability",
                "reachability",
                CheckStatus::Ok,
                "active provider endpoints are reachable over HTTP",
            ),
        ];
        DoctorReport {
            schema_version: 1,
            generated_at: "0s since unix epoch".to_string(),
            overall_status: CheckStatus::Fail,
            codex_version: "0.0.0".to_string(),
            checks,
        }
    }

    #[test]
    fn render_human_report_includes_details_by_default_without_color() {
        let rendered = render_human_report(&sample_report(), detailed_no_color_unicode_options());
        let expected = format!(
            "\
Codex Doctor v0.0.0

Notes
   ⚠ terminal     narrow terminal
   ✗ auth         token expired - Run `codex login`.
─────────────────────────────────────────────────────────────

Environment
  ✓ runtime      running local build on darwin-arm64
  ✓ install      consistent
      managed by               npm: no · bun: no · package root —
  ✓ search       search is OK (bundled)
  ⚠ terminal     narrow terminal
  ✓ state        state paths inspectable

Configuration
  ✗ auth         token expired — Run `codex login`.
      OPENAI_API_KEY           present

Updates
  ✓ updates      update configuration is locally consistent

Connectivity
  ✓ network      network environment readable
  ✓ websocket    Responses WebSocket handshake succeeded
  ✓ reachability active provider endpoints are reachable over HTTP

Background Server
  ✓ app-server   background server is not running

{}
9 ok · 2 notes · 1 warn · 1 fail failed

--summary compact output           --all expand truncated lists
--json redacted report
",
            "─".repeat(SEPARATOR_WIDTH)
        );
        assert_eq!(rendered, expected);
    }

    #[test]
    fn render_human_report_supports_summary_output_without_color() {
        let rendered = render_human_report(&sample_report(), summary_no_color_unicode_options());
        let expected = format!(
            "\
Codex Doctor v0.0.0

Notes
   ⚠ terminal     narrow terminal
   ✗ auth         token expired - Run `codex login`.
─────────────────────────────────────────────────────────────

Environment
  ✓ runtime      running local build on darwin-arm64
  ✓ install      consistent
  ✓ search       search is OK (bundled)
  ⚠ terminal     narrow terminal
  ✓ state        state paths inspectable

Configuration
  ✗ auth         token expired — Run `codex login`.

Updates
  ✓ updates      update configuration is locally consistent

Connectivity
  ✓ network      network environment readable
  ✓ websocket    Responses WebSocket handshake succeeded
  ✓ reachability active provider endpoints are reachable over HTTP

Background Server
  ✓ app-server   background server is not running

{}
9 ok · 2 notes · 1 warn · 1 fail failed

Run codex doctor without --summary for detailed diagnostics.
--all expand truncated lists       --json redacted report
",
            "─".repeat(SEPARATOR_WIDTH)
        );
        assert_eq!(rendered, expected);
    }

    #[test]
    fn render_human_report_supports_ascii_output() {
        let rendered = render_human_report(
            &sample_report(),
            HumanOutputOptions {
                show_details: false,
                show_all: false,
                ascii: true,
                color_enabled: false,
            },
        );
        let expected = format!(
            "\
Codex Doctor v0.0.0

Notes
   [!!] terminal     narrow terminal
   [XX] auth         token expired - Run `codex login`.
-------------------------------------------------------------

Environment
  [ok] runtime      running local build on darwin-arm64
  [ok] install      consistent
  [ok] search       search is OK (bundled)
  [!!] terminal     narrow terminal
  [ok] state        state paths inspectable

Configuration
  [XX] auth         token expired - Run `codex login`.

Updates
  [ok] updates      update configuration is locally consistent

Connectivity
  [ok] network      network environment readable
  [ok] websocket    Responses WebSocket handshake succeeded
  [ok] reachability active provider endpoints are reachable over HTTP

Background Server
  [ok] app-server   background server is not running

{}
9 ok | 2 notes | 1 warn | 1 fail failed

Run codex doctor without --summary for detailed diagnostics.
--all expand truncated lists       --json redacted report
",
            "-".repeat(SEPARATOR_WIDTH)
        );
        assert_eq!(rendered, expected);
    }

    #[test]
    fn render_human_report_includes_redacted_details() {
        let rendered = render_human_report(
            &sample_report(),
            HumanOutputOptions {
                show_details: true,
                show_all: false,
                ascii: false,
                color_enabled: false,
            },
        );
        assert!(rendered.contains("      OPENAI_API_KEY           present"));
    }

    #[test]
    fn render_human_report_explains_terminal_warning_issue() {
        let report = DoctorReport {
            schema_version: 1,
            generated_at: "0s since unix epoch".to_string(),
            overall_status: CheckStatus::Warning,
            codex_version: "0.0.0".to_string(),
            checks: vec![
                DoctorCheck::new(
                    "terminal.env",
                    "terminal",
                    CheckStatus::Warning,
                    "width 79 cols - output may wrap (recommended >=80)",
                )
                .detail("terminal: Ghostty")
                .detail("terminal version: 1.3.1")
                .detail("terminal size: 79x26")
                .issue(
                    super::super::DoctorIssue::new(
                        CheckStatus::Warning,
                        "width 79 cols - output may wrap (recommended >=80)",
                    )
                    .expected(">= 80 columns")
                    .remedy("resize the window to at least 80 columns")
                    .field("terminal size"),
                ),
            ],
        };

        let rendered = render_human_report(&report, detailed_no_color_unicode_options());

        assert!(
            rendered.contains("⚠ terminal     width 79 cols - output may wrap (recommended >=80)")
        );
        assert!(rendered.contains("▸ terminal size            79x26 (expected >= 80 columns)"));
        assert!(rendered.contains("→ resize the window to at least 80 columns"));
        assert!(!rendered.contains("⚠ terminal     Ghostty 1.3.1"));
    }

    #[test]
    fn render_human_report_promotes_notes_without_changing_statuses() {
        let report = DoctorReport {
            schema_version: 1,
            generated_at: "0s since unix epoch".to_string(),
            overall_status: CheckStatus::Warning,
            codex_version: "0.0.0".to_string(),
            checks: vec![
                DoctorCheck::new(
                    "updates.status",
                    "updates",
                    CheckStatus::Ok,
                    "update configuration is locally consistent",
                )
                .detail("latest version status: newer version is available")
                .detail("latest version: 0.130.0")
                .detail("dismissed version: 0.128.0"),
                DoctorCheck::new(
                    "state.paths",
                    "state",
                    CheckStatus::Ok,
                    "state paths inspectable",
                )
                .detail("active rollout files: 1515 files, 2702146365 total bytes, 1783594 average bytes"),
                DoctorCheck::new(
                    "sandbox.helpers",
                    "sandbox",
                    CheckStatus::Ok,
                    "sandbox configuration is readable",
                )
                .detail("filesystem sandbox: danger-full-access")
                .detail("network sandbox: restricted")
                .detail("approval policy: Never"),
                DoctorCheck::new(
                    "mcp.config",
                    "mcp",
                    CheckStatus::Warning,
                    "MCP configuration has optional issues",
                ),
                DoctorCheck::new(
                    "network.websocket_reachability",
                    "websocket",
                    CheckStatus::Ok,
                    "Responses WebSocket handshake succeeded",
                )
                .detail("auth mode: chatgpt"),
                DoctorCheck::new(
                    "network.provider_reachability",
                    "reachability",
                    CheckStatus::Ok,
                    "active provider endpoints are reachable over HTTP",
                )
                .detail("reachability mode: API key auth"),
                DoctorCheck::new(
                    "app_server.status",
                    "app-server",
                    CheckStatus::Ok,
                    "background server is not running",
                )
                .detail("status: not running")
                .detail("mode: ephemeral"),
            ],
        };

        let rendered = render_human_report(&report, summary_no_color_unicode_options());

        assert!(rendered.contains("Notes\n   ↑ updates"));
        assert!(rendered.contains("0.130.0 available (current 0.0.0, dismissed 0.128.0)"));
        assert!(rendered.contains("⚠ rollouts"));
        assert!(rendered.contains("⚠ sandbox"));
        assert!(rendered.contains("⚠ mcp"));
        assert!(rendered.contains(
            "⚠ auth         mixed auth signals: ChatGPT login plus API key env var; HTTP reachability uses API-key mode"
        ));
        assert!(rendered.contains("○ app-server   not running (ephemeral mode)"));
        assert!(rendered.contains("5 ok · 1 idle · 5 notes · 1 warn · 0 fail degraded"));
    }

    #[test]
    fn render_human_report_expands_feature_flags_with_all() {
        let report = DoctorReport {
            schema_version: 1,
            generated_at: "0s since unix epoch".to_string(),
            overall_status: CheckStatus::Ok,
            codex_version: "0.0.0".to_string(),
            checks: vec![
                DoctorCheck::new("config.load", "config", CheckStatus::Ok, "config loaded")
                    .detail("model: gpt-5.5")
                    .detail("model provider: openai")
                    .detail("feature flags enabled: 3")
                    .detail("enabled feature flags: shell_tool, memories, goals")
                    .detail("feature flag overrides: memories=true"),
            ],
        };

        let compact = render_human_report(&report, detailed_no_color_unicode_options());
        let expanded = render_human_report(&report, detailed_all_no_color_unicode_options());

        assert!(!compact.contains("enabled flags"));
        assert!(
            compact.contains(
                "feature flags            3 enabled · 1 overridden (full list with --all)"
            )
        );
        assert!(expanded.contains("enabled flags            shell_tool, memories, goals"));
    }

    #[test]
    fn detail_value_colors_inline_statuses_and_low_signal_values() {
        let rendered = detail_value(
            "npm: no · commit unknown · integrity ok · ~/code/codex/target/debug/codex · <redacted>",
            detailed_color_unicode_options(),
        );

        assert!(rendered.contains("npm: \u{1b}[38;5;240mno"));
        assert!(rendered.contains("\u{1b}[38;5;240munknown"));
        assert!(rendered.contains("\u{1b}[38;5;10mok"));
        assert!(rendered.contains("\u{1b}[38;5;117m~/code/codex/target/debug/codex"));
        assert!(rendered.contains("\u{1b}[38;5;244m"));
    }

    #[test]
    fn update_note_emphasizes_available_version_and_dims_context() {
        let rendered = style_update_note_summary(
            "0.130.0 available (current 0.0.0, dismissed 0.128.0)",
            detailed_color_unicode_options(),
        );

        assert!(rendered.contains("\u{1b}[38;5;220m0.130.0 available"));
        assert!(rendered.contains("\u{1b}[2m(current 0.0.0, dismissed 0.128.0)"));
    }

    #[test]
    fn redact_detail_sanitizes_urls() {
        let redacted = redact_detail(
            "reachability failed: https://user:pass@example.com/mcp?x=abc#frag (connect failed)",
        );

        assert_eq!(
            redacted,
            "reachability failed: https://example.com/mcp (connect failed)"
        );
    }

    #[test]
    fn redact_detail_sanitizes_secret_url_path_segments() {
        let redacted = redact_detail("reachability failed: https://example.com/mcp/abc123xyz");

        assert_eq!(
            redacted,
            "reachability failed: https://example.com/mcp/<redacted>"
        );
    }

    #[test]
    fn redact_detail_preserves_env_var_names() {
        assert_eq!(
            redact_detail("auth env vars present: OPENAI_API_KEY, CODEX_API_KEY"),
            "auth env vars present: OPENAI_API_KEY, CODEX_API_KEY"
        );
    }

    #[test]
    fn redact_detail_preserves_secret_presence_booleans() {
        assert_eq!(
            redact_detail("stored ChatGPT tokens: true"),
            "stored ChatGPT tokens: true"
        );
        assert_eq!(
            redact_detail("stored ChatGPT tokens: false"),
            "stored ChatGPT tokens: false"
        );
    }

    #[test]
    fn render_human_report_can_emit_color() {
        let rendered = render_human_report(
            &sample_report(),
            HumanOutputOptions {
                show_details: false,
                show_all: false,
                ascii: false,
                color_enabled: true,
            },
        );
        assert!(rendered.contains("\u{1b}["));
    }
}
