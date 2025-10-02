use std::time::Instant;

use super::model::CommandOutput;
use super::model::ExecCall;
use super::model::ExecCell;
use crate::exec_command::strip_bash_lc_and_escape;
use crate::history_cell::HistoryCell;
use crate::render::highlight::highlight_bash_to_lines;
use crate::render::line_utils::prefix_lines;
use crate::render::line_utils::push_owned_lines;
use crate::wrapping::RtOptions;
use crate::wrapping::word_wrap_line;
use codex_ansi_escape::ansi_escape_line;
use codex_common::elapsed::format_duration;
use codex_protocol::parse_command::ParsedCommand;
use itertools::Itertools;
use ratatui::prelude::*;
use ratatui::style::Modifier;
use ratatui::style::Stylize;
use ratatui::widgets::Paragraph;
use ratatui::widgets::WidgetRef;
use ratatui::widgets::Wrap;
use textwrap::WordSplitter;
use unicode_width::UnicodeWidthStr;

pub(crate) const TOOL_CALL_MAX_LINES: usize = 5;

pub(crate) struct OutputLinesParams {
    pub(crate) only_err: bool,
    pub(crate) include_angle_pipe: bool,
    pub(crate) include_prefix: bool,
}

pub(crate) fn new_active_exec_command(
    call_id: String,
    command: Vec<String>,
    parsed: Vec<ParsedCommand>,
) -> ExecCell {
    ExecCell::new(ExecCall {
        call_id,
        command,
        parsed,
        output: None,
        start_time: Some(Instant::now()),
        duration: None,
    })
}

pub(crate) fn output_lines(
    output: Option<&CommandOutput>,
    params: OutputLinesParams,
) -> Vec<Line<'static>> {
    let OutputLinesParams {
        only_err,
        include_angle_pipe,
        include_prefix,
    } = params;
    let CommandOutput {
        exit_code,
        stdout,
        stderr,
        ..
    } = match output {
        Some(output) if only_err && output.exit_code == 0 => return vec![],
        Some(output) => output,
        None => return vec![],
    };

    let src = if *exit_code == 0 { stdout } else { stderr };
    let lines: Vec<&str> = src.lines().collect();
    let total = lines.len();
    let limit = TOOL_CALL_MAX_LINES;

    let mut out = Vec::new();

    let head_end = total.min(limit);
    for (i, raw) in lines[..head_end].iter().enumerate() {
        let mut line = ansi_escape_line(raw);
        let prefix = if !include_prefix {
            ""
        } else if i == 0 && include_angle_pipe {
            "  └ "
        } else {
            "    "
        };
        line.spans.insert(0, prefix.into());
        line.spans.iter_mut().for_each(|span| {
            span.style = span.style.add_modifier(Modifier::DIM);
        });
        out.push(line);
    }

    let show_ellipsis = total > 2 * limit;
    if show_ellipsis {
        let omitted = total - 2 * limit;
        out.push(format!("… +{omitted} lines").into());
    }

    let tail_start = if show_ellipsis {
        total - limit
    } else {
        head_end
    };
    for raw in lines[tail_start..].iter() {
        let mut line = ansi_escape_line(raw);
        if include_prefix {
            line.spans.insert(0, "    ".into());
        }
        line.spans.iter_mut().for_each(|span| {
            span.style = span.style.add_modifier(Modifier::DIM);
        });
        out.push(line);
    }

    out
}

pub(crate) fn spinner(start_time: Option<Instant>) -> Span<'static> {
    const FRAMES: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
    let idx = start_time
        .map(|st| ((st.elapsed().as_millis() / 100) as usize) % FRAMES.len())
        .unwrap_or(0);
    let ch = FRAMES[idx];
    ch.to_string().into()
}

impl HistoryCell for ExecCell {
    fn display_lines(&self, width: u16) -> Vec<Line<'static>> {
        if self.is_exploring_cell() {
            self.exploring_display_lines(width)
        } else {
            self.command_display_lines(width)
        }
    }

    fn transcript_lines(&self) -> Vec<Line<'static>> {
        let mut lines: Vec<Line<'static>> = vec![];
        for call in self.iter_calls() {
            let cmd_display = strip_bash_lc_and_escape(&call.command);
            for (i, part) in cmd_display.lines().enumerate() {
                if i == 0 {
                    lines.push(vec!["$ ".magenta(), part.to_string().into()].into());
                } else {
                    lines.push(vec!["    ".into(), part.to_string().into()].into());
                }
            }

            if let Some(output) = call.output.as_ref() {
                lines.extend(output.formatted_output.lines().map(ansi_escape_line));
                let duration = call
                    .duration
                    .map(format_duration)
                    .unwrap_or_else(|| "unknown".to_string());
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
            lines.push("".into());
        }
        lines
    }
}

