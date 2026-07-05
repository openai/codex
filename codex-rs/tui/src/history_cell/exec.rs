//! Background terminal interaction and process-summary history cells.

use super::*;

fn rendered_line_text(line: &Line<'_>) -> String {
    line.spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect()
}

fn selection_prefix_width(line: &Line<'_>, prefixes: &[&str]) -> u16 {
    let rendered = rendered_line_text(line);
    for prefix in prefixes {
        if rendered.starts_with(prefix) {
            return u16::try_from(UnicodeWidthStr::width(*prefix)).unwrap_or(u16::MAX);
        }
    }
    0
}

#[derive(Debug)]
pub(crate) struct UnifiedExecInteractionCell {
    command_display: Option<String>,
    stdin: String,
}

impl UnifiedExecInteractionCell {
    pub(crate) fn new(command_display: Option<String>, stdin: String) -> Self {
        Self {
            command_display,
            stdin,
        }
    }

    fn rich_selection_text(&self) -> String {
        let waited_only = self.stdin.is_empty();
        let mut header = if waited_only {
            "Waited for background terminal".to_string()
        } else {
            "Interacted with background terminal".to_string()
        };
        if let Some(command) = self
            .command_display
            .as_ref()
            .filter(|command| !command.is_empty())
        {
            header.push_str(" · ");
            header.push_str(command);
        }

        let mut lines = vec![Line::from(header)];
        if !waited_only {
            lines.extend(raw_lines_from_source(&self.stdin));
        }
        selection_text_from_lines(&lines)
    }
}

impl HistoryCell for UnifiedExecInteractionCell {
    fn display_lines(&self, width: u16) -> Vec<Line<'static>> {
        if width == 0 {
            return Vec::new();
        }
        let wrap_width = width as usize;
        let waited_only = self.stdin.is_empty();

        let mut header_spans = if waited_only {
            vec!["• Waited for background terminal".bold()]
        } else {
            vec!["↳ ".dim(), "Interacted with background terminal".bold()]
        };
        if let Some(command) = &self.command_display
            && !command.is_empty()
        {
            header_spans.push(" · ".dim());
            header_spans.push(command.clone().dim());
        }
        let header = Line::from(header_spans);

        let mut out: Vec<Line<'static>> = Vec::new();
        let header_wrapped = adaptive_wrap_line(&header, RtOptions::new(wrap_width));
        push_owned_lines(&header_wrapped, &mut out);

        if waited_only {
            return out;
        }

        let input_lines: Vec<Line<'static>> = self
            .stdin
            .lines()
            .map(|line| Line::from(line.to_string()))
            .collect();

        let input_wrapped = adaptive_wrap_lines(
            input_lines,
            RtOptions::new(wrap_width)
                .initial_indent(Line::from("  └ ".dim()))
                .subsequent_indent(Line::from("    ".dim())),
        );
        out.extend(input_wrapped);
        out
    }

    fn raw_lines(&self) -> Vec<Line<'static>> {
        let mut out = Vec::new();
        if self.stdin.is_empty() {
            if let Some(command) = self
                .command_display
                .as_ref()
                .filter(|command| !command.is_empty())
            {
                out.push(Line::from(format!(
                    "Waited for background terminal: {command}"
                )));
            } else {
                out.push(Line::from("Waited for background terminal"));
            }
            return out;
        }

        if let Some(command) = self
            .command_display
            .as_ref()
            .filter(|command| !command.is_empty())
        {
            out.push(Line::from(format!(
                "Interacted with background terminal: {command}"
            )));
        } else {
            out.push(Line::from("Interacted with background terminal"));
        }
        out.extend(raw_lines_from_source(&self.stdin));
        out
    }

    fn selection_contribution(&self, width: u16, mode: HistoryRenderMode) -> SelectionContribution {
        match mode {
            HistoryRenderMode::Raw => {
                selection_contribution_from_display_lines(self.raw_lines(), width)
            }
            HistoryRenderMode::Rich => {
                let lines = self.display_lines(width);
                let prefix_columns = lines
                    .iter()
                    .map(|line| selection_prefix_width(line, &["  └ ", "    ", "↳ ", "• "]))
                    .collect::<Vec<_>>();
                selection_contribution_from_semantic_rows(
                    self.rich_selection_text(),
                    lines,
                    width,
                    &prefix_columns,
                )
            }
        }
    }
}

pub(crate) fn new_unified_exec_interaction(
    command_display: Option<String>,
    stdin: String,
) -> UnifiedExecInteractionCell {
    UnifiedExecInteractionCell::new(command_display, stdin)
}

#[derive(Debug)]
struct UnifiedExecProcessesCell {
    processes: Vec<UnifiedExecProcessDetails>,
}

