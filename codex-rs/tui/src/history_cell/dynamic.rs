//! Dynamic tool activity with retained arguments and output across live and loaded transcripts.

use super::HistoryCell;
use super::activity_preview::DETAIL_PREVIEW_LINES;
use super::activity_preview::clipped_line;
use super::raw_lines_from_source;
use crate::style::accent_color;
use crate::terminal_hyperlinks::HyperlinkLine;
use crate::terminal_hyperlinks::adaptive_wrap_hyperlink_lines;
use crate::terminal_hyperlinks::plain_hyperlink_lines;
use crate::terminal_hyperlinks::visible_lines;
use crate::wrapping::RtOptions;
use codex_app_server_protocol::DynamicToolCallOutputContentItem;
use codex_app_server_protocol::DynamicToolCallStatus;
use codex_app_server_protocol::ThreadItem;
use ratatui::style::Stylize;
use ratatui::text::Line;
use serde_json::Value;
use std::sync::Arc;
use std::sync::RwLock;

#[derive(Clone, Debug)]
pub(crate) struct DynamicToolCallCell {
    #[allow(dead_code, reason = "Used by later layers of the TUI refresh stack.")]
    call_id: String,
    data: Arc<RwLock<DynamicToolCallData>>,
}

#[derive(Debug)]
struct DynamicToolCallData {
    name: String,
    arguments: Value,
    status: DynamicToolCallStatus,
    interrupted: bool,
    output: Option<Vec<String>>,
    duration_ms: Option<i64>,
}

impl DynamicToolCallCell {
    pub(crate) fn from_item(item: ThreadItem) -> Option<Self> {
        let (call_id, data) = DynamicToolCallData::from_item(item)?;
        Some(Self {
            call_id,
            data: Arc::new(RwLock::new(data)),
        })
    }

    #[allow(dead_code, reason = "Used by later layers of the TUI refresh stack.")]
    pub(crate) fn call_id(&self) -> &str {
        &self.call_id
    }

    #[allow(dead_code, reason = "Used by later layers of the TUI refresh stack.")]
    pub(crate) fn is_active(&self) -> bool {
        self.data
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_active()
    }

    /// Updates the original retained row when concurrent calls complete in a different order.
    /// Callers revising an already-terminal cell must rebuild its cached renderables.
    #[allow(dead_code, reason = "Used by later layers of the TUI refresh stack.")]
    pub(crate) fn update_from_item(&self, item: ThreadItem) -> bool {
        let Some((call_id, data)) = DynamicToolCallData::from_item(item) else {
            return false;
        };
        if call_id != self.call_id {
            return false;
        }
        *self
            .data
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = data;
        true
    }

    #[allow(dead_code, reason = "Used by later layers of the TUI refresh stack.")]
    pub(crate) fn mark_interrupted(&self) {
        let mut data = self
            .data
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if !data.is_active() {
            return;
        }
        data.status = DynamicToolCallStatus::Failed;
        data.interrupted = true;
        data.output
            .get_or_insert_with(Vec::new)
            .push("Interrupted before this tool returned a result.".to_owned());
    }
}

impl DynamicToolCallData {
    fn from_item(item: ThreadItem) -> Option<(String, Self)> {
        let ThreadItem::DynamicToolCall {
            id,
            namespace,
            tool,
            arguments,
            status,
            content_items,
            success,
            duration_ms,
        } = item
        else {
            return None;
        };
        Some((
            id,
            Self {
                name: namespace
                    .map_or_else(|| tool.clone(), |namespace| format!("{namespace}.{tool}")),
                arguments,
                status: if success == Some(false) {
                    DynamicToolCallStatus::Failed
                } else {
                    status
                },
                interrupted: false,
                output: content_items.map(|items| {
                    items
                        .into_iter()
                        .map(|item| match item {
                            DynamicToolCallOutputContentItem::InputText { text } => text,
                            DynamicToolCallOutputContentItem::InputImage { .. } => {
                                "<image content>".to_owned()
                            }
                            DynamicToolCallOutputContentItem::InputAudio { .. } => {
                                "<audio content>".to_owned()
                            }
                        })
                        .collect()
                }),
                duration_ms,
            },
        ))
    }

