//! Expanded command history, preserving reasoning between grouped exploration calls.
//!
//! Compact history groups adjacent exploration; expanded history retains chronological details,
//! and raw history continues to omit transcript-only reasoning.

use super::model::ExecCell;
use crate::exec_command::strip_bash_lc_and_escape;
use crate::history_cell::HistoryRenderMode;
use crate::render::highlight::highlight_bash_to_lines;
use crate::render::line_utils::push_owned_lines;
use crate::wrapping::RtOptions;
use crate::wrapping::adaptive_wrap_line;
use crate::wrapping::adaptive_wrap_lines;
use codex_ansi_escape::ansi_escape_line;
use codex_utils_elapsed::format_duration;
use ratatui::prelude::*;

impl ExecCell {
    pub(super) fn detailed_lines(&self, width: u16, mode: HistoryRenderMode) -> Vec<Line<'static>> {
        let mut lines: Vec<Line<'static>> = vec![];
        let mut reasoning = self.reasoning.iter().peekable();
        for (i, call) in self.iter_calls().enumerate() {
            if i > 0 {
                lines.push("".into());
            }
            let script = strip_bash_lc_and_escape(&call.command);
            let highlighted_script = highlight_bash_to_lines(&script);
            let cmd_display = adaptive_wrap_lines(
                &highlighted_script,
                RtOptions::new(width as usize)
                    .initial_indent("$ ".magenta().into())
                    .subsequent_indent("    ".into()),
            );
            lines.extend(cmd_display);

            if let Some(output) = call.output.as_ref() {
                if !call.is_unified_exec_interaction() {
                    let wrap_width = width.max(1) as usize;
                    let wrap_opts = RtOptions::new(wrap_width);
                    for unwrapped in output
                        .transcript_lines()
                        .map(|line| ansi_escape_line(line.as_ref()))
                    {
                        let wrapped = adaptive_wrap_line(&unwrapped, wrap_opts.clone());
                        push_owned_lines(&wrapped, &mut lines);
                    }
                }
                if let Some(duration) = call.duration {
                    let duration = format_duration(duration);
                    let mut result: Line = if output.exit_code == 0 {
                        Line::from("✓".green().bold())
                    } else {
                        Line::from(vec![
                            "✗".red().bold(),
                            format!(" ({})", output.exit_code).into(),
                        ])
                    };
                    result.push_span(format!(" • {duration}").dim());
                    lines.push(result);
                }
            }
            if mode == HistoryRenderMode::Rich {
                while let Some((_, cell)) =
                    reasoning.next_if(|(after_calls, _)| *after_calls == i + 1)
                {
                    let reasoning_lines = cell.transcript_lines(width);
                    if !reasoning_lines.is_empty() {
                        lines.push("".into());
                        lines.extend(reasoning_lines);
                    }
                }
            }
        }
        lines
    }
}