impl UnifiedExecProcessesCell {
    fn rich_selection_text(lines: &[Line<'_>]) -> String {
        lines
            .iter()
            .enumerate()
            .map(|(index, line)| {
                let rendered = rendered_line_text(line);
                if index == 0 {
                    return rendered;
                }
                for prefix in ["    ↳ ", "      ", "  • "] {
                    if let Some(content) = rendered.strip_prefix(prefix) {
                        return content.to_string();
                    }
                }
                rendered
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn new(processes: Vec<UnifiedExecProcessDetails>) -> Self {
        Self { processes }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct UnifiedExecProcessDetails {
    pub(crate) command_display: String,
    pub(crate) recent_chunks: Vec<String>,
}

impl HistoryCell for UnifiedExecProcessesCell {
    fn display_lines(&self, width: u16) -> Vec<Line<'static>> {
        if width == 0 {
            return Vec::new();
        }

        let wrap_width = width as usize;
        let max_processes = 16usize;
        let mut out: Vec<Line<'static>> = Vec::new();
        out.push(vec!["Background terminals".bold()].into());
        out.push("".into());

        if self.processes.is_empty() {
            out.push("  • No background terminals running.".italic().into());
            return out;
        }

        let prefix = "  • ";
        let prefix_width = UnicodeWidthStr::width(prefix);
        let truncation_suffix = " [...]";
        let truncation_suffix_width = UnicodeWidthStr::width(truncation_suffix);
        let mut shown = 0usize;
        for process in &self.processes {
            if shown >= max_processes {
                break;
            }
            let command = &process.command_display;
            let (snippet, snippet_truncated) = {
                let (first_line, has_more_lines) = match command.split_once('\n') {
                    Some((first, _)) => (first, true),
                    None => (command.as_str(), false),
                };
                let max_graphemes = 80;
                let mut graphemes = first_line.grapheme_indices(true);
                if let Some((byte_index, _)) = graphemes.nth(max_graphemes) {
                    (first_line[..byte_index].to_string(), true)
                } else {
                    (first_line.to_string(), has_more_lines)
                }
            };
            if wrap_width <= prefix_width {
                out.push(Line::from(prefix.dim()));
                shown += 1;
                continue;
            }
            let budget = wrap_width.saturating_sub(prefix_width);
            let mut needs_suffix = snippet_truncated;
            if !needs_suffix {
                let (_, remainder, _) = take_prefix_by_width(&snippet, budget);
                if !remainder.is_empty() {
                    needs_suffix = true;
                }
            }
            if needs_suffix && budget > truncation_suffix_width {
                let available = budget.saturating_sub(truncation_suffix_width);
                let (truncated, _, _) = take_prefix_by_width(&snippet, available);
                out.push(vec![prefix.dim(), truncated.cyan(), truncation_suffix.dim()].into());
            } else {
                let (truncated, _, _) = take_prefix_by_width(&snippet, budget);
                out.push(vec![prefix.dim(), truncated.cyan()].into());
            }

            let chunk_prefix_first = "    ↳ ";
            let chunk_prefix_next = "      ";
            for (idx, chunk) in process.recent_chunks.iter().enumerate() {
                let chunk_prefix = if idx == 0 {
                    chunk_prefix_first
                } else {
                    chunk_prefix_next
                };
                let chunk_prefix_width = UnicodeWidthStr::width(chunk_prefix);
                if wrap_width <= chunk_prefix_width {
                    out.push(Line::from(chunk_prefix.dim()));
                    continue;
                }
                let budget = wrap_width.saturating_sub(chunk_prefix_width);
                let (truncated, remainder, _) = take_prefix_by_width(chunk, budget);
                if !remainder.is_empty() && budget > truncation_suffix_width {
                    let available = budget.saturating_sub(truncation_suffix_width);
                    let (shorter, _, _) = take_prefix_by_width(chunk, available);
                    out.push(
                        vec![chunk_prefix.dim(), shorter.dim(), truncation_suffix.dim()].into(),
                    );
                } else {
                    out.push(vec![chunk_prefix.dim(), truncated.dim()].into());
                }
            }
            shown += 1;
        }

        let remaining = self.processes.len().saturating_sub(shown);
        if remaining > 0 {
            let more_text = format!("... and {remaining} more running");
            if wrap_width <= prefix_width {
                out.push(Line::from(prefix.dim()));
            } else {
                let budget = wrap_width.saturating_sub(prefix_width);
                let (truncated, _, _) = take_prefix_by_width(&more_text, budget);
                out.push(vec![prefix.dim(), truncated.dim()].into());
            }
        }

        out
    }

    fn raw_lines(&self) -> Vec<Line<'static>> {
        plain_lines(self.display_lines(u16::MAX))
    }

    fn selection_contribution(&self, width: u16, mode: HistoryRenderMode) -> SelectionContribution {
        match mode {
            HistoryRenderMode::Raw => {
                selection_contribution_from_display_lines(self.raw_lines(), width)
            }
            HistoryRenderMode::Rich => {
                let lines = self.display_lines(width);
                let prefix_columns = lines
                    .iter()
                    .map(|line| selection_prefix_width(line, &["    ↳ ", "      ", "  • "]))
                    .collect::<Vec<_>>();
                selection_contribution_from_semantic_rows(
                    Self::rich_selection_text(&lines),
                    lines,
                    width,
                    &prefix_columns,
                )
            }
        }
    }

    fn desired_height(&self, width: u16) -> u16 {
        self.display_lines(width).len() as u16
    }
}

pub(crate) fn new_unified_exec_processes_output(
    processes: Vec<UnifiedExecProcessDetails>,
) -> CompositeHistoryCell {
    let command = PlainHistoryCell::new(vec!["/ps".magenta().into()]);
    let summary = UnifiedExecProcessesCell::new(processes);
    CompositeHistoryCell::new(vec![Box::new(command), Box::new(summary)])
}
