use codex_common::CliConfigOverrides;
use codex_common::create_config_summary_entries;
use codex_core::protocol::TokenUsage;
use codex_tui::status::StatusAccountDisplay;
use codex_tui::status::StatusRateLimitData;
use codex_tui::status::compose_account_display;
use codex_tui::status::compose_agents_summary;
use codex_tui::status::compose_model_display;
use codex_tui::status::compose_rate_limit_data;
use codex_tui::status::format_directory_display;
use codex_tui::status::format_status_limit_summary;
use codex_tui::status::format_tokens_compact;
use codex_tui::status::render_status_limit_progress_bar;

use crate::login::load_config_or_exit;

const CODEX_VERSION: &str = env!("CARGO_PKG_VERSION");
const INDENT: &str = " ";

pub fn run_status(cli_config_overrides: CliConfigOverrides) -> ! {
    let config = load_config_or_exit(cli_config_overrides);

    let entries = create_config_summary_entries(&config);
    let (model_name, model_details) = compose_model_display(&config, &entries);
    let model_value = if model_details.is_empty() {
        model_name
    } else {
        format!("{model_name} ({})", model_details.join(", "))
    };

    let approval = entries
        .iter()
        .find(|(k, _)| *k == "approval")
        .map(|(_, v)| v.clone())
        .unwrap_or_else(|| "<unknown>".to_string());

    let sandbox = match &config.sandbox_policy {
        codex_core::protocol::SandboxPolicy::DangerFullAccess => "danger-full-access".to_string(),
        codex_core::protocol::SandboxPolicy::ReadOnly => "read-only".to_string(),
        codex_core::protocol::SandboxPolicy::WorkspaceWrite { .. } => "workspace-write".to_string(),
    };

    let agents_summary = compose_agents_summary(&config);
    let account_value = compose_account_display(&config).map(account_to_string);

    let directory_value = format_directory_display(&config.cwd, None);

    let usage = TokenUsage::default();
    let rate_limit_data = compose_rate_limit_data(None);

    let mut fields = vec![
        DisplayField::new("Model", model_value),
        DisplayField::new("Directory", directory_value),
        DisplayField::new("Approval", approval),
        DisplayField::new("Sandbox", sandbox),
        DisplayField::new("Agents.md", agents_summary),
    ];

    if let Some(account) = account_value {
        fields.push(DisplayField::new("Account", account));
    }

    let total_fmt = format_tokens_compact(usage.blended_total());
    let input_fmt = format_tokens_compact(usage.non_cached_input());
    let output_fmt = format_tokens_compact(usage.output_tokens);
    let token_value = format!("{total_fmt} total ({input_fmt} input + {output_fmt} output)");
    fields.push(DisplayField::new("Token usage", token_value).with_section_break());

    match rate_limit_data {
        StatusRateLimitData::Available(rows) => {
            if rows.is_empty() {
                fields.push(DisplayField::new("Limits", "data not available yet"));
            } else {
                for row in rows {
                    let mut value = format!(
                        "{} {}",
                        render_status_limit_progress_bar(row.percent_used),
                        format_status_limit_summary(row.percent_used)
                    );
                    if let Some(resets_at) = row.resets_at.as_ref() {
                        value = format!("{value} (resets {resets_at})");
                    }
                    fields.push(DisplayField::new(row.label.clone(), value));
                }
            }
        }
        StatusRateLimitData::Missing => {
            fields.push(DisplayField::new(
                "Limits",
                "send a message to load usage data",
            ));
        }
    }

    let mut lines = Vec::new();
    lines.push(format!("{INDENT}>_ OpenAI Codex (v{CODEX_VERSION})"));
    lines.push(String::new());
    lines.extend(format_fields(&fields));

    for line in lines {
        println!("{line}");
    }

    std::process::exit(0);
}

fn account_to_string(account: StatusAccountDisplay) -> String {
    match account {
        StatusAccountDisplay::ChatGpt { email, plan } => match (email, plan) {
            (Some(email), Some(plan)) => format!("{email} ({plan})"),
            (Some(email), None) => email,
            (None, Some(plan)) => plan,
            (None, None) => "ChatGPT".to_string(),
        },
        StatusAccountDisplay::ApiKey => {
            "API key configured (run codex login to use ChatGPT)".to_string()
        }
    }
}

struct DisplayField {
    label: String,
    value: String,
    continuations: Vec<String>,
    section_break_before: bool,
}

impl DisplayField {
    fn new(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
            continuations: Vec::new(),
            section_break_before: false,
        }
    }

    fn with_section_break(mut self) -> Self {
        self.section_break_before = true;
        self
    }
}

fn format_fields(fields: &[DisplayField]) -> Vec<String> {
    if fields.is_empty() {
        return Vec::new();
    }

    let label_width = fields.iter().map(|f| f.label.len()).max().unwrap_or(0);
    let value_offset = INDENT.len() + label_width + 1 + 3;
    let value_prefix = " ".repeat(value_offset);

    let mut lines = Vec::new();
    for field in fields {
        if field.section_break_before && !lines.is_empty() {
            lines.push(String::new());
        }

        let line = if field.label.is_empty() {
            format!("{value_prefix}{}", field.value)
        } else {
            let mut prefix = String::with_capacity(value_offset);
            prefix.push_str(INDENT);
            prefix.push_str(&field.label);
            prefix.push(':');
            let padding = 3 + label_width.saturating_sub(field.label.len());
            for _ in 0..padding {
                prefix.push(' ');
            }
            format!("{prefix}{}", field.value)
        };
        lines.push(line);

        for cont in &field.continuations {
            lines.push(format!("{value_prefix}{cont}"));
        }
    }

    lines
}
