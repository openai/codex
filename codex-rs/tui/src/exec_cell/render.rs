use std::time::Instant;

use super::model::CommandOutput;
use super::model::ExecCall;
use super::model::ExecCell;
use super::model::SubagentCell;
use super::model::parse_subagent_call;
use super::model::subagent_from_formatted_output;
use crate::exec_command::strip_bash_lc_and_escape;
use crate::history_cell::HistoryCell;
use crate::render::highlight::highlight_bash_to_lines;
use crate::render::line_utils::prefix_lines;
use crate::render::line_utils::push_owned_lines;
use crate::shimmer::shimmer_spans;
use crate::wrapping::RtOptions;
use crate::wrapping::word_wrap_line;
use crate::wrapping::word_wrap_lines;
use codex_ansi_escape::ansi_escape_line;
use codex_common::elapsed::format_duration;
use codex_core::protocol::ExecCommandSource;
use codex_protocol::parse_command::ParsedCommand;
use itertools::Itertools;
use ratatui::prelude::*;
use ratatui::style::Modifier;
use ratatui::style::Stylize;
use textwrap::WordSplitter;
use unicode_width::UnicodeWidthStr;

pub(crate) const TOOL_CALL_MAX_LINES: usize = 5;
const USER_SHELL_TOOL_CALL_MAX_LINES: usize = 50;
const MAX_INTERACTION_PREVIEW_CHARS: usize = 80;

fn header_parts(cell: &SubagentCell) -> Option<(String, Option<String>)> {
    match cell {
        SubagentCell::Fork { label, .. } => Some(("Forked subagent".into(), Some(label.clone()))),
        SubagentCell::Spawn { label, .. } => Some(("Spawned subagent".into(), Some(label.clone()))),
        SubagentCell::SendMessage { label, .. } => {
            Some(("Sent message to subagent".into(), Some(label.clone())))
        }
        SubagentCell::Await { label, .. } => Some(("Awaited subagent".into(), label.clone())),
        SubagentCell::Prune { .. } => Some(("Pruned subagents".into(), None)),
        SubagentCell::Logs { label, .. } => Some((
            "Fetched subagent logs".into(),
            label.as_ref().map(|l| format!("from {l}")),
        )),
        SubagentCell::List { .. } => Some(("Listed subagents".into(), None)),
        SubagentCell::Watchdog { action, .. } => {
            Some(("Watchdog".into(), action.as_ref().map(|a| a.to_string())))
        }
        SubagentCell::Cancel { label } => Some(("Canceled subagent".into(), label.clone())),
        SubagentCell::Raw { .. } => None,
    }
}

fn subagent_header_parts(command: &[String]) -> Option<(String, Option<String>)> {
    parse_subagent_call(command).as_ref().and_then(header_parts)
}

pub(crate) struct OutputLinesParams {
    pub(crate) line_limit: usize,
    pub(crate) only_err: bool,
    pub(crate) include_angle_pipe: bool,
    pub(crate) include_prefix: bool,
}

pub(crate) fn new_active_exec_command(
    call_id: String,
    command: Vec<String>,
    parsed: Vec<ParsedCommand>,
    subagent: Option<SubagentCell>,
    source: ExecCommandSource,
    interaction_input: Option<String>,
    animations_enabled: bool,
    is_user_shell_command: bool,
) -> ExecCell {
    let subagent = subagent.or_else(|| parse_subagent_call(&command));
    ExecCell::new(
        ExecCall {
            call_id,
            command,
            parsed,
            subagent,
            output: None,
            source,
            is_user_shell_command,
            start_time: Some(Instant::now()),
            duration: None,
            interaction_input,
        },
        animations_enabled,
    )
}

fn format_unified_exec_interaction(command: &[String], input: Option<&str>) -> String {
    let command_display = command.join(" ");
    match input {
        Some(data) if !data.is_empty() => {
            let preview = summarize_interaction_input(data);
            format!("Interacted with `{command_display}`, sent `{preview}`")
        }
        _ => format!("Waited for `{command_display}`"),
    }
}

