use crate::markdown_stream::AnimatedLineStreamer;
use crate::markdown_stream::MarkdownStreamCollector;
pub(crate) mod controller;

pub(crate) struct StreamState {
    pub(crate) collector: MarkdownStreamCollector,
    pub(crate) streamer: AnimatedLineStreamer,
    pub(crate) has_seen_delta: bool,
}

impl StreamState {
    pub(crate) fn new() -> Self {
        Self {
            collector: MarkdownStreamCollector::new(),
            streamer: AnimatedLineStreamer::new(),
            has_seen_delta: false,
        }
    }
    pub(crate) fn clear(&mut self) {
        self.collector.clear();
        self.streamer.clear();
        self.has_seen_delta = false;
    }
    pub(crate) fn step(&mut self) -> crate::markdown_stream::StepResult {
        self.streamer.step()
    }
    pub(crate) fn drain_all(&mut self) -> crate::markdown_stream::StepResult {
        self.streamer.drain_all()
    }
    pub(crate) fn is_idle(&self) -> bool {
        self.streamer.is_idle()
    }
    pub(crate) fn enqueue(&mut self, lines: Vec<ratatui::text::Line<'static>>) {
        self.streamer.enqueue(lines)
    }
}

pub(crate) struct HeaderEmitter {
    emitted_this_turn: bool,
    emitted_in_stream: bool,
    label: HeaderLabel,
}

#[derive(Copy, Clone)]
pub(crate) enum HeaderLabel {
    Codex,
    Thinking,
}

impl HeaderEmitter {
    pub(crate) fn new() -> Self {
        Self::with_label(HeaderLabel::Codex)
    }

    pub(crate) fn with_label(label: HeaderLabel) -> Self {
        Self {
            emitted_this_turn: false,
            emitted_in_stream: false,
            label,
        }
    }

    pub(crate) fn reset_for_new_turn(&mut self) {
        self.emitted_this_turn = false;
        self.emitted_in_stream = false;
    }

    pub(crate) fn reset_for_stream(&mut self) {
        self.emitted_in_stream = false;
    }

    /// Allow emitting the header again within the current turn after a finalize.
    pub(crate) fn allow_reemit_in_turn(&mut self) {
        self.emitted_this_turn = false;
    }

    pub(crate) fn maybe_emit(&mut self, out_lines: &mut Vec<ratatui::text::Line<'static>>) -> bool {
        if !self.emitted_in_stream && !self.emitted_this_turn {
            // Add a leading blank line before the header for visual spacing
            out_lines.push(ratatui::text::Line::from(""));
            out_lines.push(render_header_line(self.label));
            self.emitted_in_stream = true;
            self.emitted_this_turn = true;
            return true;
        }
        false
    }
}

fn render_header_line(label: HeaderLabel) -> ratatui::text::Line<'static> {
    use ratatui::style::Stylize;
    match label {
        HeaderLabel::Codex => ratatui::text::Line::from("codex".magenta().bold()),
        HeaderLabel::Thinking => ratatui::text::Line::from("thinking".magenta().italic()),
    }
}
