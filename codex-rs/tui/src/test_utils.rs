use std::sync::mpsc::Receiver;

use crate::app_event::AppEvent;
use ratatui::text::Line as RtLine;

/// Convert a slice of ratatui `Line` into plain strings (concatenating spans).
pub(crate) fn lines_to_plain_strings(lines: &[RtLine<'_>]) -> Vec<String> {
    lines
        .iter()
        .map(|line| {
            line.spans
                .iter()
                .map(|sp| sp.content.clone())
                .collect::<Vec<_>>()
                .join("")
        })
        .collect()
}

/// Append rendered lines to a transcript buffer, one per line, with trailing newlines.
pub(crate) fn append_lines_to_transcript(lines: &[RtLine<'_>], out: &mut String) {
    for s in lines_to_plain_strings(lines) {
        out.push_str(&s);
        out.push('\n');
    }
}

/// Drain AppEvent::InsertHistory events and render them to the terminal/output writer.
pub(crate) fn drain_insert_history<B: ratatui::backend::Backend>(
    term: &mut crate::custom_terminal::Terminal<B>,
    rx: &Receiver<AppEvent>,
    out: &mut Vec<u8>,
) {
    while let Ok(ev) = rx.try_recv() {
        if let AppEvent::InsertHistory(lines) = ev {
            crate::insert_history::insert_history_lines_to_writer(term, out, lines);
        }
    }
}
