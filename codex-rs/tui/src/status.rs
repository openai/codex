use crate::exec_command::relativize_to_home;
use crate::history_cell::CompositeHistoryCell;
use crate::history_cell::HistoryCell;
use crate::history_cell::PlainHistoryCell;
use crate::history_cell::with_border_with_inner_width;
use crate::version::CODEX_CLI_VERSION;
use chrono::DateTime;
use chrono::Duration as ChronoDuration;
use chrono::Local;
use codex_common::create_config_summary_entries;
use codex_core::auth::get_auth_file;
use codex_core::auth::try_read_auth_json;
use codex_core::config::Config;
use codex_core::project_doc::discover_project_doc_paths;
use codex_core::protocol::RateLimitSnapshot;
use codex_core::protocol::RateLimitWindow;
use codex_core::protocol::SandboxPolicy;
use codex_core::protocol::TokenUsage;
use codex_protocol::mcp_protocol::ConversationId;
use ratatui::prelude::*;
use ratatui::style::Stylize;
use std::convert::TryFrom;
use std::path::Path;
use std::path::PathBuf;
use unicode_width::UnicodeWidthChar;
use unicode_width::UnicodeWidthStr;

const STATUS_LIMIT_BAR_SEGMENTS: usize = 20;
const STATUS_LIMIT_BAR_FILLED: &str = "█";
const STATUS_LIMIT_BAR_EMPTY: &str = "░";
const RESET_BULLET: &str = "·";

fn label_display(label: &str) -> String {
    format!(" {label}: ")
}

fn label_span(label: &str) -> Span<'static> {
    Span::from(label_display(label)).dim()
}

fn label_width(label: &str) -> usize {
    UnicodeWidthStr::width(label_display(label).as_str())
}

#[derive(Debug)]
struct StatusField {
    label: &'static str,
    value: Vec<Span<'static>>,
}

impl StatusField {
    fn text(label: &'static str, value: impl Into<String>) -> Self {
        Self {
            label,
            value: vec![Span::from(value.into())],
        }
    }

    fn spans(label: &'static str, value: Vec<Span<'static>>) -> Self {
        Self { label, value }
    }
}

#[derive(Debug, Default)]
struct StatusRows {
    lines: Vec<Line<'static>>,
}

impl StatusRows {
    fn new() -> Self {
        Self { lines: Vec::new() }
    }