fn summarize_interaction_input(input: &str) -> String {
    let single_line = input.replace('\n', "\\n");
    let sanitized = single_line.replace('`', "\\`");
    if sanitized.chars().count() <= MAX_INTERACTION_PREVIEW_CHARS {
        return sanitized;
    }

    let mut preview = String::new();
    for ch in sanitized.chars().take(MAX_INTERACTION_PREVIEW_CHARS) {
        preview.push(ch);
    }
    preview.push_str("...");
    preview
}

#[derive(Clone)]
pub(crate) struct OutputLines {
    pub(crate) lines: Vec<Line<'static>>,
    pub(crate) omitted: Option<usize>,
}

pub(crate) fn output_lines(
    output: Option<&CommandOutput>,
    params: OutputLinesParams,
) -> OutputLines {
    let OutputLinesParams {
        line_limit,
        only_err,
        include_angle_pipe,
        include_prefix,
    } = params;
    let CommandOutput {
        aggregated_output, ..
    } = match output {
        Some(output) if only_err && output.exit_code == 0 => {
            return OutputLines {
                lines: Vec::new(),
                omitted: None,
            };
        }
        Some(output) => output,
        None => {
            return OutputLines {
                lines: Vec::new(),
                omitted: None,
            };
        }
    };

    let src = aggregated_output;
    let lines: Vec<&str> = src.lines().collect();
    let total = lines.len();
    let mut out: Vec<Line<'static>> = Vec::new();

    let head_end = total.min(line_limit);
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

    let show_ellipsis = total > 2 * line_limit;
    let omitted = if show_ellipsis {
        Some(total - 2 * line_limit)
    } else {
        None
    };
    if show_ellipsis {
        let omitted = total - 2 * line_limit;
        out.push(format!("… +{omitted} lines").into());
    }

    let tail_start = if show_ellipsis {
        total - line_limit
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

    OutputLines {
        lines: out,
        omitted,
    }
}

pub(crate) fn spinner(start_time: Option<Instant>, animations_enabled: bool) -> Span<'static> {
    if !animations_enabled {
        return "•".dim();
    }
    let elapsed = start_time.map(|st| st.elapsed()).unwrap_or_default();
    if supports_color::on_cached(supports_color::Stream::Stdout)
        .map(|level| level.has_16m)
        .unwrap_or(false)
    {
        shimmer_spans("•")[0].clone()
    } else {
        let blink_on = (elapsed.as_millis() / 600).is_multiple_of(2);
        if blink_on { "•".into() } else { "◦".dim() }
    }
}

impl HistoryCell for ExecCell {
    fn display_lines(&self, width: u16) -> Vec<Line<'static>> {
        if self.is_exploring_cell() {
            self.exploring_display_lines(width)
        } else if self.is_subagent_cell() {
            self.subagent_display_lines(width)
        } else {
            self.command_display_lines(width)
        }
    }

    fn desired_transcript_height(&self, width: u16) -> u16 {
        self.transcript_lines(width).len() as u16
    }

    fn transcript_lines(&self, width: u16) -> Vec<Line<'static>> {
        let mut lines: Vec<Line<'static>> = vec![];
        for (i, call) in self.iter_calls().enumerate() {
            if i > 0 {
                lines.push("".into());
            }
            let script = strip_bash_lc_and_escape(&call.command);
            let highlighted_script = highlight_bash_to_lines(&script);
            let cmd_display = word_wrap_lines(
                &highlighted_script,
                RtOptions::new(width as usize)
                    .initial_indent("$ ".magenta().into())
                    .subsequent_indent("    ".into()),
            );
            lines.extend(cmd_display);

            if let Some(output) = call.output.as_ref() {
                if !call.is_unified_exec_interaction() {
                    lines.extend(output.formatted_output.lines().map(ansi_escape_line));
                }
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
        }
        lines
    }
}

