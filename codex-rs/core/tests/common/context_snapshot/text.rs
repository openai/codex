//! Layout and clipping for text already normalized by the context snapshot normalizer.

use super::ContextSnapshotOptions;
use super::MAX_SNAPSHOT_LINE_CHARS;
use super::normalize::Normalizer;
use super::normalize::TextSource;
use super::normalize::fingerprint;
use serde_json::Value;

const RETAINED_SEGMENT_EDGE_LINES: usize = 8;

pub(super) fn render_text(
    text: &str,
    source: TextSource<'_>,
    options: &ContextSnapshotOptions,
    normalizer: &mut Normalizer,
) -> String {
    if text.is_empty() && matches!(source, TextSource::ModelInstructions) {
        return "\"\"".to_string();
    }
    let normalized = normalizer.text(text, source, options);
    let mut lines = normalized.split('\n').collect::<Vec<_>>();
    // Trim only when the omitted middle is larger than one retained edge.
    // The hidden middle has a fingerprint; visible edits change only their own lines.
    let marker = (lines.len() > RETAINED_SEGMENT_EDGE_LINES * 3).then(|| {
        let omitted =
            &lines[RETAINED_SEGMENT_EDGE_LINES..lines.len() - RETAINED_SEGMENT_EDGE_LINES];
        let omitted_text = omitted.join("\n");
        format!(
            "<OMITTED {} LINES; ~{} TOKENS; hash={}>",
            omitted.len(),
            omitted_text.chars().count() / 4,
            fingerprint(&Value::String(omitted_text))
        )
    });
    if let Some(marker) = &marker {
        lines.splice(
            RETAINED_SEGMENT_EDGE_LINES..lines.len() - RETAINED_SEGMENT_EDGE_LINES,
            [marker.as_str()],
        );
    }
    lines
        .into_iter()
        .map(|line| {
            let line = line.trim_end();
            let len = line.chars().count();
            let line = if len > MAX_SNAPSHOT_LINE_CHARS {
                let head = (MAX_SNAPSHOT_LINE_CHARS - 3) * 2 / 3;
                let tail = MAX_SNAPSHOT_LINE_CHARS - 3 - head;
                format!(
                    "{}...{} [hash={}]",
                    line.chars().take(head).collect::<String>(),
                    line.chars().skip(len - tail).collect::<String>(),
                    fingerprint(&Value::String(line.to_string()))
                )
            } else {
                line.to_string()
            };
            line.replace('\\', "\\\\")
        })
        .collect::<Vec<_>>()
        .join("\n")
}