    fn push_blank(&mut self) {
        self.lines.push(Line::from(Vec::<Span<'static>>::new()));
    }

    fn push_line(&mut self, spans: Vec<Span<'static>>) {
        self.lines.push(Line::from(spans));
    }

    fn push_field(&mut self, field: StatusField) {
        let mut spans = Vec::with_capacity(field.value.len() + 1);
        spans.push(label_span(field.label));
        spans.extend(field.value);
        self.lines.push(Line::from(spans));
    }

    fn extend_fields<I>(&mut self, fields: I)
    where
        I: IntoIterator<Item = StatusField>,
    {
        for field in fields {
            self.push_field(field);
        }
    }

    fn extend_lines<I>(&mut self, lines: I)
    where
        I: IntoIterator<Item = Line<'static>>,
    {
        self.lines.extend(lines);
    }

    fn into_lines(self) -> Vec<Line<'static>> {
        self.lines
    }
}

pub(crate) fn new_status_output(
    config: &Config,
    usage: &TokenUsage,
    session_id: &Option<ConversationId>,
    rate_limits: Option<&RateLimitSnapshotDisplay>,
) -> CompositeHistoryCell {
    let command = PlainHistoryCell::new(vec!["/status".magenta().into()]);
    let card = StatusHistoryCell::new(config, usage, session_id, rate_limits);

    CompositeHistoryCell::new(vec![Box::new(command), Box::new(card)])
}

#[derive(Debug, Clone)]
struct StatusTokenUsageData {
    total: u64,
    input: u64,
    cached_input: u64,
    output: u64,
}

#[derive(Debug, Clone)]
enum StatusAccountDisplay {
    ChatGpt {
        email: Option<String>,
        plan: Option<String>,
    },
    ApiKey,
}

#[derive(Debug, Clone)]
struct StatusRateLimitRow {
    label: &'static str,
    percent_used: f64,
    resets_at: Option<String>,
}

#[derive(Debug, Clone)]
enum StatusRateLimitData {
    Available(Vec<StatusRateLimitRow>),
    Missing,
}

#[derive(Debug)]
struct StatusHistoryCell {
    model_name: String,
    model_details: Vec<String>,
    directory: PathBuf,
    approval: String,
    sandbox: String,
    agents_summary: String,
    account: Option<StatusAccountDisplay>,
    session_id: Option<String>,
    token_usage: StatusTokenUsageData,
    rate_limits: StatusRateLimitData,
}

impl StatusHistoryCell {
    fn new(
        config: &Config,
        usage: &TokenUsage,
        session_id: &Option<ConversationId>,
        rate_limits: Option<&RateLimitSnapshotDisplay>,
    ) -> Self {
        let config_entries = create_config_summary_entries(config);
        let (model_name, model_details) = compose_model_display(config, &config_entries);
        let approval = config_entries
            .iter()
            .find(|(k, _)| *k == "approval")
            .map(|(_, v)| v.clone())
            .unwrap_or_else(|| "<unknown>".to_string());
        let sandbox = match &config.sandbox_policy {
            SandboxPolicy::DangerFullAccess => "danger-full-access".to_string(),
            SandboxPolicy::ReadOnly => "read-only".to_string(),
            SandboxPolicy::WorkspaceWrite { .. } => "workspace-write".to_string(),
        };
        let agents_summary = compose_agents_summary(config);
        let account = compose_account_display(config);
        let session_id = session_id.as_ref().map(std::string::ToString::to_string);
        let token_usage = StatusTokenUsageData {
            total: usage.blended_total(),
            input: usage.non_cached_input(),
            cached_input: usage.cached_input_tokens,
            output: usage.output_tokens,
        };
        let rate_limits = compose_rate_limit_data(rate_limits);

        Self {
            model_name,
            model_details,
            directory: config.cwd.clone(),
            approval,
            sandbox,
            agents_summary,
            account,
            session_id,
            token_usage,
            rate_limits,
        }
    }

    fn primary_fields(&self, inner_width: usize) -> Vec<StatusField> {
        let mut fields = Vec::new();
        let mut model_spans = vec![Span::from(self.model_name.clone())];
        if !self.model_details.is_empty() {
            model_spans.push(Span::from(" (").dim());
            model_spans.push(Span::from(self.model_details.join(", ")).dim());
            model_spans.push(Span::from(")").dim());
        }
        fields.push(StatusField::spans("Model", model_spans));

        let directory_width = inner_width.saturating_sub(label_width("Directory"));
        let directory = format_directory_display(&self.directory, Some(directory_width));
        fields.push(StatusField::text("Directory", directory));

        fields.push(StatusField::text("Approval", self.approval.clone()));
        fields.push(StatusField::text("Sandbox", self.sandbox.clone()));
        fields.push(StatusField::text("Agents.md", self.agents_summary.clone()));

        fields
    }

    fn account_field(&self) -> Option<StatusField> {
        let account = self.account.as_ref()?;
        let value = match account {
            StatusAccountDisplay::ChatGpt { email, plan } => match (email, plan) {
                (Some(email), Some(plan)) => format!("{email} ({plan})"),
                (Some(email), None) => email.clone(),
                (None, Some(plan)) => plan.clone(),
                (None, None) => "ChatGPT".to_string(),
            },
            StatusAccountDisplay::ApiKey => {
                "API key configured (run codex login to use ChatGPT)".to_string()
            }
        };

        Some(StatusField::text("Account", value))
    }

