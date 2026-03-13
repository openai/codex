use super::*;

pub(super) struct ExecBuffer {
    pub(super) event_call_id: String,
    pub(super) session_id: Option<String>,
    pub(super) session: Arc<Session>,
    pub(super) turn: Arc<TurnContext>,
    pub(super) logs: VecDeque<String>,
    pub(super) logs_bytes: usize,
    pub(super) logs_truncated: bool,
    pub(super) all_logs: Vec<String>,
    pub(super) all_logs_bytes: usize,
    pub(super) all_logs_truncated: bool,
    pub(super) final_output: Option<String>,
    pub(super) content_items: Vec<FunctionCallOutputContentItem>,
    pub(super) error: Option<String>,
    pub(super) done: bool,
    pub(super) host_terminating: bool,
    pub(super) terminal_kind: Option<ExecTerminalKind>,
    pub(super) completed_sequence: Option<u64>,
    pub(super) started_at: Instant,
    pub(super) notify: Arc<Notify>,
    pub(super) emitted_deltas: usize,
}

impl ExecBuffer {
    pub(super) fn new(
        event_call_id: String,
        session_id: Option<String>,
        session: Arc<Session>,
        turn: Arc<TurnContext>,
    ) -> Self {
        Self {
            event_call_id,
            session_id,
            session,
            turn,
            logs: VecDeque::new(),
            logs_bytes: 0,
            logs_truncated: false,
            all_logs: Vec::new(),
            all_logs_bytes: 0,
            all_logs_truncated: false,
            final_output: None,
            content_items: Vec::new(),
            error: None,
            done: false,
            host_terminating: false,
            terminal_kind: None,
            completed_sequence: None,
            started_at: Instant::now(),
            notify: Arc::new(Notify::new()),
            emitted_deltas: 0,
        }
    }

    pub(super) fn push_log(&mut self, text: String) {
        self.logs.push_back(text.clone());
        self.logs_bytes = self.logs_bytes.saturating_add(text.len());
        while self.logs_bytes > JS_REPL_POLL_LOG_QUEUE_MAX_BYTES {
            let Some(removed) = self.logs.pop_front() else {
                break;
            };
            self.logs_bytes = self.logs_bytes.saturating_sub(removed.len());
            self.logs_truncated = true;
        }
        if self.logs_truncated
            && self
                .logs
                .front()
                .is_none_or(|line| line != JS_REPL_POLL_LOGS_TRUNCATED_MARKER)
        {
            let marker_len = JS_REPL_POLL_LOGS_TRUNCATED_MARKER.len();
            while self.logs_bytes.saturating_add(marker_len) > JS_REPL_POLL_LOG_QUEUE_MAX_BYTES {
                let Some(removed) = self.logs.pop_front() else {
                    break;
                };
                self.logs_bytes = self.logs_bytes.saturating_sub(removed.len());
            }
            self.logs
                .push_front(JS_REPL_POLL_LOGS_TRUNCATED_MARKER.to_string());
            self.logs_bytes = self.logs_bytes.saturating_add(marker_len);
        }

        if self.all_logs_truncated {
            return;
        }
        let separator_bytes = if self.all_logs.is_empty() { 0 } else { 1 };
        let next_bytes = text.len() + separator_bytes;
        if self.all_logs_bytes.saturating_add(next_bytes) > JS_REPL_POLL_ALL_LOGS_MAX_BYTES {
            self.all_logs
                .push(JS_REPL_POLL_ALL_LOGS_TRUNCATED_MARKER.to_string());
            self.all_logs_truncated = true;
            return;
        }

        self.all_logs.push(text);
        self.all_logs_bytes = self.all_logs_bytes.saturating_add(next_bytes);
    }

    pub(super) fn poll_logs(&mut self) -> Vec<String> {
        let drained: Vec<String> = self.logs.drain(..).collect();
        self.logs_bytes = 0;
        self.logs_truncated = false;
        drained
    }

    pub(super) fn display_output(&self) -> String {
        if let Some(final_output) = self.final_output.as_deref()
            && !final_output.is_empty()
        {
            return final_output.to_string();
        }
        self.all_logs.join("\n")
    }

    pub(super) fn poll_final_output(&self) -> Option<String> {
        if self.done {
            self.final_output.clone()
        } else {
            None
        }
    }

    pub(super) fn poll_content_items(&self) -> Vec<FunctionCallOutputContentItem> {
        if self.done && self.error.is_none() {
            self.content_items.clone()
        } else {
            Vec::new()
        }
    }

    pub(super) fn output_delta_chunks_for_log_line(&mut self, line: &str) -> Vec<Vec<u8>> {
        if self.emitted_deltas >= MAX_EXEC_OUTPUT_DELTAS_PER_CALL {
            return Vec::new();
        }

        let mut text = String::with_capacity(line.len() + 1);
        text.push_str(line);
        text.push('\n');

        let remaining = MAX_EXEC_OUTPUT_DELTAS_PER_CALL - self.emitted_deltas;
        let chunks =
            split_utf8_chunks_with_limits(&text, JS_REPL_OUTPUT_DELTA_MAX_BYTES, remaining);
        self.emitted_deltas += chunks.len();
        chunks
    }
}

