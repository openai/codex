use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

use codex_apply_patch::ApplyPatchArgs;
use codex_apply_patch::Hunk;
use codex_apply_patch::parse_patch_streaming;
use codex_protocol::ThreadId;
use codex_protocol::protocol::ApplyPatchChangesDeltaEvent;
use codex_protocol::protocol::FileChange;
use rand::Rng;
use tracing::error;

use codex_shell_command::parse_command::shlex_join;

const INITIAL_DELAY_MS: u64 = 200;
const BACKOFF_FACTOR: f64 = 2.0;

/// Emit structured feedback metadata as key/value pairs.
///
/// This logs a tracing event with `target: "feedback_tags"`. If
/// `codex_feedback::CodexFeedback::metadata_layer()` is installed, these fields are captured and
/// later attached as tags when feedback is uploaded.
///
/// Values are wrapped with [`tracing::field::DebugValue`], so the expression only needs to
/// implement [`std::fmt::Debug`].
///
/// Example:
///
/// ```rust
/// codex_core::feedback_tags!(model = "gpt-5", cached = true);
/// codex_core::feedback_tags!(provider = provider_id, request_id = request_id);
/// ```
#[macro_export]
macro_rules! feedback_tags {
    ($( $key:ident = $value:expr ),+ $(,)?) => {
        ::tracing::info!(
            target: "feedback_tags",
            $( $key = ::tracing::field::debug(&$value) ),+
        );
    };
}

struct Auth401FeedbackSnapshot<'a> {
    request_id: &'a str,
    cf_ray: &'a str,
    error: &'a str,
    error_code: &'a str,
}

impl<'a> Auth401FeedbackSnapshot<'a> {
    fn from_optional_fields(
        request_id: Option<&'a str>,
        cf_ray: Option<&'a str>,
        error: Option<&'a str>,
        error_code: Option<&'a str>,
    ) -> Self {
        Self {
            request_id: request_id.unwrap_or(""),
            cf_ray: cf_ray.unwrap_or(""),
            error: error.unwrap_or(""),
            error_code: error_code.unwrap_or(""),
        }
    }
}

pub(crate) fn emit_feedback_auth_recovery_tags(
    auth_recovery_mode: &str,
    auth_recovery_phase: &str,
    auth_recovery_outcome: &str,
    auth_request_id: Option<&str>,
    auth_cf_ray: Option<&str>,
    auth_error: Option<&str>,
    auth_error_code: Option<&str>,
) {
    let auth_401 = Auth401FeedbackSnapshot::from_optional_fields(
        auth_request_id,
        auth_cf_ray,
        auth_error,
        auth_error_code,
    );
    feedback_tags!(
        auth_recovery_mode = auth_recovery_mode,
        auth_recovery_phase = auth_recovery_phase,
        auth_recovery_outcome = auth_recovery_outcome,
        auth_401_request_id = auth_401.request_id,
        auth_401_cf_ray = auth_401.cf_ray,
        auth_401_error = auth_401.error,
        auth_401_error_code = auth_401.error_code
    );
}

pub fn backoff(attempt: u64) -> Duration {
    let exp = BACKOFF_FACTOR.powi(attempt.saturating_sub(1) as i32);
    let base = (INITIAL_DELAY_MS as f64 * exp) as u64;
    let jitter = rand::rng().random_range(0.9..1.1);
    Duration::from_millis((base as f64 * jitter) as u64)
}

pub(crate) fn error_or_panic(message: impl std::string::ToString) {
    if cfg!(debug_assertions) {
        panic!("{}", message.to_string());
    } else {
        error!("{}", message.to_string());
    }
}

pub fn resolve_path(base: &Path, path: &PathBuf) -> PathBuf {
    if path.is_absolute() {
        path.clone()
    } else {
        base.join(path)
    }
}

/// Trim a thread name and return `None` if it is empty after trimming.
pub fn normalize_thread_name(name: &str) -> Option<String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

pub fn resume_command(thread_name: Option<&str>, thread_id: Option<ThreadId>) -> Option<String> {
    let resume_target = thread_name
        .filter(|name| !name.is_empty())
        .map(str::to_string)
        .or_else(|| thread_id.map(|thread_id| thread_id.to_string()));
    resume_target.map(|target| {
        let needs_double_dash = target.starts_with('-');
        let escaped = shlex_join(&[target]);
        if needs_double_dash {
            format!("codex resume -- {escaped}")
        } else {
            format!("codex resume {escaped}")
        }
    })
}

#[derive(Default)]
pub(crate) struct ApplyPatchInputStream {
    call_id: Option<String>,
    input: String,
    last_progress: Option<Vec<Hunk>>,
}