impl WidgetRef for &ExecCell {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 {
            return;
        }
        let content_area = Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: area.height,
        };
        let lines = self.display_lines(area.width);
        let max_rows = area.height as usize;
        let rendered = if lines.len() > max_rows {
            lines[lines.len() - max_rows..].to_vec()
        } else {
            lines
        };

        Paragraph::new(Text::from(rendered))
            .wrap(Wrap { trim: false })
            .render(content_area, buf);
    }
}

impl ExecCell {
    fn exploring_display_lines(&self, width: u16) -> Vec<Line<'static>> {
        let mut out: Vec<Line<'static>> = Vec::new();
        out.push(Line::from(vec![
            if self.is_active() {
                spinner(self.active_start_time())
            } else {
                "•".dim()
            },
            " ".into(),
            if self.is_active() {
                "Exploring".bold()
            } else {
                "Explored".bold()
            },
        ]));

        let mut calls = self.calls.clone();
        let mut out_indented = Vec::new();
        while !calls.is_empty() {
            let mut call = calls.remove(0);
            if call
                .parsed
                .iter()
                .all(|parsed| matches!(parsed, ParsedCommand::Read { .. }))
            {
                while let Some(next) = calls.first() {
                    if next
                        .parsed
                        .iter()
                        .all(|parsed| matches!(parsed, ParsedCommand::Read { .. }))
                    {
                        call.parsed.extend(next.parsed.clone());
                        calls.remove(0);
                    } else {
                        break;
                    }
                }
            }

            let reads_only = call
                .parsed
                .iter()
                .all(|parsed| matches!(parsed, ParsedCommand::Read { .. }));

            let call_lines: Vec<(&str, Vec<Span<'static>>)> = if reads_only {
                let names = call
                    .parsed
                    .iter()
                    .map(|parsed| match parsed {
                        ParsedCommand::Read { name, .. } => name.clone(),
                        _ => unreachable!(),
                    })
                    .unique();
                vec![(
                    "Read",
                    Itertools::intersperse(names.into_iter().map(Into::into), ", ".dim()).collect(),
                )]
            } else {
                let mut lines = Vec::new();
                for parsed in &call.parsed {
                    match parsed {
                        ParsedCommand::Read { name, .. } => {
                            lines.push(("Read", vec![name.clone().into()]));
                        }
                        ParsedCommand::ListFiles { cmd, path } => {
                            lines.push(("List", vec![path.clone().unwrap_or(cmd.clone()).into()]));
                        }
                        ParsedCommand::Search { cmd, query, path } => {
                            let spans = match (query, path) {
                                (Some(q), Some(p)) => {
                                    vec![q.clone().into(), " in ".dim(), p.clone().into()]
                                }
                                (Some(q), None) => vec![q.clone().into()],
                                _ => vec![cmd.clone().into()],
                            };
                            lines.push(("Search", spans));
                        }
                        ParsedCommand::Unknown { cmd } => {
                            lines.push(("Run", vec![cmd.clone().into()]));
                        }
                    }
                }
                lines
            };

            for (title, line) in call_lines {
                let line = Line::from(line);
                let initial_indent = Line::from(vec![title.cyan(), " ".into()]);
                let subsequent_indent = " ".repeat(initial_indent.width()).into();
                let wrapped = word_wrap_line(
                    &line,
                    RtOptions::new(width as usize)
                        .initial_indent(initial_indent)
                        .subsequent_indent(subsequent_indent),
                );
                push_owned_lines(&wrapped, &mut out_indented);
            }
        }

