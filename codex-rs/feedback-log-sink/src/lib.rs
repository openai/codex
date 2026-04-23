//! Remote feedback log sink protocol helpers.

use codex_state::LogEntry;

#[path = "proto/codex.feedback_log_sink.v1.rs"]
pub mod proto;

impl From<LogEntry> for proto::FeedbackLogEntry {
    fn from(entry: LogEntry) -> Self {
        Self {
            ts: entry.ts,
            ts_nanos: entry.ts_nanos,
            level: entry.level,
            target: entry.target,
            message: entry.message,
            feedback_log_body: entry.feedback_log_body,
            thread_id: entry.thread_id,
            process_uuid: entry.process_uuid,
            module_path: entry.module_path,
            file: entry.file,
            line: entry.line,
        }
    }
}

pub fn append_log_batch_request(
    entries: Vec<LogEntry>,
    source_process_uuid: impl Into<String>,
) -> proto::AppendLogBatchRequest {
    proto::AppendLogBatchRequest {
        entries: entries.into_iter().map(Into::into).collect(),
        source_process_uuid: source_process_uuid.into(),
    }
}

#[cfg(test)]
mod tests {
    use codex_state::LogEntry;
    use pretty_assertions::assert_eq;

    use super::append_log_batch_request;
    use super::proto;

    #[test]
    fn log_entry_to_proto_preserves_all_fields() {
        let entry = populated_log_entry();

        let actual = proto::FeedbackLogEntry::from(entry);

        assert_eq!(actual, populated_feedback_log_entry());
    }

    #[test]
    fn log_entry_to_proto_preserves_absent_optional_fields() {
        let entry = LogEntry {
            ts: 1700000000,
            ts_nanos: 42,
            level: "WARN".to_string(),
            target: "codex::target".to_string(),
            message: None,
            feedback_log_body: None,
            thread_id: None,
            process_uuid: None,
            module_path: None,
            file: None,
            line: None,
        };

        let actual = proto::FeedbackLogEntry::from(entry);

        assert_eq!(
            actual,
            proto::FeedbackLogEntry {
                ts: 1700000000,
                ts_nanos: 42,
                level: "WARN".to_string(),
                target: "codex::target".to_string(),
                message: None,
                feedback_log_body: None,
                thread_id: None,
                process_uuid: None,
                module_path: None,
                file: None,
                line: None,
            }
        );
    }

    #[test]
    fn append_log_batch_request_sets_source_process_uuid_and_entries() {
        let actual = append_log_batch_request(vec![populated_log_entry()], "source-process");

        assert_eq!(
            actual,
            proto::AppendLogBatchRequest {
                entries: vec![populated_feedback_log_entry()],
                source_process_uuid: "source-process".to_string(),
            }
        );
    }

    fn populated_log_entry() -> LogEntry {
        LogEntry {
            ts: 1700000000,
            ts_nanos: 123456789,
            level: "INFO".to_string(),
            target: "codex::feedback".to_string(),
            message: Some("captured message".to_string()),
            feedback_log_body: Some("structured body".to_string()),
            thread_id: Some("thread-1".to_string()),
            process_uuid: Some("process-entry".to_string()),
            module_path: Some("codex_state::log_db".to_string()),
            file: Some("state/src/log_db.rs".to_string()),
            line: Some(123),
        }
    }

    fn populated_feedback_log_entry() -> proto::FeedbackLogEntry {
        proto::FeedbackLogEntry {
            ts: 1700000000,
            ts_nanos: 123456789,
            level: "INFO".to_string(),
            target: "codex::feedback".to_string(),
            message: Some("captured message".to_string()),
            feedback_log_body: Some("structured body".to_string()),
            thread_id: Some("thread-1".to_string()),
            process_uuid: Some("process-entry".to_string()),
            module_path: Some("codex_state::log_db".to_string()),
            file: Some("state/src/log_db.rs".to_string()),
            line: Some(123),
        }
    }
}