pub(super) fn split_utf8_chunks_with_limits(
    input: &str,
    max_bytes: usize,
    max_chunks: usize,
) -> Vec<Vec<u8>> {
    if input.is_empty() || max_bytes == 0 || max_chunks == 0 {
        return Vec::new();
    }

    let bytes = input.as_bytes();
    let mut output = Vec::new();
    let mut start = 0usize;
    while start < input.len() && output.len() < max_chunks {
        let mut end = (start + max_bytes).min(input.len());
        while end > start && !input.is_char_boundary(end) {
            end -= 1;
        }
        if end == start {
            if let Some(ch) = input[start..].chars().next() {
                end = (start + ch.len_utf8()).min(input.len());
            } else {
                break;
            }
        }

        output.push(bytes[start..end].to_vec());
        start = end;
    }
    output
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ExecTerminalKind {
    Success,
    Error,
    KernelExit,
    Cancelled,
}

pub(super) struct ExecCompletionEvent {
    pub(super) session: Arc<Session>,
    pub(super) turn: Arc<TurnContext>,
    pub(super) event_call_id: String,
    pub(super) output: String,
    pub(super) error: Option<String>,
    pub(super) duration: Duration,
    pub(super) timed_out: bool,
}

pub(super) enum KernelStreamEnd {
    Shutdown,
    StdoutEof,
}

impl KernelStreamEnd {
    pub(super) fn reason(&self) -> &'static str {
        match self {
            Self::Shutdown => "shutdown",
            Self::StdoutEof => "stdout_eof",
        }
    }

    pub(super) fn error(&self) -> Option<&str> {
        None
    }
}

pub(super) struct KernelDebugSnapshot {
    pub(super) pid: Option<u32>,
    pub(super) status: String,
    pub(super) stderr_tail: String,
}

pub(super) fn format_stderr_tail(lines: &VecDeque<String>) -> String {
    if lines.is_empty() {
        return "<empty>".to_string();
    }
    lines
        .iter()
        .cloned()
        .collect::<Vec<_>>()
        .join(JS_REPL_STDERR_TAIL_SEPARATOR)
}

pub(super) fn truncate_utf8_prefix_by_bytes(input: &str, max_bytes: usize) -> String {
    if input.len() <= max_bytes {
        return input.to_string();
    }
    if max_bytes == 0 {
        return String::new();
    }
    let mut end = max_bytes;
    while end > 0 && !input.is_char_boundary(end) {
        end -= 1;
    }
    input[..end].to_string()
}

pub(super) fn stderr_tail_formatted_bytes(lines: &VecDeque<String>) -> usize {
    if lines.is_empty() {
        return 0;
    }
    let payload_bytes: usize = lines.iter().map(String::len).sum();
    let separator_bytes = JS_REPL_STDERR_TAIL_SEPARATOR.len() * (lines.len() - 1);
    payload_bytes + separator_bytes
}

fn stderr_tail_bytes_with_candidate(lines: &VecDeque<String>, line: &str) -> usize {
    if lines.is_empty() {
        return line.len();
    }
    stderr_tail_formatted_bytes(lines) + JS_REPL_STDERR_TAIL_SEPARATOR.len() + line.len()
}

pub(super) fn push_stderr_tail_line(lines: &mut VecDeque<String>, line: &str) -> String {
    let max_line_bytes = JS_REPL_STDERR_TAIL_LINE_MAX_BYTES.min(JS_REPL_STDERR_TAIL_MAX_BYTES);
    let bounded_line = truncate_utf8_prefix_by_bytes(line, max_line_bytes);
    if bounded_line.is_empty() {
        return bounded_line;
    }

    while !lines.is_empty()
        && (lines.len() >= JS_REPL_STDERR_TAIL_LINE_LIMIT
            || stderr_tail_bytes_with_candidate(lines, &bounded_line)
                > JS_REPL_STDERR_TAIL_MAX_BYTES)
    {
        lines.pop_front();
    }

    lines.push_back(bounded_line.clone());
    bounded_line
}

pub(super) fn is_kernel_status_exited(status: &str) -> bool {
    status.starts_with("exited(")
}

pub(super) fn should_include_model_diagnostics_for_write_error(
    err_message: &str,
    snapshot: &KernelDebugSnapshot,
) -> bool {
    is_kernel_status_exited(&snapshot.status)
        || err_message.to_ascii_lowercase().contains("broken pipe")
}