impl ApplyPatchInputStream {
    pub(crate) fn push_delta(
        &mut self,
        call_id: String,
        delta: &str,
    ) -> Option<ApplyPatchChangesDeltaEvent> {
        if self.call_id.as_deref() != Some(call_id.as_str()) {
            self.call_id = Some(call_id.clone());
            self.input.clear();
            self.last_progress = None;
        }

        self.input.push_str(delta);

        let patch_text = apply_patch_text_from_tool_input_prefix(&self.input)?;
        let ApplyPatchArgs { hunks, .. } = parse_patch_streaming(&patch_text).ok()?;
        if hunks.is_empty() {
            return None;
        }
        if self.last_progress.as_ref() == Some(&hunks) {
            return None;
        }

        let active_path = hunks.last().map(Hunk::path).map(Path::to_path_buf);
        let changes = convert_apply_patch_hunks_to_protocol(&hunks);
        self.last_progress = Some(hunks);
        Some(ApplyPatchChangesDeltaEvent {
            call_id,
            changes,
            active_path,
        })
    }
}

fn convert_apply_patch_hunks_to_protocol(hunks: &[Hunk]) -> HashMap<PathBuf, FileChange> {
    hunks
        .iter()
        .map(|hunk| {
            let path = hunk_source_path(hunk).to_path_buf();
            let change = match hunk {
                Hunk::AddFile { contents, .. } => FileChange::Add {
                    content: contents.clone(),
                },
                Hunk::DeleteFile { .. } => FileChange::Delete {
                    content: String::new(),
                },
                Hunk::UpdateFile {
                    chunks, move_path, ..
                } => FileChange::Update {
                    unified_diff: format_update_chunks_for_progress(chunks),
                    move_path: move_path.clone(),
                },
            };
            (path, change)
        })
        .collect()
}

fn hunk_source_path(hunk: &Hunk) -> &Path {
    match hunk {
        Hunk::AddFile { path, .. } | Hunk::DeleteFile { path } | Hunk::UpdateFile { path, .. } => {
            path
        }
    }
}

fn format_update_chunks_for_progress(chunks: &[codex_apply_patch::UpdateFileChunk]) -> String {
    let mut unified_diff = String::new();
    for chunk in chunks {
        match &chunk.change_context {
            Some(context) => {
                unified_diff.push_str("@@ ");
                unified_diff.push_str(context);
                unified_diff.push('\n');
            }
            None => {
                unified_diff.push_str("@@");
                unified_diff.push('\n');
            }
        }
        for line in &chunk.old_lines {
            unified_diff.push('-');
            unified_diff.push_str(line);
            unified_diff.push('\n');
        }
        for line in &chunk.new_lines {
            unified_diff.push('+');
            unified_diff.push_str(line);
            unified_diff.push('\n');
        }
        if chunk.is_end_of_file {
            unified_diff.push_str("*** End of File");
            unified_diff.push('\n');
        }
    }
    unified_diff
}

fn apply_patch_text_from_tool_input_prefix(input: &str) -> Option<String> {
    let trimmed = input.trim_start();
    if trimmed.starts_with('{') {
        extract_json_input_prefix(trimmed)
    } else {
        Some(input.to_string())
    }
}

fn extract_json_input_prefix(input: &str) -> Option<String> {
    let key = "\"input\"";
    let mut search_start = 0;
    while let Some(offset) = input[search_start..].find(key) {
        let key_start = search_start + offset;
        let mut index = key_start + key.len();
        index = skip_json_whitespace(input, index);
        if input.as_bytes().get(index) != Some(&b':') {
            search_start = key_start + key.len();
            continue;
        }
        index += 1;
        index = skip_json_whitespace(input, index);
        if input.as_bytes().get(index) != Some(&b'"') {
            search_start = key_start + key.len();
            continue;
        }
        return Some(decode_json_string_prefix(&input[index + 1..]));
    }
    None
}

fn skip_json_whitespace(input: &str, mut index: usize) -> usize {
    while let Some(byte) = input.as_bytes().get(index)
        && byte.is_ascii_whitespace()
    {
        index += 1;
    }
    index
}

fn decode_json_string_prefix(input: &str) -> String {
    let mut decoded = String::new();
    let mut chars = input.chars();
    while let Some(ch) = chars.next() {
        match ch {
            '"' => break,
            '\\' => {
                let Some(escaped) = chars.next() else {
                    break;
                };
                match escaped {
                    '"' => decoded.push('"'),
                    '\\' => decoded.push('\\'),
                    '/' => decoded.push('/'),
                    'b' => decoded.push('\u{0008}'),
                    'f' => decoded.push('\u{000c}'),
                    'n' => decoded.push('\n'),
                    'r' => decoded.push('\r'),
                    't' => decoded.push('\t'),
                    'u' => {
                        let mut digits = String::with_capacity(4);
                        for _ in 0..4 {
                            let Some(digit) = chars.next() else {
                                return decoded;
                            };
                            digits.push(digit);
                        }
                        if let Ok(value) = u16::from_str_radix(&digits, 16)
                            && let Some(ch) = char::from_u32(u32::from(value))
                        {
                            decoded.push(ch);
                        }
                    }
                    other => decoded.push(other),
                }
            }
            other => decoded.push(other),
        }
    }
    decoded
}

#[cfg(test)]
#[path = "util_tests.rs"]
mod tests;
