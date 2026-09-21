use serde::Serialize;
use sqlx_macros::FromRow;

#[derive(Clone, Debug, Serialize)]
pub struct LogEntry {
    pub ts: i64,
    pub ts_nanos: i64,
    pub level: String,
    pub target: String,
    pub message: Option<String>,
    pub feedback_log_body: Option<String>,
    pub thread_id: Option<String>,
    pub process_uuid: Option<String>,
    pub module_path: Option<String>,
    pub file: Option<String>,
    pub line: Option<i64>,
}

impl LogEntry {
    pub(crate) fn estimated_bytes(&self) -> i64 {
        let feedback_log_body = self.feedback_log_body.as_ref().or(self.message.as_ref());
        feedback_log_body.map_or(0, String::len) as i64
            + self.level.len() as i64
            + self.target.len() as i64
            + self.module_path.as_ref().map_or(0, String::len) as i64
            + self.file.as_ref().map_or(0, String::len) as i64
    }
}

#[derive(Clone, Debug, FromRow)]
pub struct LogRow {
    pub id: i64,
    pub ts: i64,
    pub ts_nanos: i64,
    pub level: String,
    pub target: String,
    pub message: Option<String>,
    pub thread_id: Option<String>,
    pub process_uuid: Option<String>,
    pub file: Option<String>,
    pub line: Option<i64>,
}

#[derive(Clone, Debug, Default)]
pub struct LogQuery {
    pub levels_upper: Vec<String>,
    pub from_ts: Option<i64>,
    pub to_ts: Option<i64>,
    pub module_like: Vec<String>,
    pub file_like: Vec<String>,
    pub thread_ids: Vec<String>,
    pub search: Option<String>,
    pub include_threadless: bool,
    pub after_id: Option<i64>,
    pub limit: Option<usize>,
    pub descending: bool,
}
