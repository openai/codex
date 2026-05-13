//! Renders doctor reports for terminal users.
//!
//! The renderer is intentionally separate from check construction so the JSON
//! report can stay stable while the human view optimizes for scanability. It
//! groups checks by concern, colors only status/actionable tokens, and redacts
//! sensitive detail lines before showing them in verbose output.

use std::fmt::Write as _;

use owo_colors::OwoColorize;

use super::CheckStatus;
use super::DoctorCheck;
use super::DoctorReport;

const NAME_WIDTH: usize = 12;
const SEPARATOR_WIDTH: usize = 45;

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
    pub(super) verbose: bool,
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
        dim(&format!("v{}", report.codex_version), options)
    );
    out.push('\n');

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
    let _ = writeln!(
        out,
        "  {} {:<NAME_WIDTH$} {}",
        status_marker(check.status, options),
        check.category,
        style_description(&description, check.status, options)
    );

    if options.verbose {
        for detail in &check.details {
            let _ = writeln!(
                out,
                "    - {}",
                dim(&highlight_actions(&redact_detail(detail), options), options)
            );
        }
    }
}

fn row_description(check: &DoctorCheck, options: HumanOutputOptions) -> String {
    if matches!(check.status, CheckStatus::Warning | CheckStatus::Fail)
        && let Some(remediation) = &check.remediation
    {
        let dash = if options.ascii { " - " } else { " — " };
        let summary = &check.summary;
        return format!("{summary}{dash}{remediation}");
    }

    check.summary.clone()
}

fn status_marker(status: CheckStatus, options: HumanOutputOptions) -> String {
    let marker = if options.ascii {
        match status {
            CheckStatus::Ok => "[ok]",
            CheckStatus::Warning => "[!!]",
            CheckStatus::Fail => "[XX]",
        }
    } else {
        match status {
            CheckStatus::Ok => "✓",
            CheckStatus::Warning => "⚠",
            CheckStatus::Fail => "✗",
        }
    };

    match status {
        CheckStatus::Ok => green(marker, options),
        CheckStatus::Warning => yellow(marker, options),
        CheckStatus::Fail => red(marker, options),
    }
}

fn style_description(
    description: &str,
    status: CheckStatus,
    options: HumanOutputOptions,
) -> String {
    let highlighted = highlight_actions(description, options);
    match status {
        CheckStatus::Ok => dim(&highlighted, options),
        CheckStatus::Warning => yellow(&highlighted, options),
        CheckStatus::Fail => red(&highlighted, options),
    }
}

fn summary_line(report: &DoctorReport, options: HumanOutputOptions) -> String {
    let counts = StatusCounts::from_checks(&report.checks);
    let separator = dim(if options.ascii { " | " } else { " · " }, options);
    let status = overall_status_label(report.overall_status);
    format!(
        "{}{}{}{}{} {}",
        count_label(counts.ok, "ok", CheckStatus::Ok, options),
        separator,
        count_label(counts.warning, "warn", CheckStatus::Warning, options),
        separator,
        count_label(counts.fail, "fail", CheckStatus::Fail, options),
        styled_overall_status(status, report.overall_status, options)
    )
}

fn count_label(
    count: usize,
    label: &str,
    status: CheckStatus,
    options: HumanOutputOptions,
) -> String {
    let count = dim(&count.to_string(), options);
    let label = match status {
        CheckStatus::Ok => green(label, options),
        CheckStatus::Warning => yellow(label, options),
        CheckStatus::Fail => red(label, options),
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
    let _ = writeln!(
        out,
        "{} {}",
        cyan("--json", options),
        dim("redacted support report", options)
    );
    let _ = writeln!(
        out,
        "{}",
        dim(
            "Still having issues? Run codex doctor --verbose for more details.",
            options
        )
    );
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
    warning: usize,
    fail: usize,
}

impl StatusCounts {
    fn from_checks(checks: &[DoctorCheck]) -> Self {
        let mut counts = Self::default();
        for check in checks {
            match check.status {
                CheckStatus::Ok => counts.ok += 1,
                CheckStatus::Warning => counts.warning += 1,
                CheckStatus::Fail => counts.fail += 1,
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

fn green(text: &str, options: HumanOutputOptions) -> String {
    if options.color_enabled {
        text.green().to_string()
    } else {
        text.to_string()
    }
}

fn yellow(text: &str, options: HumanOutputOptions) -> String {
    if options.color_enabled {
        text.yellow().to_string()
    } else {
        text.to_string()
    }
}

fn red(text: &str, options: HumanOutputOptions) -> String {
    if options.color_enabled {
        text.red().to_string()
    } else {
        text.to_string()
    }
}

fn cyan(text: &str, options: HumanOutputOptions) -> String {
    if options.color_enabled {
        text.cyan().to_string()
    } else {
        text.to_string()
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    fn no_color_unicode_options() -> HumanOutputOptions {
        HumanOutputOptions {
            verbose: false,
            ascii: false,
            color_enabled: false,
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
    fn render_human_report_groups_checks_without_color() {
        let rendered = render_human_report(&sample_report(), no_color_unicode_options());
        let expected = "\
Codex Doctor v0.0.0

Environment
  ✓ runtime      running local build on darwin-arm64
  ✓ install      installation looks consistent
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

─────────────────────────────────────────────
9 ok · 1 warn · 1 fail failed

--json redacted support report
Still having issues? Run codex doctor --verbose for more details.
";
        assert_eq!(rendered, expected);
    }

    #[test]
    fn render_human_report_supports_ascii_output() {
        let rendered = render_human_report(
            &sample_report(),
            HumanOutputOptions {
                verbose: false,
                ascii: true,
                color_enabled: false,
            },
        );
        let expected = format!(
            "\
Codex Doctor v0.0.0

Environment
  [ok] runtime      running local build on darwin-arm64
  [ok] install      installation looks consistent
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
9 ok | 1 warn | 1 fail failed

--json redacted support report
Still having issues? Run codex doctor --verbose for more details.
",
            "-".repeat(SEPARATOR_WIDTH)
        );
        assert_eq!(rendered, expected);
    }

    #[test]
    fn render_human_report_includes_verbose_redacted_details() {
        let rendered = render_human_report(
            &sample_report(),
            HumanOutputOptions {
                verbose: true,
                ascii: false,
                color_enabled: false,
            },
        );
        assert!(rendered.contains("    - OPENAI_API_KEY: <redacted>"));
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
    fn render_human_report_can_emit_color() {
        let rendered = render_human_report(
            &sample_report(),
            HumanOutputOptions {
                verbose: false,
                ascii: false,
                color_enabled: true,
            },
        );
        assert!(rendered.contains("\u{1b}["));
    }
}
