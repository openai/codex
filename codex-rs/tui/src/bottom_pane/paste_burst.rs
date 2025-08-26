use std::time::Duration;
use std::time::Instant;

// Heuristic thresholds for detecting paste-like input bursts.
// Detect quickly to avoid showing typed prefix before paste is recognized
const PASTE_BURST_MIN_CHARS: u16 = 3;
const PASTE_BURST_CHAR_INTERVAL: Duration = Duration::from_millis(8);
const PASTE_ENTER_SUPPRESS_WINDOW: Duration = Duration::from_millis(120);

#[derive(Default)]
pub(crate) struct PasteBurst {
    last_plain_char_time: Option<Instant>,
    consecutive_plain_char_burst: u16,
    burst_window_until: Option<Instant>,
    buffer: String,
    active: bool,
    // Hold first fast char briefly to avoid rendering flicker
    pending_first_char: Option<(char, Instant)>,
}

pub(crate) enum CharDecision {
    BeginBuffer { retro_chars: u16 },
    BufferAppend,
    // Do not insert/render this char yet; we are holding briefly.
    HoldFirstChar,
    // Begin buffering using the previously held first char (no retro grab needed).
    BeginBufferFromPending,
}

pub(crate) struct RetroGrab {
    pub start_byte: usize,
    pub grabbed: String,
}

impl PasteBurst {
    pub fn recommended_flush_delay() -> Duration {
        PASTE_BURST_CHAR_INTERVAL + Duration::from_millis(1)
    }
    // Entry point: decide how to treat a plain char with current timing.
    pub fn on_plain_char(&mut self, ch: char, now: Instant) -> CharDecision {
        match self.last_plain_char_time {
            Some(prev) if now.duration_since(prev) <= PASTE_BURST_CHAR_INTERVAL => {
                self.consecutive_plain_char_burst =
                    self.consecutive_plain_char_burst.saturating_add(1)
            }
            _ => self.consecutive_plain_char_burst = 1,
        }
        self.last_plain_char_time = Some(now);

        if self.active {
            self.burst_window_until = Some(now + PASTE_ENTER_SUPPRESS_WINDOW);
            return CharDecision::BufferAppend;
        }

        // If we already held a first char and receive a second fast char,
        // start buffering without retro-grabbing (we never rendered the first).
        if let Some((held, held_at)) = self.pending_first_char
            && now.duration_since(held_at) <= PASTE_BURST_CHAR_INTERVAL
        {
            self.active = true;
            // take() to clear pending; we already captured the held char above
            let _ = self.pending_first_char.take();
            self.buffer.push(held);
            self.burst_window_until = Some(now + PASTE_ENTER_SUPPRESS_WINDOW);
            return CharDecision::BeginBufferFromPending;
        }

        if self.consecutive_plain_char_burst >= PASTE_BURST_MIN_CHARS {
            return CharDecision::BeginBuffer {
                retro_chars: self.consecutive_plain_char_burst.saturating_sub(1),
            };
        }

        // Hold the first fast char very briefly to see if a burst follows.
        self.pending_first_char = Some((ch, now));
        CharDecision::HoldFirstChar
    }

    // Timer: flush buffered burst if timeout elapsed.
    pub fn flush_if_due(&mut self, now: Instant) -> Option<String> {
        let timed_out = self
            .last_plain_char_time
            .is_some_and(|t| now.duration_since(t) > PASTE_BURST_CHAR_INTERVAL);
        if timed_out && self.is_active_internal() {
            self.active = false;
            let out = std::mem::take(&mut self.buffer);
            Some(out)
        } else if timed_out {
            // If we were holding a single fast char and no burst followed,
            // flush it as normal typed input.
            if let Some((ch, _at)) = self.pending_first_char.take() {
                return Some(ch.to_string());
            }
            None
        } else {
            None
        }
    }

    // While bursting: accumulate newline instead of submitting.
    pub fn append_newline_if_active(&mut self, now: Instant) -> bool {
        if self.is_active() {
            self.buffer.push('\n');
            self.burst_window_until = Some(now + PASTE_ENTER_SUPPRESS_WINDOW);
            true
        } else {
            false
        }
    }

    // Decide if Enter should insert a newline (burst context) vs submit.
    pub fn newline_should_insert_instead_of_submit(&self, now: Instant) -> bool {
        let in_burst_window = self.burst_window_until.is_some_and(|until| now <= until);
        self.is_active() || in_burst_window
    }

    // Keep the burst window alive.
    pub fn extend_window(&mut self, now: Instant) {
        self.burst_window_until = Some(now + PASTE_ENTER_SUPPRESS_WINDOW);
    }

    // Begin buffering with retroactively grabbed text.
    pub fn begin_with_retro_grabbed(&mut self, grabbed: String, now: Instant) {
        if !grabbed.is_empty() {
            self.buffer.push_str(&grabbed);
        }
        self.active = true;
        self.burst_window_until = Some(now + PASTE_ENTER_SUPPRESS_WINDOW);
    }

    // Append a char into the burst buffer.
    pub fn append_char_to_buffer(&mut self, ch: char, now: Instant) {
        self.buffer.push(ch);
        self.burst_window_until = Some(now + PASTE_ENTER_SUPPRESS_WINDOW);
    }

    // Decide whether to begin buffering by retroactively capturing recent chars
    // from the slice before the cursor. Returns start byte and grabbed text
    // if it looks like a paste (whitespace present or long segment).
    pub fn decide_begin_buffer(
        &mut self,
        now: Instant,
        before: &str,
        retro_chars: usize,
    ) -> Option<RetroGrab> {
        let start_byte = retro_start_index(before, retro_chars);
        let grabbed = before[start_byte..].to_string();
        let looks_pastey = grabbed.chars().any(|c| c.is_whitespace()) || grabbed.len() >= 16;
        if looks_pastey {
            // Note: caller is responsible for removing this slice from UI text.
            self.begin_with_retro_grabbed(grabbed.clone(), now);
            Some(RetroGrab {
                start_byte,
                grabbed,
            })
        } else {
            None
        }
    }

    // Before applying modified/non-char input: flush buffered burst immediately.
    pub fn flush_before_modified_input(&mut self) -> Option<String> {
        if self.is_active() {
            self.active = false;
            Some(std::mem::take(&mut self.buffer))
        } else {
            None
        }
    }

    // Clear only the timing window (keep buffer state untouched if already flushed).
    pub fn clear_window_after_non_char(&mut self) {
        self.consecutive_plain_char_burst = 0;
        self.last_plain_char_time = None;
        self.burst_window_until = None;
        self.active = false;
        self.pending_first_char = None;
    }

    // Utility
    pub fn is_active(&self) -> bool {
        self.is_active_internal() || self.pending_first_char.is_some()
    }

    fn is_active_internal(&self) -> bool {
        self.active || !self.buffer.is_empty()
    }

    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear_after_explicit_paste(&mut self) {
        self.last_plain_char_time = None;
        self.consecutive_plain_char_burst = 0;
        self.burst_window_until = None;
        self.active = false;
        self.buffer.clear();
        self.pending_first_char = None;
    }
}

pub(crate) fn retro_start_index(before: &str, retro_chars: usize) -> usize {
    if retro_chars == 0 {
        return before.len();
    }
    before
        .char_indices()
        .rev()
        .nth(retro_chars.saturating_sub(1))
        .map(|(idx, _)| idx)
        .unwrap_or(0)
}
