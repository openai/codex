use std::fmt::Write as _;

use owo_colors::OwoColorize;

use super::CheckStatus;
use super::DoctorCheck;
use super::DoctorMode;
use super::DoctorReport;

const NAME_WIDTH: usize = 12;
const SEPARATOR_WIDTH: usize = 45;

const GROUPS: &[OutputGroup] = &[
    OutputGroup {
        title: "Environment",
        keys: &["install", "terminal", "state"],
    },
    OutputGroup {
        title: "Configuration",
        keys: &["config", "auth", "mcp", "sandbox"],
    },
    OutputGroup {
        title: "Connectivity",
        keys: &["network", "app-server", "deep"],
    },
];

struct OutputGroup {
    title: &'static str,
    keys: &'static [&'static str],
}

#[derive(Clone, Copy, Debug)]
pub(super) struct HumanOutputOptions {
    pub(super) verbose: bool,
    pub(super) ascii: bool,
    pub(super) color_enabled: bool,
}

pub(super) fn render_human_report(report: &DoctorReport, options: HumanOutputOptions) -> String {
    let mut out = String::new();
    let title_separator = if options.ascii { " - " } else { " · " };
    let _ = writeln!(
        out,
        "{} {}{}{} mode",
        bold("Codex Doctor", options),
        dim(&format!("v{}", report.codex_version), options),
        dim(title_separator, options),
        dim(report.mode.name(), options)
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
    write_footer(&mut out, report, options);
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
                .filter(move |check| output_key(check) == *key)
        })
        .collect()
}

fn output_key(check: &DoctorCheck) -> &str {
    if is_deep_probe_check(check) {
        "deep"
    } else {
        &check.category
    }
}

fn display_name(check: &DoctorCheck) -> &str {
    if is_deep_probe_check(check) {
        "deep probes"
    } else {
        &check.category
    }
}

fn is_deep_probe_check(check: &DoctorCheck) -> bool {
    check.id == "deep.probes" || check.id == "network.deep_reachability"
}

fn write_check_row(out: &mut String, check: &DoctorCheck, options: HumanOutputOptions) {
    let description = row_description(check, options);
    let _ = writeln!(
        out,
        "  {} {:<NAME_WIDTH$} {}",
        status_marker(check.status, options),
        display_name(check),
        style_description(&description, check.status, options)
    );

    if options.verbose {
        for detail in &check.details {
            let _ = writeln!(
                out,
                "    - {}",
                dim(
                    &highlight_actions(&redact_for_display(detail), options),
                    options
                )
            );
        }
    }
}

fn row_description(check: &DoctorCheck, options: HumanOutputOptions) -> String {
    if check.id == "deep.probes" {
        return format!("skipped{}run with --deep", dash(options));
    }

    if matches!(check.status, CheckStatus::Warning | CheckStatus::Fail)
        && let Some(remediation) = &check.remediation
    {
        let dash = dash(options);
        let summary = &check.summary;
        return format!("{summary}{dash}{remediation}");
    }

    check.summary.clone()
}

fn dash(options: HumanOutputOptions) -> &'static str {
    if options.ascii { " - " } else { " — " }
}

fn status_marker(status: CheckStatus, options: HumanOutputOptions) -> String {
    let marker = if options.ascii {
        match status {
            CheckStatus::Ok => "[ok]",
            CheckStatus::Warning => "[!!]",
            CheckStatus::Fail => "[XX]",
            CheckStatus::Skipped => "[--]",
        }
    } else {
        match status {
            CheckStatus::Ok => "✓",
            CheckStatus::Warning => "⚠",
            CheckStatus::Fail => "✗",
            CheckStatus::Skipped => "⊘",
        }
    };

    match status {
        CheckStatus::Ok => green(marker, options),
        CheckStatus::Warning => yellow(marker, options),
        CheckStatus::Fail => red(marker, options),
        CheckStatus::Skipped => dim(marker, options),
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
        CheckStatus::Skipped => dim(&highlighted, options),
    }
}

