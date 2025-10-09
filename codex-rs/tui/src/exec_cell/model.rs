use std::collections::VecDeque;
use std::time::Duration;
use std::time::Instant;

use codex_core::protocol::ExecOutputStream;
use codex_protocol::parse_command::ParsedCommand;

const LIVE_OUTPUT_MAX_LINES: usize = 200;

#[derive(Clone, Debug)]
pub(crate) struct LiveExecLine {
    pub(crate) stream: ExecOutputStream,
    pub(crate) text: String,
}

#[derive(Clone, Debug)]
pub(crate) struct LiveExecStream {
    lines: VecDeque<LiveExecLine>,
    pending_stdout: String,
    pending_stderr: String,
    pending_order: Vec<ExecOutputStream>,
}

impl LiveExecStream {
    pub(crate) fn new() -> Self {
        Self {
            lines: VecDeque::new(),
            pending_stdout: String::new(),
            pending_stderr: String::new(),
            pending_order: Vec::new(),
        }
    }

    pub(crate) fn clear(&mut self) {
        self.lines.clear();
        self.pending_stdout.clear();
        self.pending_stderr.clear();
        self.pending_order.clear();
    }

    pub(crate) fn has_content(&self) -> bool {
        !self.lines.is_empty() || !self.pending_stdout.is_empty() || !self.pending_stderr.is_empty()
    }

    pub(crate) fn push_chunk(&mut self, stream: ExecOutputStream, chunk: &[u8]) -> Option<String> {
        let text = String::from_utf8_lossy(chunk);
        match stream {
            ExecOutputStream::Stdout => self.pending_stdout.push_str(&text),
            ExecOutputStream::Stderr => self.pending_stderr.push_str(&text),
        }
        self.touch_pending_order(&stream);

        let (completed_lines, pending_snapshot, pending_empty) = {
            let pending = match stream {
                ExecOutputStream::Stdout => &mut self.pending_stdout,
                ExecOutputStream::Stderr => &mut self.pending_stderr,
            };
            let mut completed: Vec<String> = Vec::new();
            while let Some(idx) = pending.find('\n') {
                let mut line: String = pending.drain(..=idx).collect();
                if line.ends_with('\n') {
                    line.pop();
                }
                if line.ends_with('\r') {
                    line.pop();
                }
                completed.push(line);
            }
            let pending_snapshot = pending.clone();
            let pending_empty = pending.is_empty();
            (completed, pending_snapshot, pending_empty)
        };

        if pending_empty {
            self.pending_order.retain(|s| s != &stream);
        }

        for line in &completed_lines {
            self.push_line(stream.clone(), line.clone());
        }

        if let Some(last) = completed_lines.last() {
            Some(last.clone())
        } else if !pending_snapshot.is_empty() {
            Some(pending_snapshot)
        } else {
            None
        }
    }

    pub(crate) fn lines_for_display(&self) -> Vec<LiveExecLine> {
        let mut out: Vec<LiveExecLine> = self.lines.iter().cloned().collect();
        for stream in &self.pending_order {
            let pending = match stream {
                ExecOutputStream::Stdout => &self.pending_stdout,
                ExecOutputStream::Stderr => &self.pending_stderr,
            };
            if pending.is_empty() {
                continue;
            }
            out.push(LiveExecLine {
                stream: stream.clone(),
                text: pending.clone(),
            });
        }
        out
    }

    fn push_line(&mut self, stream: ExecOutputStream, text: String) {
        self.lines.push_back(LiveExecLine { stream, text });
        while self.lines.len() > LIVE_OUTPUT_MAX_LINES {
            self.lines.pop_front();
        }
    }

    fn touch_pending_order(&mut self, stream: &ExecOutputStream) {
        self.pending_order.retain(|s| s != stream);
        self.pending_order.push(stream.clone());
    }
}

#[derive(Clone, Debug)]
pub(crate) struct CommandOutput {
    pub(crate) exit_code: i32,
    pub(crate) stdout: String,
    pub(crate) stderr: String,
    pub(crate) formatted_output: String,
}

#[derive(Debug, Clone)]
pub(crate) struct ExecCall {
    pub(crate) call_id: String,
    pub(crate) command: Vec<String>,
    pub(crate) parsed: Vec<ParsedCommand>,
    pub(crate) output: Option<CommandOutput>,
    pub(crate) start_time: Option<Instant>,
    pub(crate) duration: Option<Duration>,
    pub(crate) live: LiveExecStream,
}

#[derive(Debug)]
pub(crate) struct ExecCell {
    pub(crate) calls: Vec<ExecCall>,
    show_live_output: bool,
}

impl ExecCell {
    pub(crate) fn new(call: ExecCall) -> Self {
        Self {
            calls: vec![call],
            show_live_output: false,
        }
    }