fn format_model_kernel_failure_details(
    reason: &str,
    stream_error: Option<&str>,
    snapshot: &KernelDebugSnapshot,
) -> String {
    let payload = serde_json::json!({
        "reason": reason,
        "stream_error": stream_error
            .map(|err| truncate_utf8_prefix_by_bytes(err, JS_REPL_MODEL_DIAG_ERROR_MAX_BYTES)),
        "kernel_pid": snapshot.pid,
        "kernel_status": snapshot.status,
        "kernel_stderr_tail": truncate_utf8_prefix_by_bytes(
            &snapshot.stderr_tail,
            JS_REPL_MODEL_DIAG_STDERR_MAX_BYTES,
        ),
    });
    let encoded = serde_json::to_string(&payload)
        .unwrap_or_else(|err| format!(r#"{{"reason":"serialization_error","error":"{err}"}}"#));
    format!("js_repl diagnostics: {encoded}")
}

pub(super) fn with_model_kernel_failure_message(
    base_message: &str,
    reason: &str,
    stream_error: Option<&str>,
    snapshot: &KernelDebugSnapshot,
) -> String {
    format!(
        "{base_message}\n\n{}",
        format_model_kernel_failure_details(reason, stream_error, snapshot)
    )
}

fn join_outputs(stdout: &str, stderr: &str) -> String {
    if stdout.is_empty() {
        stderr.to_string()
    } else if stderr.is_empty() {
        stdout.to_string()
    } else {
        format!("{stdout}\n{stderr}")
    }
}

pub(super) fn build_js_repl_exec_output(
    output: &str,
    error: Option<&str>,
    duration: Duration,
    timed_out: bool,
) -> ExecToolCallOutput {
    let stdout = output.to_string();
    let stderr = error.unwrap_or("").to_string();
    let aggregated_output = join_outputs(&stdout, &stderr);
    ExecToolCallOutput {
        exit_code: if error.is_some() { 1 } else { 0 },
        stdout: StreamOutput::new(stdout),
        stderr: StreamOutput::new(stderr),
        aggregated_output: StreamOutput::new(aggregated_output),
        duration,
        timed_out,
    }
}

pub(super) fn emitted_image_content_item(
    turn: &TurnContext,
    image_url: String,
    detail: Option<ImageDetail>,
) -> FunctionCallOutputContentItem {
    FunctionCallOutputContentItem::InputImage {
        image_url,
        detail: detail.or_else(|| default_output_image_detail_for_turn(turn)),
    }
}

pub(super) fn drain_broadcast_lines(buffer: &mut Vec<u8>) -> Vec<String> {
    let mut lines = Vec::new();
    loop {
        let Some(pos) = buffer.iter().position(|byte| *byte == b'\n') else {
            break;
        };
        let line = buffer.drain(..=pos).collect::<Vec<_>>();
        lines.push(decode_broadcast_line(&line));
    }
    lines
}

pub(super) fn finish_broadcast_line(buffer: &mut Vec<u8>) -> Option<String> {
    if buffer.is_empty() {
        None
    } else {
        Some(decode_broadcast_line(&std::mem::take(buffer)))
    }
}

fn decode_broadcast_line(line: &[u8]) -> String {
    let line = String::from_utf8_lossy(line);
    line.trim_end_matches(['\n', '\r']).to_string()
}

pub(super) fn validate_emitted_image_url(image_url: &str) -> Result<(), String> {
    if image_url
        .get(..5)
        .is_some_and(|scheme| scheme.eq_ignore_ascii_case("data:"))
    {
        Ok(())
    } else {
        Err("codex.emitImage only accepts data URLs".to_string())
    }
}

fn default_output_image_detail_for_turn(turn: &TurnContext) -> Option<ImageDetail> {
    (turn.config.features.enabled(Feature::ImageDetailOriginal)
        && turn.model_info.supports_image_detail_original)
        .then_some(ImageDetail::Original)
}

pub(super) fn build_exec_result_content_items(
    output: String,
    content_items: Vec<FunctionCallOutputContentItem>,
) -> Vec<FunctionCallOutputContentItem> {
    let mut all_content_items = Vec::with_capacity(content_items.len() + 1);
    all_content_items.push(FunctionCallOutputContentItem::InputText { text: output });
    all_content_items.extend(content_items);
    all_content_items
}

pub(super) fn split_exec_result_content_items(
    mut content_items: Vec<FunctionCallOutputContentItem>,
) -> (String, Vec<FunctionCallOutputContentItem>) {
    match content_items.first() {
        Some(FunctionCallOutputContentItem::InputText { .. }) => {
            let FunctionCallOutputContentItem::InputText { text } = content_items.remove(0) else {
                unreachable!("first content item should be input_text");
            };
            (text, content_items)
        }
        Some(FunctionCallOutputContentItem::InputImage { .. }) | None => {
            (String::new(), content_items)
        }
    }
}

pub(super) fn clamp_poll_ms(value: Option<u64>) -> u64 {
    value
        .unwrap_or(JS_REPL_POLL_DEFAULT_MS)
        .clamp(JS_REPL_POLL_MIN_MS, JS_REPL_POLL_MAX_MS)
}
