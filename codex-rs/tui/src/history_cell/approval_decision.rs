use super::HistoryCell;
use super::prefixed_wrapped::PrefixedWrappedHistoryCell;
use crate::exec_command::strip_bash_lc_and_escape;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;

/// Approval decision banner summarizing how the user responded to a command prompt.
///
/// Used after Codex prompts for a command approval to show whether the user approved/denied. Renders
/// a colored bullet (`✔` for approvals, `✗` for denials/abort) followed by a short sentence with a
/// truncated command snippet, wrapped with a hanging indent so multi-line commands stay aligned.
///
/// # Output
///
/// ```plain
/// ✔ You approved codex to run echo hello every time this session
/// ```
#[derive(Debug)]
pub(crate) struct ApprovalDecisionCell {
    inner: PrefixedWrappedHistoryCell,
}

/// Build a boxed approval decision entry to slot directly into the history stream.
pub(crate) fn new_approval_decision_cell(
    command: Vec<String>,
    decision: codex_core::protocol::ReviewDecision,
) -> Box<dyn super::HistoryCell> {
    ApprovalDecisionCell::new_boxed(command, decision)
}

impl ApprovalDecisionCell {
    pub(crate) fn new_boxed(
        command: Vec<String>,
        decision: codex_core::protocol::ReviewDecision,
    ) -> Box<dyn super::HistoryCell> {
        Box::new(Self::new(command, decision))
    }

    /// Build a decision cell from the full command and the decision result.
    ///
    /// Multiline commands are truncated to a single-line snippet with ellipsis and dimmed; wrapped
    /// lines are prefixed with either the colored bullet or a two-space hanging indent.
    pub(crate) fn new(
        command: Vec<String>,
        decision: codex_core::protocol::ReviewDecision,
    ) -> Self {
        use codex_core::protocol::ReviewDecision::*;

        let (symbol, summary): (Span<'static>, Vec<Span<'static>>) = match decision {
            Approved => {
                let snippet = Span::from(exec_snippet(&command)).dim();
                (
                    "✔ ".green(),
                    vec![
                        "You ".into(),
                        "approved".bold(),
                        " codex to run ".into(),
                        snippet,
                        " this time".bold(),
                    ],
                )
            }
            ApprovedForSession => {
                let snippet = Span::from(exec_snippet(&command)).dim();
                (
                    "✔ ".green(),
                    vec![
                        "You ".into(),
                        "approved".bold(),
                        " codex to run ".into(),
                        snippet,
                        " every time this session".bold(),
                    ],
                )
            }
            Denied => {
                let snippet = Span::from(exec_snippet(&command)).dim();
                (
                    "✗ ".red(),
                    vec![
                        "You ".into(),
                        "did not approve".bold(),
                        " codex to run ".into(),
                        snippet,
                    ],
                )
            }
            Abort => {
                let snippet = Span::from(exec_snippet(&command)).dim();
                (
                    "✗ ".red(),
                    vec![
                        "You ".into(),
                        "canceled".bold(),
                        " the request to run ".into(),
                        snippet,
                    ],
                )
            }
        };

        Self {
            inner: PrefixedWrappedHistoryCell::new(Line::from(summary), symbol, "  "),
        }
    }
}

impl HistoryCell for ApprovalDecisionCell {
    /// Forward to the wrapped `PrefixedWrappedHistoryCell` so rendering is consistent.
    ///
    /// Shows a colored bullet plus the decision summary, wrapping with a hanging indent to keep long
    /// commands aligned with their prefix.
    fn display_lines(&self, width: u16) -> Vec<Line<'static>> {
        self.inner.display_lines(width)
    }

    /// Use the wrapped cell’s height calculation.
    fn desired_height(&self, width: u16) -> u16 {
        self.inner.desired_height(width)
    }
}

/// Trim multiline commands down to a single preview line with ellipsis and truncation.
fn truncate_exec_snippet(full_cmd: &str) -> String {
    let mut snippet = match full_cmd.split_once('\n') {
        Some((first, _)) => format!("{first} ..."),
        None => full_cmd.to_string(),
    };
    snippet = crate::text_formatting::truncate_text(&snippet, 80);
    snippet
}

/// Convert the raw command vector into a displayable snippet for audit banners.
fn exec_snippet(command: &[String]) -> String {
    let full_cmd = strip_bash_lc_and_escape(command);
    truncate_exec_snippet(&full_cmd)
}

#[cfg(test)]
mod tests {
    use super::*;
    use insta::assert_snapshot;

    fn render(decision: codex_core::protocol::ReviewDecision) -> String {
        let cell = ApprovalDecisionCell::new(
            vec![
                "bash".into(),
                "-lc".into(),
                "echo checking migration script safety before applying".into(),
            ],
            decision,
        );
        cell.display_string(46)
    }

    #[test]
    fn approved_for_session_shows_every_time_copy() {
        let rendered = render(codex_core::protocol::ReviewDecision::ApprovedForSession);
        assert!(rendered.contains("every time this session"));
        assert!(rendered.starts_with('✔'));
    }

    #[test]
    fn denied_shows_red_cross() {
        let rendered = render(codex_core::protocol::ReviewDecision::Denied);
        assert!(rendered.starts_with('✗'));
        assert!(rendered.contains("did not approve"));
    }

    #[test]
    fn snapshots_for_each_decision() {
        assert_snapshot!(
            "approved",
            render(codex_core::protocol::ReviewDecision::Approved)
        );
        assert_snapshot!(
            "approved_for_session",
            render(codex_core::protocol::ReviewDecision::ApprovedForSession)
        );
        assert_snapshot!(
            "denied",
            render(codex_core::protocol::ReviewDecision::Denied)
        );
        assert_snapshot!("abort", render(codex_core::protocol::ReviewDecision::Abort));
    }
}
