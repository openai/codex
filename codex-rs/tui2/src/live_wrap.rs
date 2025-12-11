use unicode_width::UnicodeWidthChar;
use unicode_width::UnicodeWidthStr;

/// A single visual row produced by RowBuilder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub text: String,
    /// True if this row ends with an explicit line break (as opposed to a hard wrap).
    pub explicit_break: bool,
}

impl Row {
    pub fn width(&self) -> usize {
        self.text.width()
    }
}

/// Incrementally wraps input text into visual rows of at most `width` cells.
///
/// Step 1: plain-text only. ANSI-carry and styled spans will be added later.
pub struct RowBuilder {
    target_width: usize,
    /// Buffer for the current logical line (until a '\n' is seen).
    current_line: String,
    /// Output rows built so far for the current logical line and previous ones.
    rows: Vec<Row>,
}

impl RowBuilder {
    pub fn new(target_width: usize) -> Self {
        Self {
            target_width: target_width.max(1),
            current_line: String::new(),
            rows: Vec::new(),
        }
    }

    pub fn width(&self) -> usize {
        self.target_width
    }

    pub fn set_width(&mut self, width: usize) {
        self.target_width = width.max(1);
        // Rewrap everything we have (simple approach for Step 1).
        let mut all = String::new();
        for row in self.rows.drain(..) {
            all.push_str(&row.text);
            if row.explicit_break {
                all.push('\n');
            }
        }
        all.push_str(&self.current_line);
        self.current_line.clear();
        self.push_fragment(&all);
    }

    /// Push an input fragment. May contain newlines.
    pub fn push_fragment(&mut self, fragment: &str) {
        if fragment.is_empty() {
            return;
        }
        let mut start = 0usize;
        for (i, ch) in fragment.char_indices() {
            if ch == '\n' {
                // Flush anything pending before the newline.
                if start < i {
                    self.current_line.push_str(&fragment[start..i]);
                }
                self.flush_current_line(true);
                start = i + ch.len_utf8();
            }
        }
        if start < fragment.len() {
            self.current_line.push_str(&fragment[start..]);
            self.wrap_current_line();
        }
    }

    /// Mark the end of the current logical line (equivalent to pushing a '\n').
    pub fn end_line(&mut self) {
        self.flush_current_line(true);
    }

    /// Drain and return all produced rows.
    pub fn drain_rows(&mut self) -> Vec<Row> {
        std::mem::take(&mut self.rows)
    }

    /// Return a snapshot of produced rows (non-draining).
    pub fn rows(&self) -> &[Row] {
        &self.rows
    }

    /// Rows suitable for display, including the current partial line if any.
    pub fn display_rows(&self) -> Vec<Row> {
        let mut out = self.rows.clone();
        if !self.current_line.is_empty() {
            out.push(Row {
                text: self.current_line.clone(),
                explicit_break: false,
            });
        }
        out
    }

    /// Drain the oldest rows that exceed `max_keep` display rows (including the
    /// current partial line, if any). Returns the drained rows in order.
    pub fn drain_commit_ready(&mut self, max_keep: usize) -> Vec<Row> {
        let display_count = self.rows.len() + if self.current_line.is_empty() { 0 } else { 1 };
        if display_count <= max_keep {
            return Vec::new();
        }
        let to_commit = display_count - max_keep;
        let commit_count = to_commit.min(self.rows.len());
        let mut drained = Vec::with_capacity(commit_count);
        for _ in 0..commit_count {
            drained.push(self.rows.remove(0));
        }
        drained
    }

    fn flush_current_line(&mut self, explicit_break: bool) {
        // Wrap any remaining content in the current line and then finalize with explicit_break.
        self.wrap_current_line();
        // If the current line ended exactly on a width boundary and is non-empty, represent
        // the explicit break as an empty explicit row so that fragmentation invariance holds.
        if explicit_break {
            if self.current_line.is_empty() {
                // We ended on a boundary previously; add an empty explicit row.
                self.rows.push(Row {
                    text: String::new(),
                    explicit_break: true,
                });
            } else {
                // There is leftover content that did not wrap yet; push it now with the explicit flag.
                let mut s = String::new();
                std::mem::swap(&mut s, &mut self.current_line);
                self.rows.push(Row {
                    text: s,
                    explicit_break: true,
                });
            }
        }
        // Reset current line buffer for next logical line.
        self.current_line.clear();
    }