    fn session_field(&self) -> Option<StatusField> {
        self.session_id
            .as_ref()
            .map(|session| StatusField::text("Session", session.clone()))
    }

    fn token_usage_field(&self) -> StatusField {
        StatusField::spans("Token Usage", self.token_usage_spans())
    }

    fn token_usage_spans(&self) -> Vec<Span<'static>> {
        let total_fmt = format_tokens_compact(self.token_usage.total);
        let input_fmt = format_tokens_compact(self.token_usage.input);
        let output_fmt = format_tokens_compact(self.token_usage.output);

        let mut spans: Vec<Span<'static>> = vec![
            Span::from(total_fmt),
            Span::from(" (").dim(),
            Span::from(input_fmt),
            Span::from(" input").dim(),
            Span::from(" + ").dim(),
            Span::from(output_fmt),
            Span::from(" output").dim(),
            Span::from(")").dim(),
        ];

        if self.token_usage.cached_input > 0 {
            let cached_fmt = format_tokens_compact(self.token_usage.cached_input);
            spans.push(Span::from(" + ").dim());
            spans.push(Span::from(format!("{cached_fmt} cached input")).dim());
        }

        spans
    }

    fn rate_limit_lines(&self, available_inner_width: usize) -> Vec<Line<'static>> {
        match &self.rate_limits {
            StatusRateLimitData::Available(rows_data) => {
                if rows_data.is_empty() {
                    return vec![Line::from(vec![
                        label_span("Limits"),
                        Span::from("data not available yet").dim(),
                    ])];
                }

                let mut lines = Vec::new();

                let label_width = rows_data
                    .iter()
                    .map(|row| UnicodeWidthStr::width(row.label))
                    .max()
                    .unwrap_or(0);
                let resets_indent = format!("  {:<label_width$}  ", "");

                for row in rows_data {
                    let padded = format!("{label:<label_width$}", label = row.label);
                    let base_spans = vec![
                        Span::from(format!(" {padded}: ")).dim(),
                        Span::from(render_status_limit_progress_bar(row.percent_used)),
                        Span::from(" "),
                        Span::from(format_status_limit_summary(row.percent_used)),
                    ];

                    if let Some(resets_at) = row.resets_at.as_ref() {
                        let resets_span =
                            Span::from(format!("{RESET_BULLET} resets {resets_at}")).dim();
                        let mut inline_spans = base_spans.clone();
                        inline_spans.push(Span::from(" ").dim());
                        inline_spans.push(resets_span.clone());

                        if line_display_width(&Line::from(inline_spans.clone()))
                            <= available_inner_width
                        {
                            lines.push(Line::from(inline_spans));
                        } else {
                            lines.push(Line::from(base_spans));
                            lines.push(
                                vec![Span::from(resets_indent.clone()).dim(), resets_span].into(),
                            );
                        }
                    } else {
                        lines.push(Line::from(base_spans));
                    }
                }

                lines
            }
            StatusRateLimitData::Missing => {
                vec![Line::from(vec![
                    label_span("Limits"),
                    Span::from("data not available yet").dim(),
                ])]
            }
        }
    }
}

impl HistoryCell for StatusHistoryCell {
    fn display_lines(&self, width: u16) -> Vec<Line<'static>> {
        let mut rows = StatusRows::new();
        rows.push_line(vec![
            Span::from(">_ ").dim(),
            Span::from("OpenAI Codex").bold(),
            Span::from(" ").dim(),
            Span::from(format!("(v{CODEX_CLI_VERSION})")).dim(),
        ]);
        rows.push_blank();
        rows.extend_fields(self.primary_fields(usize::MAX));

        if let Some(account) = self.account_field() {
            rows.push_field(account);
        }

        if let Some(session) = self.session_field() {
            rows.push_field(session);
        }

        rows.push_blank();
        rows.push_field(self.token_usage_field());

