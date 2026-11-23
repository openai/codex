use super::command_output_line;
use itertools::Either;
use std::borrow::Cow;
use std::collections::VecDeque;

const LIVE_COMMAND_OUTPUT_MAX_BYTES: usize = 1024 * 1024;
const LIVE_COMMAND_OUTPUT_MAX_LINES: usize = 50;
const LIVE_COMMAND_OUTPUT_MAX_LINE_BYTES: usize =
    LIVE_COMMAND_OUTPUT_MAX_BYTES / (2 * LIVE_COMMAND_OUTPUT_MAX_LINES + 2);
const LIVE_COMMAND_OUTPUT_LINE_HEAD_BYTES: usize = LIVE_COMMAND_OUTPUT_MAX_LINE_BYTES / 2;
const LIVE_COMMAND_OUTPUT_LINE_TAIL_BYTES: usize =
    LIVE_COMMAND_OUTPUT_MAX_LINE_BYTES - LIVE_COMMAND_OUTPUT_LINE_HEAD_BYTES;

/// A bounded, incremental preview of streamed command output.
///
/// All output is retained until the byte budget is exceeded. Once truncated, the first and last
/// 50 completed lines and any in-progress line are retained, and each line independently keeps a
/// prefix and suffix so commands that never emit a newline cannot grow the active cell without bound.
#[derive(Debug, Default)]
pub(super) struct LiveCommandOutput {
    full_output: String,
    truncated: bool,
    head: Vec<String>,
    tail: VecDeque<String>,
    current: PendingLine,
    completed_lines: usize,
    has_partial_line: bool,
}

impl LiveCommandOutput {
    /// Appends a delta, preserving `str::lines` semantics across arbitrary chunk boundaries.
    pub(super) fn push_str(&mut self, chunk: &str) {
        if !self.truncated {
            if self.full_output.len().saturating_add(chunk.len()) <= LIVE_COMMAND_OUTPUT_MAX_BYTES {
                self.full_output.push_str(chunk);
                self.completed_lines = self
                    .completed_lines
                    .saturating_add(chunk.bytes().filter(|byte| *byte == b'\n').count());
                if !chunk.is_empty() {
                    self.has_partial_line = !chunk.ends_with('\n');
                }
                return;
            }

            self.truncated = true;
            let full_output = std::mem::take(&mut self.full_output);
            self.completed_lines = 0;
            self.has_partial_line = false;
            self.push_truncated_str(&full_output);
        }

        self.push_truncated_str(chunk);
    }

    fn push_truncated_str(&mut self, chunk: &str) {
        if !chunk.is_empty() {
            self.has_partial_line = !chunk.ends_with('\n');
        }
        for line in self.current.push_lines(chunk) {
            self.completed_lines = self.completed_lines.saturating_add(1);
            if self.head.len() < LIVE_COMMAND_OUTPUT_MAX_LINES {
                self.head.push(line);
            } else {
                if self.tail.len() == LIVE_COMMAND_OUTPUT_MAX_LINES {
                    self.tail.pop_front();
                }
                self.tail.push_back(line);
            }
        }
    }

    pub(super) fn total_lines(&self) -> usize {
        self.completed_lines
            .saturating_add(usize::from(self.has_partial_line))
    }

    pub(super) fn retained_lines(&self) -> usize {
        if self.truncated {
            self.head
                .len()
                .saturating_add(self.tail.len())
                .saturating_add(usize::from(self.has_partial_line))
        } else {
            self.total_lines()
        }
    }

    /// Returns reverse-capable preview lines, abbreviating long lines even below the byte budget.
    pub(super) fn lines(&self) -> impl DoubleEndedIterator<Item = Cow<'_, str>> {
        if self.truncated {
            Either::Left(
                self.head
                    .iter()
                    .chain(self.tail.iter())
                    .map(|line| Cow::Borrowed(line.as_str()))
                    .chain(
                        self.has_partial_line
                            .then(|| Cow::Owned(self.current.clone().render())),
                    ),
            )
        } else {
            Either::Right(self.full_output.lines().map(|line| {
                let line = command_output_line(line);
                if line.len() <= LIVE_COMMAND_OUTPUT_MAX_LINE_BYTES {
                    line
                } else {
                    let mut truncated = LiveCommandOutputLine::default();
                    truncated.push_str(&line);
                    Cow::Owned(truncated.render())
                }
            }))
        }
    }

    /// Returns full visible transcript lines until truncated, then inserts the omitted-line marker.
    pub(super) fn transcript_lines(&self) -> impl Iterator<Item = Cow<'_, str>> {
        let omitted = self.total_lines().saturating_sub(self.retained_lines());
        if self.truncated {
            Either::Left(
                self.head
                    .iter()
                    .map(|line| Cow::Borrowed(line.as_str()))
                    .chain((omitted > 0).then(|| {
                        let noun = if omitted == 1 { "line" } else { "lines" };
                        Cow::Owned(format!("… +{omitted} {noun}"))
                    }))
                    .chain(self.tail.iter().map(|line| Cow::Borrowed(line.as_str())))
                    .chain(
                        self.has_partial_line
                            .then(|| Cow::Owned(self.current.clone().render())),
                    ),
            )
        } else {
            Either::Right(self.full_output.lines().map(command_output_line))
        }
    }
}

/// Bounded state for an unfinished row. Keep the write after the last CR
/// together across deltas, including any partial escape or grapheme.
#[derive(Clone, Debug, Default)]
pub(crate) struct PendingLine {
    previous: LiveCommandOutputLine,
    write: LiveCommandOutputLine,
}

