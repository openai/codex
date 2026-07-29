use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

/// Scroll position and geometry for a vertical scroll view.
#[derive(Clone, Copy, Debug, Default)]
pub struct ScrollViewState {
    pub scroll: u16,
    pub viewport_h: u16,
    pub content_h: u16,
}

impl ScrollViewState {
    pub fn clamp(&mut self) {
        let max_scroll = self.content_h.saturating_sub(self.viewport_h);
        if self.scroll > max_scroll {
            self.scroll = max_scroll;
        }
    }
}

/// A simple, local scrollable view for diffs or message text.
///
/// Owns raw lines, caches wrapped lines for a given width, and maintains
/// a small scroll state that is clamped whenever geometry shrinks.
#[derive(Clone, Debug, Default)]
pub struct ScrollableDiff {
    raw: Vec<String>,
    wrapped: Vec<String>,
    wrapped_src_idx: Vec<usize>,
    wrap_cols: Option<u16>,
    pub state: ScrollViewState,
}

impl ScrollableDiff {
    pub fn new() -> Self {
        Self::default()
    }

    /// Replace the raw content lines. Does not rewrap immediately; call `set_width` next.
    pub fn set_content(&mut self, lines: Vec<String>) {
        self.raw = lines;
        self.wrapped.clear();
        self.wrapped_src_idx.clear();
        self.state.content_h = 0;
        // Force rewrap on next set_width even if width is unchanged
        self.wrap_cols = None;
    }

    /// Set the wrap width. If changed, rebuild wrapped lines and clamp scroll.
    pub fn set_width(&mut self, width: u16) {
        if self.wrap_cols == Some(width) {
            return;
        }
        self.wrap_cols = Some(width);
        self.rewrap(width);
        self.state.clamp();
    }

    /// Update viewport height and clamp scroll if needed.
    pub fn set_viewport(&mut self, height: u16) {
        self.state.viewport_h = height;
        self.state.clamp();
    }

    /// Return the cached wrapped lines. Call `set_width` first when area changes.
    pub fn wrapped_lines(&self) -> &[String] {
        &self.wrapped
    }

    pub fn wrapped_src_indices(&self) -> &[usize] {
        &self.wrapped_src_idx
    }

    pub fn raw_line_at(&self, idx: usize) -> &str {
        self.raw.get(idx).map(String::as_str).unwrap_or("")
    }

    /// Scroll by a signed delta; clamps to content.
    pub fn scroll_by(&mut self, delta: i16) {
        let s = self.state.scroll as i32 + delta as i32;
        self.state.scroll = s.clamp(0, self.max_scroll() as i32) as u16;
    }

    /// Page by a signed delta; typically viewport_h - 1.
    pub fn page_by(&mut self, delta: i16) {
        self.scroll_by(delta);
    }

    pub fn scroll_to_top(&mut self) {
        self.state.scroll = 0;
    }

    pub fn scroll_to_bottom(&mut self) {
        self.state.scroll = self.max_scroll();
    }

    /// Optional percent scrolled; None when not enough geometry is known.
    pub fn percent_scrolled(&self) -> Option<u8> {
        if self.state.content_h == 0 || self.state.viewport_h == 0 {
            return None;
        }
        if self.state.content_h <= self.state.viewport_h {
            return None;
        }
        let visible_bottom = self.state.scroll.saturating_add(self.state.viewport_h) as f32;
        let pct = (visible_bottom / self.state.content_h as f32 * 100.0).round();
        Some(pct.clamp(0.0, 100.0) as u8)
    }

    fn max_scroll(&self) -> u16 {
        self.state.content_h.saturating_sub(self.state.viewport_h)
    }

    fn rewrap(&mut self, width: u16) {
        if width == 0 {
            self.wrapped = self.raw.clone();
            self.state.content_h = self.wrapped.len() as u16;
            return;
        }
        let max_cols = width as usize;
        let mut out: Vec<String> = Vec::new();
        let mut out_idx: Vec<usize> = Vec::new();
        for (raw_idx, raw) in self.raw.iter().enumerate() {
            // Normalize tabs for width accounting (MVP: 4 spaces).
            let raw = raw.replace('\t', "    ");
            if raw.is_empty() {
                out.push(String::new());
                out_idx.push(raw_idx);
                continue;
            }
            let mut line = String::new();
            let mut line_cols = 0usize;
            let mut last_soft_idx: Option<usize> = None; // last whitespace or punctuation break
            for grapheme in raw
                .graphemes(/*is_extended*/ true)
                .flat_map(|grapheme| grapheme.split_inclusive(char::is_whitespace))
            {
                if grapheme == "\n" {
                    out.push(std::mem::take(&mut line));
                    out_idx.push(raw_idx);
                    line_cols = 0;
                    last_soft_idx = None;
                    continue;
                }
                let grapheme_width = display_width(grapheme);
                if line_cols.saturating_add(grapheme_width) > max_cols {
                    if let Some(split) = last_soft_idx {
                        let (prefix, rest) = line.split_at(split);
                        let prefix = prefix.trim_end();
                        if !prefix.is_empty() {
                            out.push(prefix.to_string());
                            out_idx.push(raw_idx);
                        }
                        line = rest.trim_start().to_string();
                        last_soft_idx = None;
                        // Retry adding the current grapheme now that the line may be shorter.
                    } else if !line.is_empty() {
                        out.push(std::mem::take(&mut line));
                        out_idx.push(raw_idx);
                    }
                }
                if grapheme.chars().all(char::is_whitespace)
                    || grapheme.chars().any(|ch| {
                        matches!(
                            ch,
                            ',' | ';'
                                | '.'
                                | ':'
                                | ')'
                                | ']'
                                | '}'
                                | '|'
                                | '/'
                                | '?'
                                | '!'
                                | '-'
                                | '_'
                        )
                    })
                {
                    last_soft_idx = Some(line.len());
                }
                line.push_str(grapheme);
                line_cols = display_width(&line);
            }
            if !line.is_empty() {
                out.push(line);
                out_idx.push(raw_idx);
            }
        }
        self.wrapped = out;
        self.wrapped_src_idx = out_idx;
        self.state.content_h = self.wrapped.len() as u16;
    }
}

/// Counts terminal cells, including the halfwidth sound marks omitted by `unicode-width`.
fn display_width(text: &str) -> usize {
    UnicodeWidthStr::width(text)
        + text
            .chars()
            .filter(|ch| matches!(ch, '\u{FF9E}' | '\u{FF9F}'))
            .count()
}

#[cfg(test)]
#[path = "scrollable_diff_tests.rs"]
mod tests;