    pub(crate) fn with_added_call(
        &self,
        call_id: String,
        command: Vec<String>,
        parsed: Vec<ParsedCommand>,
    ) -> Option<Self> {
        let call = ExecCall {
            call_id,
            command,
            parsed,
            output: None,
            start_time: Some(Instant::now()),
            duration: None,
            live: LiveExecStream::new(),
        };
        if self.is_exploring_cell() && Self::is_exploring_call(&call) {
            Some(Self {
                calls: [self.calls.clone(), vec![call]].concat(),
                show_live_output: false,
            })
        } else {
            None
        }
    }

    pub(crate) fn complete_call(
        &mut self,
        call_id: &str,
        output: CommandOutput,
        duration: Duration,
    ) {
        if let Some(call) = self.calls.iter_mut().rev().find(|c| c.call_id == call_id) {
            call.output = Some(output);
            call.duration = Some(duration);
            call.start_time = None;
            call.live.clear();
            self.show_live_output = false;
        }
    }

    pub(crate) fn should_flush(&self) -> bool {
        !self.is_exploring_cell() && self.calls.iter().all(|c| c.output.is_some())
    }

    pub(crate) fn mark_failed(&mut self) {
        for call in self.calls.iter_mut() {
            if call.output.is_none() {
                let elapsed = call
                    .start_time
                    .map(|st| st.elapsed())
                    .unwrap_or_else(|| Duration::from_millis(0));
                call.start_time = None;
                call.duration = Some(elapsed);
                call.output = Some(CommandOutput {
                    exit_code: 1,
                    stdout: String::new(),
                    stderr: String::new(),
                    formatted_output: String::new(),
                });
                call.live.clear();
            }
        }
        self.show_live_output = false;
    }

    pub(crate) fn is_exploring_cell(&self) -> bool {
        self.calls.iter().all(Self::is_exploring_call)
    }

    pub(crate) fn is_active(&self) -> bool {
        self.calls.iter().any(|c| c.output.is_none())
    }

    pub(crate) fn active_start_time(&self) -> Option<Instant> {
        self.calls
            .iter()
            .find(|c| c.output.is_none())
            .and_then(|c| c.start_time)
    }

    pub(crate) fn iter_calls(&self) -> impl Iterator<Item = &ExecCall> {
        self.calls.iter()
    }

    pub(crate) fn append_live_chunk(
        &mut self,
        call_id: &str,
        stream: ExecOutputStream,
        chunk: &[u8],
    ) -> Option<String> {
        if let Some(call) = self.calls.iter_mut().rev().find(|c| c.call_id == call_id) {
            return call.live.push_chunk(stream, chunk);
        }
        None
    }

    pub(crate) fn toggle_live_output(&mut self) -> bool {
        self.show_live_output = !self.show_live_output;
        self.show_live_output
    }

    pub(crate) fn set_live_output_visible(&mut self, visible: bool) {
        self.show_live_output = visible;
    }

    pub(crate) fn show_live_output(&self) -> bool {
        self.show_live_output
    }

    pub(crate) fn latest_live_preview(&self) -> Option<String> {
        self.calls
            .iter()
            .rev()
            .find(|c| c.output.is_none())
            .and_then(|call| {
                if !call.live.has_content() {
                    None
                } else {
                    call.live
                        .lines_for_display()
                        .last()
                        .map(|line| line.text.clone())
                }
            })
    }

    pub(crate) fn live_lines_for_display(&self) -> Vec<LiveExecLine> {
        self.calls
            .iter()
            .rev()
            .find(|c| c.output.is_none())
            .map(|call| call.live.lines_for_display())
            .unwrap_or_default()
    }

    pub(super) fn is_exploring_call(call: &ExecCall) -> bool {
        !call.parsed.is_empty()
            && call.parsed.iter().all(|p| {
                matches!(
                    p,
                    ParsedCommand::Read { .. }
                        | ParsedCommand::ListFiles { .. }
                        | ParsedCommand::Search { .. }
                )
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn live_exec_stream_collects_lines() {
        let mut stream = LiveExecStream::new();
        let preview = stream.push_chunk(ExecOutputStream::Stdout, b"foo");
        assert_eq!(preview.as_deref(), Some("foo"));
        let mut lines = stream.lines_for_display();
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].text, "foo");

        let preview = stream.push_chunk(ExecOutputStream::Stdout, b"bar\nbaz\n");
        assert_eq!(preview.as_deref(), Some("baz"));
        lines = stream.lines_for_display();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].text, "foobar");
        assert_eq!(lines[1].text, "baz");
    }

    #[test]
    fn live_exec_stream_caps_lines() {
        let mut stream = LiveExecStream::new();
        for i in 0..(LIVE_OUTPUT_MAX_LINES + 5) {
            let line = format!("line-{i}\n");
            let _ = stream.push_chunk(ExecOutputStream::Stdout, line.as_bytes());
        }
        let lines = stream.lines_for_display();
        assert_eq!(lines.len(), LIVE_OUTPUT_MAX_LINES);
        assert_eq!(lines.first().unwrap().text, "line-5");
        assert_eq!(
            lines.last().unwrap().text,
            format!("line-{}", LIVE_OUTPUT_MAX_LINES + 4)
        );
    }
}
