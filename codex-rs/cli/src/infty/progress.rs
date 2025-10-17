use chrono::Local;
use codex_core::protocol::AgentMessageEvent;
use codex_core::protocol::EventMsg;
use codex_infty::AggregatedVerifierVerdict;
use codex_infty::DirectiveResponse;
use codex_infty::ProgressReporter;
use codex_infty::VerifierDecision;
use codex_infty::VerifierVerdict;
use crossterm::style::Stylize;
use std::path::Path;
use supports_color::Stream;

#[derive(Debug, Default, Clone)]
pub(crate) struct TerminalProgressReporter;

impl TerminalProgressReporter {
    pub(crate) fn with_color(_color_enabled: bool) -> Self {
        Self
    }

    fn format_role_label(&self, role: &str) -> String {
        let lower = role.to_ascii_lowercase();
        if lower == "solver" {
            return "[solver]".magenta().bold().to_string();
        }
        if lower == "director" {
            return "[director]".blue().bold().to_string();
        }
        if lower == "user" {
            return "[user]".cyan().bold().to_string();
        }
        if lower.contains("verifier") {
            return format!("[{role}]").green().bold().to_string();
        }
        format!("[{role}]").magenta().bold().to_string()
    }

    fn timestamp(&self) -> String {
        let timestamp = Local::now().format("%H:%M:%S");
        let display = format!("[{timestamp}]");
        if supports_color::on(Stream::Stdout).is_some() {
            format!("{}", display.dim())
        } else {
            display
        }
    }

    fn print_exchange(
        &self,
        from_role: &str,
        to_role: &str,
        lines: Vec<String>,
        trailing_blank_line: bool,
    ) {
        let header = format!(
            "{} ----> {}",
            self.format_role_label(from_role),
            self.format_role_label(to_role)
        );
        println!("{} {header}", self.timestamp());
        for line in lines {
            println!("{line}");
        }
        if trailing_blank_line {
            println!();
        }
    }

    fn format_decision(&self, decision: VerifierDecision) -> String {
        match decision {
            VerifierDecision::Pass => "pass".green().bold().to_string(),
            VerifierDecision::Fail => "fail".red().bold().to_string(),
        }
    }
}

impl ProgressReporter for TerminalProgressReporter {
    fn objective_posted(&self, objective: &str) {
        let objective_line = format!("{}", format!("→ objective: {objective}").dim());
        self.print_exchange("user", "solver", vec![objective_line], true);
    }

    fn solver_event(&self, event: &EventMsg) {
        match serde_json::to_string_pretty(event) {
            Ok(json) => {
                tracing::debug!("[solver:event]\n{json}");
            }
            Err(err) => {
                tracing::warn!("[solver:event] (failed to serialize: {err}) {event:?}");
            }
        }
    }

    fn role_event(&self, role: &str, event: &EventMsg) {
        match serde_json::to_string_pretty(event) {
            Ok(json) => {
                tracing::debug!("[{role}:event]\n{json}");
            }
            Err(err) => {
                tracing::warn!("[{role}:event] (failed to serialize: {err}) {event:?}");
            }
        }
    }

    fn solver_agent_message(&self, agent_msg: &AgentMessageEvent) {
        tracing::info!("Agent Message: {agent_msg:?}");
    }

    fn invalid_solver_signal(&self, raw_message: &str) {
        let heading = "Warning".yellow().bold();
        let body = format!(
            "solver reply did not match expected JSON signal; got: {}",
            raw_message
        );
        println!("{} {} {}", self.timestamp(), heading, body);
    }

    fn direction_request(&self, prompt: &str) {
        let prompt_line = format!("{}", prompt.yellow());
        self.print_exchange("solver", "director", vec![prompt_line], true);
    }

    fn director_response(&self, directive: &DirectiveResponse) {
        let suffix = directive
            .rationale
            .as_deref()
            .filter(|rationale| !rationale.is_empty())
            .map(|rationale| format!(" (rationale: {rationale})"))
            .unwrap_or_default();
        let directive_line = format!("{}{}", directive.directive, suffix);
        self.print_exchange("director", "solver", vec![directive_line], true);
    }

    fn verification_request(&self, claim_path: &str, notes: Option<&str>) {
        let mut lines = Vec::new();
        let path_line = format!("→ path: {claim_path}");
        lines.push(format!("{}", path_line.dim()));
        if let Some(notes) = notes.filter(|notes| !notes.is_empty()) {
            let note_line = format!("→ note: {notes}");
            lines.push(format!("{}", note_line.dim()));
        }
        self.print_exchange("solver", "verifier", lines, true);
    }

    fn verifier_verdict(&self, role: &str, verdict: &VerifierVerdict) {
        let decision = self.format_decision(verdict.verdict);
        let mut lines = Vec::new();
        lines.push(format!("verdict: {decision}"));
        if !verdict.reasons.is_empty() {
            let reasons = verdict.reasons.join("; ");
            let reason_line = format!("→ reasons: {reasons}");
            lines.push(format!("{}", reason_line.dim()));
        }
        if !verdict.suggestions.is_empty() {
            let suggestions = verdict.suggestions.join("; ");
            let suggestion_line = format!("→ suggestions: {suggestions}");
            lines.push(format!("{}", suggestion_line.dim()));
        }
        self.print_exchange(role, "solver", lines, false);
    }

    fn verification_summary(&self, summary: &AggregatedVerifierVerdict) {
        let decision = self.format_decision(summary.overall);
        let heading = "Verification summary".bold();
        let summary_line = format!("{heading}: {decision}");
        self.print_exchange("verifier", "solver", vec![summary_line], true);
    }

    fn final_delivery(&self, deliverable_path: &Path, summary: Option<&str>) {
        let delivery_line = format!(
            "{}",
            format!("→ path: {}", deliverable_path.display()).dim()
        );
        let summary_line = format!(
            "{}",
            format!("→ summary: {}", summary.unwrap_or("<none>")).dim()
        );
        self.print_exchange(
            "solver",
            "verifier",
            vec![delivery_line, summary_line],
            true,
        );
    }

    fn run_interrupted(&self) {
        println!(
            "{}",
            "Run interrupted by Ctrl+C. Shutting down sessions…"
                .red()
                .bold(),
        );
    }
}