        let available_inner_width = usize::from(width.saturating_sub(4));
        if available_inner_width == 0 {
            return Vec::new();
        }

        rows.extend_lines(self.rate_limit_lines(available_inner_width));

        let lines = rows.into_lines();
        let content_width = lines.iter().map(line_display_width).max().unwrap_or(0);
        let inner_width = content_width.min(available_inner_width);
        let truncated_lines: Vec<Line<'static>> = lines
            .into_iter()
            .map(|line| truncate_line_to_width(line, inner_width))
            .collect();

        // Keep the border math centralized so other cards can adopt the helper
        // without each reimplementing padding logic.
        with_border_with_inner_width(truncated_lines, inner_width)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct RateLimitWindowDisplay {
    pub used_percent: f64,
    pub resets_at: Option<String>,
}

impl RateLimitWindowDisplay {
    fn from_window(window: &RateLimitWindow, captured_at: DateTime<Local>) -> Self {
        let resets_at = window
            .resets_in_seconds
            .and_then(|seconds| i64::try_from(seconds).ok())
            .and_then(|secs| captured_at.checked_add_signed(ChronoDuration::seconds(secs)))
            .map(|dt| format_reset_timestamp(dt, captured_at));

        Self {
            used_percent: window.used_percent,
            resets_at,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct RateLimitSnapshotDisplay {
    pub primary: Option<RateLimitWindowDisplay>,
    pub secondary: Option<RateLimitWindowDisplay>,
}

pub(crate) fn rate_limit_snapshot_display(
    snapshot: &RateLimitSnapshot,
    captured_at: DateTime<Local>,
) -> RateLimitSnapshotDisplay {
    RateLimitSnapshotDisplay {
        primary: snapshot
            .primary
            .as_ref()
            .map(|window| RateLimitWindowDisplay::from_window(window, captured_at)),
        secondary: snapshot
            .secondary
            .as_ref()
            .map(|window| RateLimitWindowDisplay::from_window(window, captured_at)),
    }
}

fn compose_model_display(config: &Config, entries: &[(&str, String)]) -> (String, Vec<String>) {
    let mut details: Vec<String> = Vec::new();
    if let Some((_, effort)) = entries.iter().find(|(k, _)| *k == "reasoning effort") {
        details.push(format!("reasoning {}", title_case(effort)));
    }
    if let Some((_, summary)) = entries.iter().find(|(k, _)| *k == "reasoning summaries") {
        let summary = summary.trim();
        if summary.eq_ignore_ascii_case("none") || summary.eq_ignore_ascii_case("off") {
            details.push("summaries off".to_string());
        } else if !summary.is_empty() {
            details.push(format!("summaries {}", title_case(summary)));
        }
    }

    (config.model.clone(), details)
}

fn compose_agents_summary(config: &Config) -> String {
    match discover_project_doc_paths(config) {
        Ok(paths) => {
            let mut rels: Vec<String> = Vec::new();
            for p in paths {
                let display = if let Some(parent) = p.parent() {
                    if parent == config.cwd {
                        "AGENTS.md".to_string()
                    } else {
                        let mut cur = config.cwd.as_path();
                        let mut ups = 0usize;
                        let mut reached = false;
                        while let Some(c) = cur.parent() {
                            if cur == parent {
                                reached = true;
                                break;
                            }
                            cur = c;
                            ups += 1;
                        }
                        if reached {
                            let up = format!("..{}", std::path::MAIN_SEPARATOR);
                            format!("{}AGENTS.md", up.repeat(ups))
                        } else if let Ok(stripped) = p.strip_prefix(&config.cwd) {
                            stripped.display().to_string()
                        } else {
                            p.display().to_string()
                        }
                    }
                } else {
                    p.display().to_string()
                };
                rels.push(display);
            }
            if rels.is_empty() {
                "<none>".to_string()
            } else {
                rels.join(", ")
            }
        }
        Err(_) => "<none>".to_string(),
    }
}

fn compose_account_display(config: &Config) -> Option<StatusAccountDisplay> {
    let auth_file = get_auth_file(&config.codex_home);
    let auth = try_read_auth_json(&auth_file).ok()?;

    if let Some(tokens) = auth.tokens.as_ref() {
        let info = &tokens.id_token;
        let email = info.email.clone();
        let plan = info.get_chatgpt_plan_type().map(|p| title_case(&p));
        return Some(StatusAccountDisplay::ChatGpt { email, plan });
    }

    if let Some(key) = auth.openai_api_key
        && !key.is_empty()
    {
        return Some(StatusAccountDisplay::ApiKey);
    }

    None
}

fn compose_rate_limit_data(snapshot: Option<&RateLimitSnapshotDisplay>) -> StatusRateLimitData {
    match snapshot {
        Some(snapshot) => {
            let mut rows = Vec::new();

            if let Some(primary) = snapshot.primary.as_ref() {
                rows.push(StatusRateLimitRow {
                    label: "5h limit",
                    percent_used: primary.used_percent,
                    resets_at: primary.resets_at.clone(),
                });
            }

            if let Some(secondary) = snapshot.secondary.as_ref() {
                rows.push(StatusRateLimitRow {
                    label: "Weekly limit",
                    percent_used: secondary.used_percent,
                    resets_at: secondary.resets_at.clone(),
                });
            }

            if rows.is_empty() {
                StatusRateLimitData::Missing
            } else {
                StatusRateLimitData::Available(rows)
            }
        }
        None => StatusRateLimitData::Missing,
    }
}

fn format_tokens_compact(value: u64) -> String {
    if value == 0 {
        return "0".to_string();
    }
    if value < 1_000 {
        return value.to_string();
    }

    let (scaled, suffix) = if value >= 1_000_000_000_000 {
        (value as f64 / 1_000_000_000_000.0, "T")
    } else if value >= 1_000_000_000 {
        (value as f64 / 1_000_000_000.0, "B")
    } else if value >= 1_000_000 {
        (value as f64 / 1_000_000.0, "M")
    } else {
        (value as f64 / 1_000.0, "K")
    };

    let decimals = if scaled < 10.0 {
        2
    } else if scaled < 100.0 {
        1
    } else {
        0
    };

    let mut formatted = format!("{scaled:.decimals$}");
    if formatted.contains('.') {
        while formatted.ends_with('0') {
            formatted.pop();
        }
        if formatted.ends_with('.') {
            formatted.pop();
        }
    }

    format!("{formatted}{suffix}")
}

fn render_status_limit_progress_bar(percent_used: f64) -> String {
    let ratio = (percent_used / 100.0).clamp(0.0, 1.0);
    let filled = (ratio * STATUS_LIMIT_BAR_SEGMENTS as f64).round() as usize;
    let filled = filled.min(STATUS_LIMIT_BAR_SEGMENTS);
    let empty = STATUS_LIMIT_BAR_SEGMENTS.saturating_sub(filled);
    format!(
        "[{}{}]",
        STATUS_LIMIT_BAR_FILLED.repeat(filled),
        STATUS_LIMIT_BAR_EMPTY.repeat(empty)
    )
}

fn format_status_limit_summary(percent_used: f64) -> String {
    format!("{percent_used:.0}% used")
}

fn format_reset_timestamp(dt: DateTime<Local>, captured_at: DateTime<Local>) -> String {
    let time = dt.format("%H:%M").to_string();
    if dt.date_naive() == captured_at.date_naive() {
        time
    } else {
        format!("{} ({time})", dt.format("%-d %b"))
    }
}

fn title_case(s: &str) -> String {
    if s.is_empty() {
        return String::new();
    }
    let mut chars = s.chars();
    let first = match chars.next() {
        Some(c) => c,
        None => return String::new(),
    };
    let rest: String = chars.as_str().to_ascii_lowercase();
    first.to_uppercase().collect::<String>() + &rest
}

fn format_directory_display(directory: &Path, max_width: Option<usize>) -> String {
    let formatted = if let Some(rel) = relativize_to_home(directory) {
        if rel.as_os_str().is_empty() {
            "~".to_string()
        } else {
            format!("~{}{}", std::path::MAIN_SEPARATOR, rel.display())
        }
    } else {
        directory.display().to_string()
    };

    if let Some(max_width) = max_width {
        if max_width == 0 {
            return String::new();
        }
        if UnicodeWidthStr::width(formatted.as_str()) > max_width {
            return crate::text_formatting::center_truncate_path(&formatted, max_width);
        }
    }

    formatted
}

fn line_display_width(line: &Line<'static>) -> usize {
    line.iter()
        .map(|span| UnicodeWidthStr::width(span.content.as_ref()))
        .sum()
}

fn truncate_line_to_width(line: Line<'static>, max_width: usize) -> Line<'static> {
    if max_width == 0 {
        return Line::from(Vec::<Span<'static>>::new());
    }

    let mut used = 0usize;
    let mut spans_out: Vec<Span<'static>> = Vec::new();

    for span in line.spans {
        let text = span.content.into_owned();
        let style = span.style;
        let span_width = UnicodeWidthStr::width(text.as_str());

        if span_width == 0 {
            spans_out.push(Span::styled(text, style));
            continue;
        }

        if used >= max_width {
            break;
        }

        if used + span_width <= max_width {
            used += span_width;
            spans_out.push(Span::styled(text, style));
            continue;
        }

        let mut truncated = String::new();
        for ch in text.chars() {
            let ch_width = UnicodeWidthChar::width(ch).unwrap_or(0);
            if used + ch_width > max_width {
                break;
            }
            truncated.push(ch);
            used += ch_width;
        }

        if !truncated.is_empty() {
            spans_out.push(Span::styled(truncated, style));
        }

        break;
    }

    Line::from(spans_out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use codex_core::config::ConfigOverrides;
    use codex_core::config::ConfigToml;
    use codex_protocol::config_types::ReasoningEffort;
    use codex_protocol::config_types::ReasoningSummary;
    use insta::assert_snapshot;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn test_config(temp_home: &TempDir) -> Config {
        Config::load_from_base_config_with_overrides(
            ConfigToml::default(),
            ConfigOverrides::default(),
            temp_home.path().to_path_buf(),
        )
        .expect("load config")
    }

    fn render_lines(lines: &[Line<'static>]) -> Vec<String> {
        lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>()
            })
            .collect()
    }

    fn sanitize_directory(lines: Vec<String>) -> Vec<String> {
        lines
            .into_iter()
            .map(|line| {
                if let (Some(dir_pos), Some(pipe_idx)) = (line.find("Directory: "), line.rfind('│'))
                {
                    let prefix = &line[..dir_pos + "Directory: ".len()];
                    let suffix = &line[pipe_idx..];
                    let content_width = pipe_idx.saturating_sub(dir_pos + "Directory: ".len());
                    let replacement = "[[workspace]]";
                    let mut rebuilt = prefix.to_string();
                    rebuilt.push_str(replacement);
                    if content_width > replacement.len() {
                        rebuilt.push_str(&" ".repeat(content_width - replacement.len()));
                    }
                    rebuilt.push_str(suffix);
                    rebuilt
                } else {
                    line
                }
            })
            .collect()
    }

    #[test]
    fn status_snapshot_includes_reasoning_details() {
        let temp_home = TempDir::new().expect("temp home");
        let mut config = test_config(&temp_home);
        config.model = "gpt-5-codex".to_string();
        config.model_provider_id = "openai".to_string();
        config.model_reasoning_effort = Some(ReasoningEffort::High);
        config.model_reasoning_summary = ReasoningSummary::Detailed;
        config.sandbox_policy = SandboxPolicy::WorkspaceWrite {
            writable_roots: Vec::new(),
            network_access: false,
            exclude_tmpdir_env_var: false,
            exclude_slash_tmp: false,
        };

        config.cwd = PathBuf::from("/workspace/tests");

        let usage = TokenUsage {
            input_tokens: 1_200,
            cached_input_tokens: 200,
            output_tokens: 900,
            reasoning_output_tokens: 150,
            total_tokens: 2_250,
        };

        let snapshot = RateLimitSnapshot {
            primary: Some(RateLimitWindow {
                used_percent: 72.5,
                window_minutes: Some(300),
                resets_in_seconds: Some(600),
            }),
            secondary: Some(RateLimitWindow {
                used_percent: 45.0,
                window_minutes: Some(1_440),
                resets_in_seconds: Some(1_200),
            }),
        };
        let captured_at = chrono::Local
            .with_ymd_and_hms(2024, 1, 2, 3, 4, 5)
            .single()
            .expect("timestamp");
        let rate_display = rate_limit_snapshot_display(&snapshot, captured_at);

        let composite = new_status_output(&config, &usage, &None, Some(&rate_display));
        let mut rendered_lines = render_lines(&composite.display_lines(80));
        if cfg!(windows) {
            for line in &mut rendered_lines {
                *line = line.replace('\\', "/");
            }
        }
        let sanitized = sanitize_directory(rendered_lines).join("\n");
        assert_snapshot!(sanitized);
    }

    #[test]
    fn status_card_preserves_cached_input_when_space_allows() {
        let temp_home = TempDir::new().expect("temp home");
        let mut config = test_config(&temp_home);
        config.model = "gpt-5-codex".to_string();
        config.cwd = PathBuf::from("/workspace/tests");

        let usage = TokenUsage {
            input_tokens: 1_200,
            cached_input_tokens: 200,
            output_tokens: 900,
            reasoning_output_tokens: 0,
            total_tokens: 2_100,
        };

        let composite = new_status_output(&config, &usage, &None, None);
        let rendered = render_lines(&composite.display_lines(120));

        assert!(
            rendered.iter().any(|line| line.contains("cached input")),
            "expected cached input tokens to remain visible, got: {rendered:?}"
        );
    }

    #[test]
    fn status_snapshot_truncates_in_narrow_terminal() {
        let temp_home = TempDir::new().expect("temp home");
        let mut config = test_config(&temp_home);
        config.model = "gpt-5-codex".to_string();
        config.model_provider_id = "openai".to_string();
        config.model_reasoning_effort = Some(ReasoningEffort::High);
        config.model_reasoning_summary = ReasoningSummary::Detailed;
        config.cwd = PathBuf::from("/workspace/tests");

        let usage = TokenUsage {
            input_tokens: 1_200,
            cached_input_tokens: 200,
            output_tokens: 900,
            reasoning_output_tokens: 150,
            total_tokens: 2_250,
        };

        let snapshot = RateLimitSnapshot {
            primary: Some(RateLimitWindow {
                used_percent: 72.5,
                window_minutes: Some(300),
                resets_in_seconds: Some(600),
            }),
            secondary: None,
        };
        let captured_at = chrono::Local
            .with_ymd_and_hms(2024, 1, 2, 3, 4, 5)
            .single()
            .expect("timestamp");
        let rate_display = rate_limit_snapshot_display(&snapshot, captured_at);

        let composite = new_status_output(&config, &usage, &None, Some(&rate_display));
        let mut rendered_lines = render_lines(&composite.display_lines(46));
        if cfg!(windows) {
            for line in &mut rendered_lines {
                *line = line.replace('\\', "/");
            }
        }
        let sanitized = sanitize_directory(rendered_lines).join("\n");

        assert_snapshot!(sanitized);
    }
}