    #[allow(dead_code, reason = "Used by later layers of the TUI refresh stack.")]
    fn is_active(&self) -> bool {
        matches!(self.status, DynamicToolCallStatus::InProgress)
    }

    fn header(&self) -> Line<'static> {
        let (marker, verb) = if self.interrupted {
            ("•".red().bold(), "Interrupted")
        } else {
            match self.status {
                DynamicToolCallStatus::InProgress => ("•".dim(), "Calling"),
                DynamicToolCallStatus::Completed => ("•".green(), "Called"),
                DynamicToolCallStatus::Failed => ("•".red().bold(), "Failed"),
            }
        };
        let mut line = Line::from(vec![
            marker,
            " ".into(),
            verb.bold(),
            " ".into(),
            self.name.clone().fg(accent_color()),
        ]);
        if let Some(duration_ms) = self.duration_ms.filter(|duration| *duration >= 0) {
            line.push_span(format!(" · {duration_ms}ms").dim());
        }
        line
    }

    fn output_lines(&self) -> Vec<Line<'static>> {
        match &self.output {
            Some(output) if output.is_empty() => vec![Line::from("(no output)".dim())],
            Some(output) => output
                .iter()
                .flat_map(|text| raw_lines_from_source(text))
                .collect(),
            None if self.is_active() => Vec::new(),
            None => vec![Line::from("(output unavailable)".dim())],
        }
    }
}

impl HistoryCell for DynamicToolCallCell {
    fn has_stable_transcript_height(&self) -> bool {
        !self.is_active()
    }

    fn activity_ids(&self) -> Vec<String> {
        vec![format!("dynamic:{}", self.call_id)]
    }

    fn display_lines(&self, width: u16) -> Vec<Line<'static>> {
        visible_lines(self.compact_hyperlink_lines(width))
    }

    fn compact_hyperlink_lines(&self, width: u16) -> Vec<HyperlinkLine> {
        let data = self
            .data
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut lines = vec![clipped_line(data.header(), width)];
        let (skip, output) = match &data.output {
            Some(output) if !output.is_empty() => {
                let output = output.iter().flat_map(|text| text.split_terminator('\n'));
                let skip = output.clone().count().saturating_sub(DETAIL_PREVIEW_LINES);
                let tail = output
                    .skip(skip)
                    .map(|text| {
                        // Bound allocation and grapheme scanning before clipping to display width.
                        let end = text.floor_char_boundary(16 * 1024);
                        let mut text_preview = text[..end].to_owned();
                        if end < text.len() {
                            text_preview.push('…');
                        }
                        Line::from(text_preview)
                    })
                    .collect();
                (skip, tail)
            }
            _ => (0, data.output_lines()),
        };
        if skip > 0 {
            lines.push(clipped_line(
                format!("  … {skip} earlier lines hidden").dim().into(),
                width,
            ));
        }
        for (index, mut line) in output.into_iter().enumerate() {
            line.spans.insert(
                /*index*/ 0,
                if index == 0 { "  └ " } else { "    " }.dim(),
            );
            lines.push(clipped_line(line.dim(), width));
        }
        lines
    }

    fn transcript_hyperlink_lines(&self, width: u16) -> Vec<HyperlinkLine> {
        let data = self
            .data
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut lines = vec![data.header().into()];
        let args = serde_json::to_string_pretty(&data.arguments)
            .unwrap_or_else(|_| data.arguments.to_string());
        let details = raw_lines_from_source(&format!("Arguments: {args}"))
            .into_iter()
            .chain(data.output_lines())
            .collect();
        lines.extend(adaptive_wrap_hyperlink_lines(
            &plain_hyperlink_lines(details),
            RtOptions::new(usize::from(width).max(/*other*/ 1))
                .initial_indent("  └ ".dim().into())
                .subsequent_indent("    ".into()),
        ));
        lines
    }

    fn raw_lines(&self) -> Vec<Line<'static>> {
        let data = self
            .data
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut lines = vec![data.header()];
        lines.extend(raw_lines_from_source(&format!(
            "Arguments: {}",
            data.arguments
        )));
        lines.extend(data.output_lines());
        super::plain_lines(lines)
    }
}

#[cfg(test)]
#[path = "dynamic_tests.rs"]
mod tests;