fn summary_line(report: &DoctorReport, options: HumanOutputOptions) -> String {
    let counts = StatusCounts::from_checks(&report.checks);
    let separator = dim(if options.ascii { " | " } else { " · " }, options);
    let status = overall_status_label(report.overall_status);
    format!(
        "{}{}{}{}{}{}{} {}",
        count_label(counts.ok, "ok", CheckStatus::Ok, options),
        separator,
        count_label(counts.warning, "warn", CheckStatus::Warning, options),
        separator,
        count_label(counts.fail, "fail", CheckStatus::Fail, options),
        separator,
        count_label(counts.skipped, "skipped", CheckStatus::Skipped, options),
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
        CheckStatus::Skipped => dim(label, options),
    };
    format!("{count} {label}")
}

fn overall_status_label(status: CheckStatus) -> &'static str {
    match status {
        CheckStatus::Ok | CheckStatus::Skipped => "ok",
        CheckStatus::Warning => "degraded",
        CheckStatus::Fail => "failed",
    }
}

fn styled_overall_status(label: &str, status: CheckStatus, options: HumanOutputOptions) -> String {
    if !options.color_enabled {
        return label.to_string();
    }

    match status {
        CheckStatus::Ok | CheckStatus::Skipped => label.green().bold().to_string(),
        CheckStatus::Warning => label.yellow().bold().to_string(),
        CheckStatus::Fail => label.red().bold().to_string(),
    }
}

fn write_footer(out: &mut String, report: &DoctorReport, options: HumanOutputOptions) {
    if matches!(report.mode, DoctorMode::Fast) {
        let _ = writeln!(
            out,
            "{} {}",
            cyan("--deep", options),
            dim("reachability probes (network, app-server, MCP)", options)
        );
    }
    let _ = writeln!(
        out,
        "{} {}",
        cyan("--json", options),
        dim("redacted support report", options)
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

fn redact_for_display(detail: &str) -> String {
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
        detail.to_string()
    }
}

impl DoctorMode {
    fn name(&self) -> &'static str {
        match self {
            DoctorMode::Fast => "fast",
            DoctorMode::Deep => "deep",
        }
    }
}

#[derive(Default)]
struct StatusCounts {
    ok: usize,
    warning: usize,
    fail: usize,
    skipped: usize,
}

impl StatusCounts {
    fn from_checks(checks: &[DoctorCheck]) -> Self {
        let mut counts = Self::default();
        for check in checks {
            match check.status {
                CheckStatus::Ok => counts.ok += 1,
                CheckStatus::Warning => counts.warning += 1,
                CheckStatus::Fail => counts.fail += 1,
                CheckStatus::Skipped => counts.skipped += 1,
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
                "installation",
                "install",
                CheckStatus::Ok,
                "installation looks consistent",
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
                "network.env",
                "network",
                CheckStatus::Ok,
                "network environment readable",
            ),
            DoctorCheck::new(
                "deep.probes",
                "deep",
                CheckStatus::Skipped,
                "deep reachability probes were skipped",
            ),
        ];
        DoctorReport {
            schema_version: 1,
            generated_at: "0s since unix epoch".to_string(),
            mode: DoctorMode::Fast,
            overall_status: CheckStatus::Fail,
            codex_version: "0.0.0".to_string(),
            checks,
        }
    }

    #[test]
    fn render_human_report_groups_checks_without_color() {
        let rendered = render_human_report(&sample_report(), no_color_unicode_options());
        let expected = "\
Codex Doctor v0.0.0 · fast mode

Environment
  ✓ install      installation looks consistent
  ⚠ terminal     narrow terminal
  ✓ state        state paths inspectable

Configuration
  ✗ auth         token expired — Run `codex login`.

Connectivity
  ✓ network      network environment readable
  ⊘ deep probes  skipped — run with --deep

─────────────────────────────────────────────
3 ok · 1 warn · 1 fail · 1 skipped failed

--deep reachability probes (network, app-server, MCP)
--json redacted support report
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
Codex Doctor v0.0.0 - fast mode

Environment
  [ok] install      installation looks consistent
  [!!] terminal     narrow terminal
  [ok] state        state paths inspectable

Configuration
  [XX] auth         token expired - Run `codex login`.

Connectivity
  [ok] network      network environment readable
  [--] deep probes  skipped - run with --deep

{}
3 ok | 1 warn | 1 fail | 1 skipped failed

--deep reachability probes (network, app-server, MCP)
--json redacted support report
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