impl ExecCell {
    fn exploring_display_lines(&self, width: u16) -> Vec<Line<'static>> {
        let mut out: Vec<Line<'static>> = Vec::new();
        out.push(Line::from(vec![
            if self.is_active() {
                spinner(self.active_start_time(), self.animations_enabled())
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

    fn subagent_display_lines(&self, _width: u16) -> Vec<Line<'static>> {
        let [call] = &self.calls.as_slice() else {
            panic!("Expected exactly one call in a subagent display cell");
        };
        let bullet = if let Some(output) = &call.output {
            if output.exit_code == 0 {
                "•".green().bold()
            } else {
                "•".red().bold()
            }
        } else {
            spinner(call.start_time, self.animations_enabled())
        };

        let render = call
            .output
            .as_ref()
            .and_then(|o| subagent_from_formatted_output(&o.formatted_output))
            .or_else(|| call.subagent.clone())
            .or_else(|| {
                subagent_header_parts(&call.command).map(|(verb, rest)| SubagentCell::Raw {
                    text: format!(
                        "{}{}",
                        verb,
                        rest.map(|r| format!(" {r}")).unwrap_or_default()
                    ),
                })
            });

        let Some((verb, rest)) = render.as_ref().and_then(header_parts) else {
            return vec![Line::from(vec![bullet, " ".into(), "Subagent".bold()])];
        };

        let mut header: Vec<Span<'static>> = vec![bullet, " ".into(), verb.bold()];
        if let Some(rest) = rest
            && !rest.is_empty()
        {
            header.push(" ".into());
            header.push(rest.into());
        }

        let mut lines = vec![Line::from(header)];

        if let Some(render) = render {
            let mut body_lines: Vec<Line<'static>> = Vec::new();
            match render {
                SubagentCell::Spawn { model, summary, .. }
                | SubagentCell::Fork { model, summary, .. } => {
                    if let Some(m) = model {
                        body_lines.push(Line::from(vec![format!("model={m}").into()]));
                    }
                    if let Some(s) = summary {
                        body_lines.push(Line::from(vec![format!("\"{s}\"").into()]));
                    }
                }
                SubagentCell::SendMessage { summary, .. } => {
                    if let Some(s) = summary {
                        body_lines.push(Line::from(vec![format!("\"{s}\"").into()]));
                    }
                }
                SubagentCell::List { count } => {
                    if let Some(count) = count {
                        body_lines
                            .push(Line::from(vec![format!("listed {count} subagents").into()]));
                    }
                }
                SubagentCell::Await {
                    timed_out,
                    message,
                    lifecycle_status,
                    ..
                } => {
                    if timed_out.unwrap_or(false) {
                        body_lines.push(Line::from("Timed out"));
                    } else if let Some(msg) = message {
                        body_lines.push(Line::from(msg));
                    } else if let Some(status) = lifecycle_status {
                        body_lines.push(Line::from(vec![format!("status={status}").into()]));
                    }
                }
                SubagentCell::Cancel { .. } => {}
                SubagentCell::Prune { counts } => {
                    if let Some(counts) = counts {
                        body_lines.push(Line::from(vec![
                            format!("pruned subagents {counts}").into(),
                        ]));
                    }
                }
                SubagentCell::Logs { rendered, .. } => {
                    if let Some(rendered) = rendered {
                        for l in rendered.lines() {
                            body_lines.push(Line::from(l.to_string()));
                        }
                    }
                }
                SubagentCell::Watchdog {
                    action,
                    interval_s,
                    message,
                } => {
                    if let Some(a) = action {
                        body_lines.push(Line::from(vec![format!("action={a}").into()]));
                    }
                    if let Some(i) = interval_s {
                        body_lines.push(Line::from(vec![format!("interval={i}s").into()]));
                    }
                    if let Some(msg) = message {
                        body_lines.push(Line::from(vec![format!("\"{msg}\"").into()]));
                    }
                }
                SubagentCell::Raw { text } => {
                    body_lines.push(Line::from(text));
                }
            }

            if !body_lines.is_empty() {
                lines.extend(prefix_lines(body_lines, "  └ ".dim(), "    ".into()));
            }
        } else if let Some(output) = call.output.as_ref() {
            let OutputLines {
                lines: out_lines, ..
            } = output_lines(
                Some(output),
                OutputLinesParams {
                    line_limit: USER_SHELL_TOOL_CALL_MAX_LINES,
                    only_err: false,
                    include_angle_pipe: false,
                    include_prefix: false,
                },
            );
            let prefixed = prefix_lines(out_lines, "  └ ".dim(), "    ".into());
            lines.extend(prefixed);
        }

        lines
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
            None => spinner(call.start_time, self.animations_enabled()),
        };
        let is_interaction = call.is_unified_exec_interaction();
        let title = if is_interaction {
            ""
        } else if self.is_active() {
            "Running"
        } else if call.is_user_shell_command() {
            "You ran"
        } else {
            "Ran"
        };

        let mut header_line = if is_interaction {
            Line::from(vec![bullet.clone(), " ".into()])
        } else {
            Line::from(vec![bullet.clone(), " ".into(), title.bold(), " ".into()])
        };
        let header_prefix_width = header_line.width();

        let cmd_display = if call.is_unified_exec_interaction() {
            format_unified_exec_interaction(&call.command, call.interaction_input.as_deref())
        } else {
            strip_bash_lc_and_escape(&call.command)
        };
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
            let line_limit = if call.is_user_shell_command() {
                USER_SHELL_TOOL_CALL_MAX_LINES
            } else {
                TOOL_CALL_MAX_LINES
            };
            let raw_output = output_lines(
                Some(output),
                OutputLinesParams {
                    line_limit,
                    only_err: false,
                    include_angle_pipe: false,
                    include_prefix: false,
                },
            );
            let display_limit = if call.is_user_shell_command() {
                USER_SHELL_TOOL_CALL_MAX_LINES
            } else {
                layout.output_max_lines
            };

            if raw_output.lines.is_empty() {
                if !call.is_unified_exec_interaction() {
                    lines.extend(prefix_lines(
                        vec![Line::from("(no output)".dim())],
                        Span::from(layout.output_block.initial_prefix).dim(),
                        Span::from(layout.output_block.subsequent_prefix),
                    ));
                }
            } else {
                let trimmed_output = Self::truncate_lines_middle(
                    &raw_output.lines,
                    display_limit,
                    raw_output.omitted,
                );

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

    fn truncate_lines_middle(
        lines: &[Line<'static>],
        max: usize,
        omitted_hint: Option<usize>,
    ) -> Vec<Line<'static>> {
        if max == 0 {
            return Vec::new();
        }
        if lines.len() <= max {
            return lines.to_vec();
        }
        if max == 1 {
            // Carry forward any previously omitted count and add any
            // additionally hidden content lines from this truncation.
            let base = omitted_hint.unwrap_or(0);
            // When an existing ellipsis is present, `lines` already includes
            // that single representation line; exclude it from the count of
            // additionally omitted content lines.
            let extra = lines
                .len()
                .saturating_sub(usize::from(omitted_hint.is_some()));
            let omitted = base + extra;
            return vec![Self::ellipsis_line(omitted)];
        }

        let head = (max - 1) / 2;
        let tail = max - head - 1;
        let mut out: Vec<Line<'static>> = Vec::new();

        if head > 0 {
            out.extend(lines[..head].iter().cloned());
        }

        let base = omitted_hint.unwrap_or(0);
        let additional = lines
            .len()
            .saturating_sub(head + tail)
            .saturating_sub(usize::from(omitted_hint.is_some()));
        out.push(Self::ellipsis_line(base + additional));

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
