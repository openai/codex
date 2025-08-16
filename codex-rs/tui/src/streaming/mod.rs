use crate::markdown_stream::AnimatedLineStreamer;
use crate::markdown_stream::MarkdownStreamCollector;
pub(crate) mod controller;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StreamKind {
    Answer,
    Reasoning,
}

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
    reasoning_emitted_this_turn: bool,
    answer_emitted_this_turn: bool,
    reasoning_emitted_in_stream: bool,
    answer_emitted_in_stream: bool,
}

impl HeaderEmitter {
    pub(crate) fn new() -> Self {
        Self {
            reasoning_emitted_this_turn: false,
            answer_emitted_this_turn: false,
            reasoning_emitted_in_stream: false,
            answer_emitted_in_stream: false,
        }
    }

    pub(crate) fn reset_for_new_turn(&mut self) {
        self.reasoning_emitted_this_turn = false;
        self.answer_emitted_this_turn = false;
        self.reasoning_emitted_in_stream = false;
        self.answer_emitted_in_stream = false;
    }

    pub(crate) fn reset_for_stream(&mut self, kind: StreamKind) {
        match kind {
            StreamKind::Reasoning => self.reasoning_emitted_in_stream = false,
            StreamKind::Answer => self.answer_emitted_in_stream = false,
        }
    }

    pub(crate) fn has_emitted_for_stream(&self, kind: StreamKind) -> bool {
        match kind {
            StreamKind::Reasoning => self.reasoning_emitted_in_stream,
            StreamKind::Answer => self.answer_emitted_in_stream,
        }
    }

    /// Allow emitting the header again for the same kind within the current turn.
    ///
    /// This is used when a stream (e.g., Answer) is finalized and a subsequent
    /// block of the same kind is started within the same turn. Without this,
    /// only the first block would render a header.
    pub(crate) fn allow_reemit_for_same_kind_in_turn(&mut self, kind: StreamKind) {
        match kind {
            StreamKind::Reasoning => self.reasoning_emitted_this_turn = false,
            StreamKind::Answer => self.answer_emitted_this_turn = false,
        }
    }

    pub(crate) fn maybe_emit(
        &mut self,
        kind: StreamKind,
        out_lines: &mut Vec<ratatui::text::Line<'static>>,
    ) -> bool {
        let already_emitted_this_turn = match kind {
            StreamKind::Reasoning => self.reasoning_emitted_this_turn,
            StreamKind::Answer => self.answer_emitted_this_turn,
        };
        let already_emitted_in_stream = self.has_emitted_for_stream(kind);
        if !already_emitted_in_stream && !already_emitted_this_turn {
            out_lines.push(render_header_line(kind));
            match kind {
                StreamKind::Reasoning => {
                    self.reasoning_emitted_in_stream = true;
                    self.reasoning_emitted_this_turn = true;
                    // Reset opposite header so it may be emitted again this turn
                    self.answer_emitted_this_turn = false;
                }
                StreamKind::Answer => {
                    self.answer_emitted_in_stream = true;
                    self.answer_emitted_this_turn = true;
                    // Reset opposite header so it may be emitted again this turn
                    self.reasoning_emitted_this_turn = false;
                }
            }
            true
        } else {
            false
        }
    }
}

fn render_header_line(kind: StreamKind) -> ratatui::text::Line<'static> {
    use ratatui::style::Stylize;
    match kind {
        StreamKind::Reasoning => ratatui::text::Line::from("thinking".magenta().italic()),
        StreamKind::Answer => ratatui::text::Line::from("codex".magenta().bold()),
    }
}