        out.extend(prefix_lines(out_indented, "  └ ".dim(), "    ".into()));
        out
    }

    fn command_display_lines(&self, width: u16) -> Vec<Line<'static>> {
        let [call] = &self.calls.as_slice() else {
            panic!("Expected exactly one call in a command display cell");
        };
        let layout = EXEC_DISPLAY_LAYOUT;
        let success = call.output.as_ref().map(|o| o.exit_code == 0);
        let bullet = match success {
            Some(true) => "•".green().bold(),
            Some(false) => "•".red().bold(),
            None => spinner(call.start_time),
        };
        let title = if self.is_active() { "Running" } else { "Ran" };

        let mut header_line =
            Line::from(vec![bullet.clone(), " ".into(), title.bold(), " ".into()]);
        let header_prefix_width = header_line.width();

        let cmd_display = strip_bash_lc_and_escape(&call.command);
        let highlighted_lines = highlight_bash_to_lines(&cmd_display);

        let continuation_wrap_width = layout.command_continuation.wrap_width(width);
        let continuation_opts =
            RtOptions::new(continuation_wrap_width).word_splitter(WordSplitter::NoHyphenation);

        let mut continuation_lines: Vec<Line<'static>> = Vec::new();

        if let Some((first, rest)) = highlighted_lines.split_first() {
            let available_first_width = (width as usize).saturating_sub(header_prefix_width).max(1);
            let first_opts =
                RtOptions::new(available_first_width).word_splitter(WordSplitter::NoHyphenation);
            let mut first_wrapped: Vec<Line<'static>> = Vec::new();
            push_owned_lines(&word_wrap_line(first, first_opts), &mut first_wrapped);
            let mut first_wrapped_iter = first_wrapped.into_iter();
            if let Some(first_segment) = first_wrapped_iter.next() {
                header_line.extend(first_segment);
            }
            continuation_lines.extend(first_wrapped_iter);

            for line in rest {
                push_owned_lines(
                    &word_wrap_line(line, continuation_opts.clone()),
                    &mut continuation_lines,
                );
            }
        }

        let mut lines: Vec<Line<'static>> = vec![header_line];

        let continuation_lines = Self::limit_lines_from_start(
            &continuation_lines,
            layout.command_continuation_max_lines,
        );
        if !continuation_lines.is_empty() {
            lines.extend(prefix_lines(
                continuation_lines,
                Span::from(layout.command_continuation.initial_prefix).dim(),
                Span::from(layout.command_continuation.subsequent_prefix).dim(),
            ));
        }

        if let Some(output) = call.output.as_ref() {
            let raw_output_lines = output_lines(
                Some(output),
                OutputLinesParams {
                    only_err: false,
                    include_angle_pipe: false,
                    include_prefix: false,
                },
            );

            if raw_output_lines.is_empty() {
                lines.extend(prefix_lines(
                    vec![Line::from("(no output)".dim())],
                    Span::from(layout.output_block.initial_prefix).dim(),
                    Span::from(layout.output_block.subsequent_prefix),
                ));
            } else {
                let trimmed_output =
                    Self::truncate_lines_middle(&raw_output_lines, layout.output_max_lines);

                let mut wrapped_output: Vec<Line<'static>> = Vec::new();
                let output_wrap_width = layout.output_block.wrap_width(width);
                let output_opts =
                    RtOptions::new(output_wrap_width).word_splitter(WordSplitter::NoHyphenation);
                for line in trimmed_output {
                    push_owned_lines(
                        &word_wrap_line(&line, output_opts.clone()),
                        &mut wrapped_output,
                    );
                }

                if !wrapped_output.is_empty() {
                    lines.extend(prefix_lines(
                        wrapped_output,
                        Span::from(layout.output_block.initial_prefix).dim(),
                        Span::from(layout.output_block.subsequent_prefix),
                    ));
                }
            }
        }

        lines
    }

    fn limit_lines_from_start(lines: &[Line<'static>], keep: usize) -> Vec<Line<'static>> {
        if lines.len() <= keep {
            return lines.to_vec();
        }
        if keep == 0 {
            return vec![Self::ellipsis_line(lines.len())];
        }

        let mut out: Vec<Line<'static>> = lines[..keep].to_vec();
        out.push(Self::ellipsis_line(lines.len() - keep));
        out
    }

    fn truncate_lines_middle(lines: &[Line<'static>], max: usize) -> Vec<Line<'static>> {
        if max == 0 {
            return Vec::new();
        }
        if lines.len() <= max {
            return lines.to_vec();
        }
        if max == 1 {
            return vec![Self::ellipsis_line(lines.len())];
        }

        let head = (max - 1) / 2;
        let tail = max - head - 1;
        let mut out: Vec<Line<'static>> = Vec::new();

        if head > 0 {
            out.extend(lines[..head].iter().cloned());
        }

        let omitted = lines.len().saturating_sub(head + tail);
        out.push(Self::ellipsis_line(omitted));

        if tail > 0 {
            out.extend(lines[lines.len() - tail..].iter().cloned());
        }

        out
    }

    fn ellipsis_line(omitted: usize) -> Line<'static> {
        Line::from(vec![format!("… +{omitted} lines").dim()])
    }
}

#[derive(Clone, Copy)]
struct PrefixedBlock {
    initial_prefix: &'static str,
    subsequent_prefix: &'static str,
}

impl PrefixedBlock {
    const fn new(initial_prefix: &'static str, subsequent_prefix: &'static str) -> Self {
        Self {
            initial_prefix,
            subsequent_prefix,
        }
    }

    fn wrap_width(self, total_width: u16) -> usize {
        let prefix_width = UnicodeWidthStr::width(self.initial_prefix)
            .max(UnicodeWidthStr::width(self.subsequent_prefix));
        usize::from(total_width).saturating_sub(prefix_width).max(1)
    }
}

#[derive(Clone, Copy)]
struct ExecDisplayLayout {
    command_continuation: PrefixedBlock,
    command_continuation_max_lines: usize,
    output_block: PrefixedBlock,
    output_max_lines: usize,
}

impl ExecDisplayLayout {
    const fn new(
        command_continuation: PrefixedBlock,
        command_continuation_max_lines: usize,
        output_block: PrefixedBlock,
        output_max_lines: usize,
    ) -> Self {
        Self {
            command_continuation,
            command_continuation_max_lines,
            output_block,
            output_max_lines,
        }
    }
}

const EXEC_DISPLAY_LAYOUT: ExecDisplayLayout = ExecDisplayLayout::new(
    PrefixedBlock::new("  │ ", "  │ "),
    2,
    PrefixedBlock::new("  └ ", "    "),
    5,
);
