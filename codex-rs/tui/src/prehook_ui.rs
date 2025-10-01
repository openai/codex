use ratatui::style::Stylize;
use ratatui::text::{Line, Span};

/// Render lines for a prehook Ask outcome.
pub fn render_ask_modal(message: &str) -> Vec<Line<'static>> {
    let mut out: Vec<Line<'static>> = Vec::new();
    out.push(vec!["⚠ Pre-execution approval required".yellow().bold()].into());
    out.push(vec!["".into()].into());
    for l in crate::wrapping::word_wrap_lines(&[Span::from(message)], 68) {
        out.push(l);
    }
    out.push(vec!["".into()].into());
    out.push(vec!["Press Enter to approve, Esc to cancel".cyan()].into());
    out
}

/// Render lines for a prehook Patch outcome (preview only).
pub fn render_patch_preview(message: Option<&str>, diff: &str) -> Vec<Line<'static>> {
    let mut out: Vec<Line<'static>> = Vec::new();
    out.push(vec!["🛠 Pre-execution patch proposal".magenta().bold()].into());
    if let Some(msg) = message {
        out.push(vec!["".into()].into());
        for l in crate::wrapping::word_wrap_lines(&[Span::from(msg)], 68) {
            out.push(l);
        }
    }
    out.push(vec!["".into()].into());
    // Show a few header lines of the diff to keep preview compact.
    for line in diff.lines().take(12) {
        let styled: Span<'static> = if line.starts_with("+++") || line.starts_with("---") {
            Span::from(line).yellow().into()
        } else if line.starts_with("++") || line.starts_with("+") {
            Span::from(line).green().into()
        } else if line.starts_with("--") || line.starts_with("-") {
            Span::from(line).red().into()
        } else if line.starts_with("@@") {
            Span::from(line).cyan().into()
        } else {
            Span::from(line)
        };
        out.push(vec![styled].into());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use insta::assert_snapshot;

    #[test]
    fn ask_modal_snapshot() {
        let out = render_ask_modal("Human approval required to proceed with applying changes.");
        let text = out
            .into_iter()
            .map(|l| l.spans.into_iter().map(|s| s.content.to_string()).collect::<Vec<_>>().join(""))
            .collect::<Vec<_>>()
            .join("\n");
        assert_snapshot!(text);
    }

    #[test]
    fn patch_preview_snapshot() {
        let diff = "*** Begin Patch\n*** Update File: src/main.rs\n@@\n-println!(\"hello\");\n+println!(\"hello, world!\");\n*** End Patch\n";
        let out = render_patch_preview(Some("Proposed fix for greeting."), diff);
        let text = out
            .into_iter()
            .map(|l| l.spans.into_iter().map(|s| s.content.to_string()).collect::<Vec<_>>().join(""))
            .collect::<Vec<_>>()
            .join("\n");
        assert_snapshot!(text);
    }
}