impl PendingLine {
    /// Yield completed rows lazily and keep the unfinished row for the next delta.
    pub(crate) fn push_lines(&mut self, chunk: &str) -> impl DoubleEndedIterator<Item = String> {
        let mut parts = chunk.split('\n');
        let partial = parts.next_back().unwrap_or_default();
        let first = parts.next().map(|part| (std::mem::take(self), part));
        self.push_str(partial);
        first
            .into_iter()
            .chain(parts.map(|part| (Self::default(), part)))
            .map(|(mut row, part)| {
                row.push_str(part);
                row.render()
            })
    }

    fn push_str(&mut self, mut text: &str) {
        while !text.is_empty() {
            // Fold a batch of redraws together before abbreviating. A long
            // single write still uses the ordinary bounded head/tail buffer.
            let room = if self.previous.omitted_bytes > 0 && !self.previous.variable_width {
                // Overlay individual writes directly at their original byte offsets.
                0
            } else {
                LIVE_COMMAND_OUTPUT_MAX_LINE_BYTES
                    .saturating_sub(self.write.head.len() + self.write.tail.len())
            };
            let end = text.floor_char_boundary(room.min(text.len()));
            let Some(cr) = text[..end].rfind('\r').or_else(|| text.find('\r')) else {
                self.write.push_str(text);
                break;
            };
            self.write.push_str(text[..cr].trim_end_matches('\r'));
            self.previous.overwrite(std::mem::take(&mut self.write));
            text = &text[cr + 1..];
        }
    }

    pub(crate) fn render(mut self) -> String {
        self.previous.overwrite(self.write);
        self.previous.render()
    }
}

#[derive(Clone, Debug, Default)]
struct LiveCommandOutputLine {
    head: String,
    tail: String,
    omitted_bytes: usize,
    variable_width: bool,
}

impl LiveCommandOutputLine {
    fn overwrite(&mut self, write: Self) {
        if write.head.is_empty() {
            return;
        }
        if !self.variable_width && !write.variable_width {
            // Byte offsets equal columns, even across an omitted middle.
            let end = write.head.len() + write.omitted_bytes + write.tail.len();
            let tail_start = self.head.len() + self.omitted_bytes;
            if end >= tail_start + self.tail.len() {
                *self = write;
            } else {
                self.head.replace_range(..write.head.len(), &write.head);
                let overlap = end.saturating_sub(tail_start);
                self.tail
                    .replace_range(..overlap, &write.tail[write.tail.len() - overlap..]);
            }
            return;
        }

        let mut raw = self.render();
        if !raw.is_empty() {
            raw.push('\r');
        }
        raw.push_str(&write.render());
        match command_output_line(&raw) {
            Cow::Borrowed(_) => *self = write,
            Cow::Owned(line) => {
                *self = Self::default();
                self.push_str(&line);
            }
        }
    }

    fn push_str(&mut self, chunk: &str) {
        self.variable_width |= !chunk.is_ascii() || chunk.contains(['\x1b', '\t', '\r']);
        let head_remaining = if self.tail.is_empty() && self.omitted_bytes == 0 {
            LIVE_COMMAND_OUTPUT_LINE_HEAD_BYTES.saturating_sub(self.head.len())
        } else {
            0
        };
        let mut head_end = head_remaining.min(chunk.len());
        while !chunk.is_char_boundary(head_end) {
            head_end -= 1;
        }
        self.head.push_str(&chunk[..head_end]);
        let chunk = &chunk[head_end..];
        if chunk.is_empty() {
            return;
        }

        if chunk.len() >= LIVE_COMMAND_OUTPUT_LINE_TAIL_BYTES {
            let mut tail_start = chunk.len() - LIVE_COMMAND_OUTPUT_LINE_TAIL_BYTES;
            while !chunk.is_char_boundary(tail_start) {
                tail_start += 1;
            }
            self.omitted_bytes = self
                .omitted_bytes
                .saturating_add(self.tail.len())
                .saturating_add(tail_start);
            self.tail.clear();
            self.tail.push_str(&chunk[tail_start..]);
            return;
        }

        self.tail.push_str(chunk);
        let mut tail_start = self
            .tail
            .len()
            .saturating_sub(LIVE_COMMAND_OUTPUT_LINE_TAIL_BYTES);
        while !self.tail.is_char_boundary(tail_start) {
            tail_start += 1;
        }
        if tail_start > 0 {
            self.tail.drain(..tail_start);
            self.omitted_bytes = self.omitted_bytes.saturating_add(tail_start);
        }
    }

    /// Renders the retained line, closing truncated ANSI sequences before the omission marker.
    fn render(&self) -> String {
        let omission_marker = (self.omitted_bytes > 0)
            .then(|| format!("... {} bytes omitted ...", self.omitted_bytes));
        let mut line = String::with_capacity(
            self.head
                .len()
                .saturating_add(self.tail.len())
                .saturating_add(omission_marker.as_ref().map_or(0, String::len))
                .saturating_add(1),
        );
        line.push_str(&self.head);
        if let Some(omission_marker) = omission_marker {
            let terminator = self.head.rfind('\x1b').and_then(|start| {
                let escape = &self.head.as_bytes()[start + 1..];
                match escape.first() {
                    Some(b']') if !escape.contains(&b'\x07') => Some('\x07'),
                    Some(b'[') if !escape[1..].iter().any(u8::is_ascii_alphabetic) => Some('m'),
                    _ => None,
                }
            });
            if let Some(terminator) = terminator {
                line.push(terminator);
            }
            line.push_str("\x1b[0m");
            line.push_str(&omission_marker);
        }
        line.push_str(&self.tail);
        line
    }
}

#[cfg(test)]
#[path = "live_output_tests.rs"]
mod tests;
