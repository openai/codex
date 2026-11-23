//! Resolve progress writes within one LF-delimited command output row before layout.
//! CR returns to column zero. Erasing the line discards the unwritten suffix.

use crate::width::display_width;
use ratatui::style::Style;
use regex_lite::Regex;
use std::borrow::Cow;
use std::sync::LazyLock;
use unicode_segmentation::UnicodeSegmentation;

// ansi-to-tui only recognizes BEL as an OSC terminator.
static OSC: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\x1b\][^\x07\x1b]*(?:\x07|\x1b\\|\x1b?$)")
        .unwrap_or_else(|err| panic!("invalid OSC regex: {err}"))
});
static CSI: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\x1b\[[0-?]*[ -/]*(?:[@-~]|$)")
        .unwrap_or_else(|err| panic!("invalid CSI regex: {err}"))
});

/// Plain text for /ps and historical command fallbacks.
pub(crate) fn plain_output_text(mut line: String) -> String {
    if line.contains('\x1b') {
        line = codex_ansi_escape::ansi_escape_line(&line).to_string();
    }
    line.truncate(line.trim_end().len());
    line
}

/// Keep an SGR only while it supplies an attribute that later SGRs leave alone.
/// Use the renderer's parser so compound codes and partial resets agree with layout.
fn style_prefixes(raw: &str, offsets: impl Iterator<Item = usize>) -> Vec<Vec<&str>> {
    let mut active: Vec<(&str, Style)> = Vec::new();
    let mut sample = String::new();
    let mut controls = CSI.find_iter(raw).peekable();
    offsets
        .map(|offset| {
            while let Some(control) = controls.next_if(|c| c.start() < offset) {
                let sgr = control.as_str();
                if !sgr.ends_with('m') || active.first().is_some_and(|(last, _)| *last == sgr) {
                    continue;
                }
                if matches!(sgr, "\x1b[m" | "\x1b[0m") {
                    active.clear();
                    continue;
                }
                // The parser attaches the style to the next printable character.
                sample.clear();
                sample.push_str(sgr);
                sample.push(' ');
                let text = codex_ansi_escape::ansi_escape(&sample);
                let Some(span) = text.lines.first().and_then(|line| line.spans.first()) else {
                    continue;
                };
                let mut newer = span.style;
                // Newest first. Each retained code contributes at least one attribute.
                active.retain(|(_, style)| {
                    let combined = style.patch(newer);
                    let needed = combined != newer;
                    newer = combined;
                    needed
                });
                active.insert(0, (sgr, span.style));
            }
            active.iter().rev().map(|(sgr, _)| *sgr).collect()
        })
        .collect()
}

pub(crate) fn command_output_line(raw: &str) -> Cow<'_, str> {
    let raw = raw.trim_end_matches('\r');
    let raw = if raw.contains("\x1b]") && (raw.contains('\r') || raw.contains("\x1b\\")) {
        OSC.replace_all(raw, "")
    } else {
        Cow::Borrowed(raw)
    };
    if !raw.contains('\r') && !raw.contains("\x1b[2K") {
        return raw;
    }

    // Later writes cover a prefix of every earlier write. Walk backwards so a
    // short repeated progress update never copies a long surviving tail twice.
    let mut covered = 0;
    let styled = raw.contains('\x1b');
    let mut result = String::new();
    let mut fragments = Vec::new();
    let mut end = raw.len();
    for write in raw.rsplit('\r') {
        let start_of_write = end - write.len();
        end = start_of_write.saturating_sub(1);
        let mut offset = start_of_write;
        let mut erased = false;
        let (column, suffix) = if write.is_ascii() && !write.contains(['\x1b', '\t']) {
            (
                write.len(),
                Cow::Borrowed(write.get(covered..).unwrap_or_default()),
            )
        } else {
            let mut column = 0;
            let mut start = 0;
            let mut suffix = String::new();
            let mut visible_end = 0;
            // A whole-line erase blanks earlier columns without moving the cursor.
            let clear = write.rfind("\x1b[2K");
            for control in CSI.find_iter(write).map(Some).chain([None]) {
                let end = control
                    .as_ref()
                    .map_or(write.len(), regex_lite::Match::start);
                for g in write[start..end].graphemes(true) {
                    if g == "\x1b" {
                        continue;
                    }
                    let width = if g == "\t" { 4 } else { display_width(g) };
                    if clear.is_some_and(|clear| start < clear) {
                        column += width;
                        continue;
                    }
                    if suffix.is_empty() {
                        offset = start_of_write + start;
                    }
                    if column >= covered {
                        suffix.push_str(g);
                    } else if column + width > covered {
                        // Overwriting half a wide glyph clears its remaining cells.
                        suffix.extend(std::iter::repeat_n(' ', column + width - covered));
                    }
                    column += width;
                    visible_end = suffix.len();
                }
                if let Some(control) = control {
                    match control.as_str() {
                        "\x1b[K" | "\x1b[0K" => erased = true,
                        "\x1b[2K" if Some(control.start()) == clear => {
                            erased = true;
                            offset = start_of_write + control.start();
                            suffix.extend(std::iter::repeat_n(' ', column.saturating_sub(covered)));
                            visible_end = suffix.len();
                        }
                        sgr if !suffix.is_empty() && sgr.ends_with('m') => suffix.push_str(sgr),
                        _ => {}
                    }
                    start = control.end();
                }
            }
            suffix.truncate(visible_end);
            (column, Cow::Owned(suffix))
        };
        if column > covered {
            if styled {
                fragments.push((offset, suffix));
            } else {
                result.push_str(&suffix);
            }
            covered = column;
        }
        if erased {
            break;
        }
    }

    if !styled {
        return Cow::Owned(result);
    }
    // Resolve styles in write order, then apply them in display order. Restore
    // the final cursor style because bounded previews reuse this row for later CRs.
    let offsets = fragments.iter().rev().map(|(offset, _)| *offset);
    let mut styles = style_prefixes(&raw, offsets.chain([raw.len()]));
    let cursor = styles.pop().unwrap_or_default();
    let texts = fragments.iter().map(|(_, text)| text.as_ref()).chain([""]);
    let mut current: Option<&[&str]> = Some(&[]);
    for (style, text) in styles.iter().rev().chain([&cursor]).zip(texts) {
        if current != Some(style.as_slice()) {
            if !matches!(current, Some([])) {
                result.push_str("\x1b[0m");
            }
            result.extend(style.iter().copied());
        }
        result.push_str(text);
        current = (!text.contains('\x1b')).then_some(style.as_slice());
    }
    Cow::Owned(result)
}
