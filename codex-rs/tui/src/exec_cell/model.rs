use std::time::Duration;
use std::time::Instant;

use codex_protocol::parse_command::ParsedCommand;
use codex_protocol::protocol::ExecOutputStream;

const LIVE_OUTPUT_MAX_BYTES: usize = 16 * 1024;
const LIVE_OUTPUT_TARGET_BYTES: usize = 12 * 1024;

#[derive(Debug, Clone, Default)]
pub(crate) struct LiveCommandOutput {
    buffer: String,
    truncated: bool,
}

impl LiveCommandOutput {
    pub(crate) fn append(&mut self, _stream: ExecOutputStream, chunk: &[u8]) {
        if chunk.is_empty() {
            return;
        }

        let text = String::from_utf8_lossy(chunk);
        if text.is_empty() {
            return;
        }

        self.buffer.push_str(&text);
        self.trim();
    }

    pub(crate) fn extend(&mut self, other: LiveCommandOutput) {
        if other.buffer.is_empty() {
            return;
        }

        self.buffer.push_str(&other.buffer);
        self.truncated |= other.truncated;
        self.trim();
    }

    pub(crate) fn text(&self) -> &str {
        &self.buffer
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    pub(crate) fn was_truncated(&self) -> bool {
        self.truncated
    }

    pub(crate) fn clear(&mut self) {
        self.buffer.clear();
        self.truncated = false;
    }

    fn trim(&mut self) {
        if self.buffer.len() <= LIVE_OUTPUT_MAX_BYTES {
            return;
        }

        let mut drop = self.buffer.len().saturating_sub(LIVE_OUTPUT_TARGET_BYTES);

        if drop >= self.buffer.len() {
            self.buffer.clear();
            self.truncated = true;
            return;
        }

        while drop < self.buffer.len() && !self.buffer.is_char_boundary(drop) {
            drop += 1;
        }

        self.buffer.drain(..drop);
        self.truncated = true;
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
    pub(crate) live_output: LiveCommandOutput,
}

#[derive(Debug)]
pub(crate) struct ExecCell {
    pub(crate) calls: Vec<ExecCall>,
}

impl ExecCell {
    pub(crate) fn new(call: ExecCall) -> Self {
        Self { calls: vec![call] }
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
            live_output: LiveCommandOutput::default(),
        };
        if self.is_exploring_cell() && Self::is_exploring_call(&call) {
            Some(Self {
                calls: [self.calls.clone(), vec![call]].concat(),
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
            call.live_output.clear();
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
            }
        }
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

    pub(crate) fn append_live_output(
        &mut self,
        call_id: &str,
        stream: ExecOutputStream,
        chunk: &[u8],
    ) -> bool {
        if let Some(call) = self
            .calls
            .iter_mut()
            .rev()
            .find(|c| c.call_id == call_id && c.output.is_none())
        {
            call.live_output.append(stream, chunk);
            return true;
        }
        false
    }

    pub(crate) fn apply_pending_live_output(
        &mut self,
        call_id: &str,
        pending: LiveCommandOutput,
    ) -> bool {
        if pending.is_empty() {
            return false;
        }
        if let Some(call) = self
            .calls
            .iter_mut()
            .rev()
            .find(|c| c.call_id == call_id && c.output.is_none())
        {
            call.live_output.extend(pending);
            return true;
        }
        false
    }

    pub(crate) fn iter_calls(&self) -> impl Iterator<Item = &ExecCall> {
        self.calls.iter()
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

    fn sample_call() -> ExecCall {
        ExecCall {
            call_id: "call-1".to_string(),
            command: vec!["bash".into(), "-lc".into(), "echo hi".into()],
            parsed: Vec::new(),
            output: None,
            start_time: Some(Instant::now()),
            duration: None,
            live_output: LiveCommandOutput::default(),
        }
    }

    #[test]
    fn live_output_append_and_trim() {
        let mut live = LiveCommandOutput::default();

        live.append(ExecOutputStream::Stdout, b"hello");
        assert_eq!(live.text(), "hello");
        assert!(!live.was_truncated());

        let large_chunk = vec![b'a'; LIVE_OUTPUT_MAX_BYTES + 100];
        live.append(ExecOutputStream::Stdout, &large_chunk);

        assert!(live.text().len() <= LIVE_OUTPUT_MAX_BYTES);
        assert!(live.was_truncated());
    }

    #[test]
    fn extend_merges_buffers_and_truncation() {
        let mut live = LiveCommandOutput::default();
        live.append(ExecOutputStream::Stdout, b"prefix\n");

        let mut pending = LiveCommandOutput::default();
        pending.append(ExecOutputStream::Stdout, &vec![b'b'; LIVE_OUTPUT_MAX_BYTES]);
        pending.append(ExecOutputStream::Stdout, b"tail");
        let pending_text = pending.text().to_string();

        live.extend(pending.clone());

        assert!(live.text().ends_with(&pending_text));
        assert!(live.was_truncated());
        assert!(pending.was_truncated());
    }

    #[test]
    fn append_live_output_updates_active_call_only() {
        let mut cell = ExecCell::new(sample_call());

        assert!(cell.append_live_output("call-1", ExecOutputStream::Stdout, b"one"));
        let text = cell
            .iter_calls()
            .next()
            .expect("call")
            .live_output
            .text()
            .to_string();
        assert_eq!(text, "one");

        assert!(!cell.append_live_output("call-2", ExecOutputStream::Stdout, b"two"));
    }

    #[test]
    fn apply_pending_live_output_skips_completed_calls() {
        let mut cell = ExecCell::new(sample_call());

        let mut pending = LiveCommandOutput::default();
        pending.append(ExecOutputStream::Stdout, b"pending");
        assert!(cell.apply_pending_live_output("call-1", pending));
        assert_eq!(
            cell.iter_calls().next().expect("call").live_output.text(),
            "pending"
        );

        cell.complete_call(
            "call-1",
            CommandOutput {
                exit_code: 0,
                stdout: String::new(),
                stderr: String::new(),
                formatted_output: String::new(),
            },
            Duration::from_secs(1),
        );

        let mut late = LiveCommandOutput::default();
        late.append(ExecOutputStream::Stdout, b"late");
        assert!(!cell.apply_pending_live_output("call-1", late));
    }
}