    fn wrap_current_line(&mut self) {
        // While the current_line exceeds width, cut a prefix.
        loop {
            if self.current_line.is_empty() {
                break;
            }
            let (prefix, suffix, taken) =
                take_prefix_by_width(&self.current_line, self.target_width);
            if taken == 0 {
                // Avoid infinite loop on pathological inputs; take one scalar and continue.
                if let Some((i, ch)) = self.current_line.char_indices().next() {
                    let len = i + ch.len_utf8();
                    let p = self.current_line[..len].to_string();
                    self.rows.push(Row {
                        text: p,
                        explicit_break: false,
                    });
                    self.current_line = self.current_line[len..].to_string();
                    continue;
                }
                break;
            }
            if suffix.is_empty() {
                // Fits entirely; keep in buffer (do not push yet) so we can append more later.
                break;
            } else {
                // Emit wrapped prefix as a non-explicit row and continue with the remainder.
                self.rows.push(Row {
                    text: prefix,
                    explicit_break: false,
                });
                self.current_line = suffix.to_string();
            }
        }
    }
}

/// Take a prefix of `text` whose visible width is at most `max_cols`.
/// Returns (prefix, suffix, prefix_width).
pub fn take_prefix_by_width(text: &str, max_cols: usize) -> (String, &str, usize) {
    if max_cols == 0 || text.is_empty() {
        return (String::new(), text, 0);
    }
    let mut cols = 0usize;
    let mut end_idx = 0usize;
    for (i, ch) in text.char_indices() {
        let ch_width = UnicodeWidthChar::width(ch).unwrap_or(0);
        if cols.saturating_add(ch_width) > max_cols {
            break;
        }
        cols += ch_width;
        end_idx = i + ch.len_utf8();
        if cols == max_cols {
            break;
        }
    }
    let prefix = text[..end_idx].to_string();
    let suffix = &text[end_idx..];
    (prefix, suffix, cols)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn rows_do_not_exceed_width_ascii() {
        let mut rb = RowBuilder::new(10);
        rb.push_fragment("hello whirl this is a test");
        let rows = rb.rows().to_vec();
        assert_eq!(
            rows,
            vec![
                Row {
                    text: "hello whir".to_string(),
                    explicit_break: false
                },
                Row {
                    text: "l this is ".to_string(),
                    explicit_break: false
                }
            ]
        );
    }

    #[test]
    fn rows_do_not_exceed_width_emoji_cjk() {
        // 😀 is width 2; 你/好 are width 2.
        let mut rb = RowBuilder::new(6);
        rb.push_fragment("😀😀 你好");
        let rows = rb.rows().to_vec();
        // At width 6, we expect the first row to fit exactly two emojis and a space
        // (2 + 2 + 1 = 5) plus one more column for the first CJK char (2 would overflow),
        // so only the two emojis and the space fit; the rest remains buffered.
        assert_eq!(
            rows,
            vec![Row {
                text: "😀😀 ".to_string(),
                explicit_break: false
            }]
        );
    }

    #[test]
    fn fragmentation_invariance_long_token() {
        let s = "ABCDEFGHIJKLMNOPQRSTUVWXYZ"; // 26 chars
        let mut rb_all = RowBuilder::new(7);
        rb_all.push_fragment(s);
        let all_rows = rb_all.rows().to_vec();

        let mut rb_chunks = RowBuilder::new(7);
        for i in (0..s.len()).step_by(3) {
            let end = (i + 3).min(s.len());
            rb_chunks.push_fragment(&s[i..end]);
        }
        let chunk_rows = rb_chunks.rows().to_vec();

        assert_eq!(all_rows, chunk_rows);
    }

    #[test]
    fn newline_splits_rows() {
        let mut rb = RowBuilder::new(10);
        rb.push_fragment("hello\nworld");
        let rows = rb.display_rows();
        assert!(rows.iter().any(|r| r.explicit_break));
        assert_eq!(rows[0].text, "hello");
        // Second row should begin with 'world'
        assert!(rows.iter().any(|r| r.text.starts_with("world")));
    }

    #[test]
    fn rewrap_on_width_change() {
        let mut rb = RowBuilder::new(10);
        rb.push_fragment("abcdefghijK");
        assert!(!rb.rows().is_empty());
        rb.set_width(5);
        for r in rb.rows() {
            assert!(r.width() <= 5);
        }
    }
}
