use anyhow::Result;
use chrono::DateTime;
use chrono::Utc;
use serde_json::Value;
use sqlx::Row;
use sqlx::sqlite::SqliteRow;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentJobStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl AgentJobStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            AgentJobStatus::Pending => "pending",
            AgentJobStatus::Running => "running",
            AgentJobStatus::Completed => "completed",
            AgentJobStatus::Failed => "failed",
            AgentJobStatus::Cancelled => "cancelled",
        }
    }

    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "pending" => Ok(Self::Pending),
            "running" => Ok(Self::Running),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            "cancelled" => Ok(Self::Cancelled),
            _ => Err(anyhow::anyhow!("invalid agent job status: {value}")),
        }
    }

    pub fn is_final(self) -> bool {
        matches!(
            self,
            AgentJobStatus::Completed | AgentJobStatus::Failed | AgentJobStatus::Cancelled
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentJobItemStatus {
    Pending,
    Running,
    Completed,
    Failed,
}

impl AgentJobItemStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            AgentJobItemStatus::Pending => "pending",
            AgentJobItemStatus::Running => "running",
            AgentJobItemStatus::Completed => "completed",
            AgentJobItemStatus::Failed => "failed",
        }
    }

    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "pending" => Ok(Self::Pending),
            "running" => Ok(Self::Running),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            _ => Err(anyhow::anyhow!("invalid agent job item status: {value}")),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AgentJob {
    pub id: String,
    pub name: String,
    pub status: AgentJobStatus,
    pub instruction: String,
    pub auto_export: bool,
    pub output_schema_json: Option<Value>,
    pub input_headers: Vec<String>,
    pub input_csv_path: String,
    pub output_csv_path: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AgentJobItem {
    pub job_id: String,
    pub item_id: String,
    pub row_index: i64,
    pub source_id: Option<String>,
    pub row_json: Value,
    pub status: AgentJobItemStatus,
    pub assigned_thread_id: Option<String>,
    pub attempt_count: i64,
    pub result_json: Option<Value>,
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub reported_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentJobProgress {
    pub total_items: usize,
    pub pending_items: usize,
    pub running_items: usize,
    pub completed_items: usize,
    pub failed_items: usize,
}

#[derive(Debug, Clone)]
pub struct AgentJobCreateParams {
    pub id: String,
    pub name: String,
    pub instruction: String,
    pub auto_export: bool,
    pub output_schema_json: Option<Value>,
    pub input_headers: Vec<String>,
    pub input_csv_path: String,
    pub output_csv_path: String,
}

#[derive(Debug, Clone)]
pub struct AgentJobItemCreateParams {
    pub item_id: String,
    pub row_index: i64,
    pub source_id: Option<String>,
    pub row_json: Value,
}

#[derive(Debug)]
pub(crate) struct AgentJobRow {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) status: String,
    pub(crate) instruction: String,
    pub(crate) auto_export: i64,
    pub(crate) output_schema_json: Option<String>,
    pub(crate) input_headers_json: String,
    pub(crate) input_csv_path: String,
    pub(crate) output_csv_path: String,
    pub(crate) created_at: i64,
    pub(crate) updated_at: i64,
    pub(crate) started_at: Option<i64>,
    pub(crate) completed_at: Option<i64>,
    pub(crate) last_error: Option<String>,
}

impl AgentJobRow {
    pub(crate) fn try_from_row(row: &SqliteRow) -> Result<Self> {
        Ok(Self {
            id: row.try_get("id")?,
            name: row.try_get("name")?,
            status: row.try_get("status")?,
            instruction: row.try_get("instruction")?,
            auto_export: row.try_get("auto_export")?,
            output_schema_json: row.try_get("output_schema_json")?,
            input_headers_json: row.try_get("input_headers_json")?,
            input_csv_path: row.try_get("input_csv_path")?,
            output_csv_path: row.try_get("output_csv_path")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
            started_at: row.try_get("started_at")?,
            completed_at: row.try_get("completed_at")?,
            last_error: row.try_get("last_error")?,
        })
    }
}

impl TryFrom<AgentJobRow> for AgentJob {
    type Error = anyhow::Error;

    fn try_from(value: AgentJobRow) -> Result<Self, Self::Error> {
        let output_schema_json = value
            .output_schema_json
            .as_deref()
            .map(serde_json::from_str)
            .transpose()?;
        let input_headers = serde_json::from_str(value.input_headers_json.as_str())?;
        Ok(Self {
            id: value.id,
            name: value.name,
            status: AgentJobStatus::parse(value.status.as_str())?,
            instruction: value.instruction,
            auto_export: value.auto_export != 0,
            output_schema_json,
            input_headers,
            input_csv_path: value.input_csv_path,
            output_csv_path: value.output_csv_path,
            created_at: epoch_seconds_to_datetime(value.created_at)?,
            updated_at: epoch_seconds_to_datetime(value.updated_at)?,
            started_at: value
                .started_at
                .map(epoch_seconds_to_datetime)
                .transpose()?,
            completed_at: value
                .completed_at
                .map(epoch_seconds_to_datetime)
                .transpose()?,
            last_error: value.last_error,
        })
    }
}

#[derive(Debug)]
pub(crate) struct AgentJobItemRow {
    pub(crate) job_id: String,
    pub(crate) item_id: String,
    pub(crate) row_index: i64,
    pub(crate) source_id: Option<String>,
    pub(crate) row_json: String,
    pub(crate) status: String,
    pub(crate) assigned_thread_id: Option<String>,
    pub(crate) attempt_count: i64,
    pub(crate) result_json: Option<String>,
    pub(crate) last_error: Option<String>,
    pub(crate) created_at: i64,
    pub(crate) updated_at: i64,
    pub(crate) completed_at: Option<i64>,
    pub(crate) reported_at: Option<i64>,
}

impl AgentJobItemRow {
    pub(crate) fn try_from_row(row: &SqliteRow) -> Result<Self> {
        Ok(Self {
            job_id: row.try_get("job_id")?,
            item_id: row.try_get("item_id")?,
            row_index: row.try_get("row_index")?,
            source_id: row.try_get("source_id")?,
            row_json: row.try_get("row_json")?,
            status: row.try_get("status")?,
            assigned_thread_id: row.try_get("assigned_thread_id")?,
            attempt_count: row.try_get("attempt_count")?,
            result_json: row.try_get("result_json")?,
            last_error: row.try_get("last_error")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
            completed_at: row.try_get("completed_at")?,
            reported_at: row.try_get("reported_at")?,
        })
    }
}

impl TryFrom<AgentJobItemRow> for AgentJobItem {
    type Error = anyhow::Error;

    fn try_from(value: AgentJobItemRow) -> Result<Self, Self::Error> {
        Ok(Self {
            job_id: value.job_id,
            item_id: value.item_id,
            row_index: value.row_index,
            source_id: value.source_id,
            row_json: serde_json::from_str(value.row_json.as_str())?,
            status: AgentJobItemStatus::parse(value.status.as_str())?,
            assigned_thread_id: value.assigned_thread_id,
            attempt_count: value.attempt_count,
            result_json: value
                .result_json
                .as_deref()
                .map(serde_json::from_str)
                .transpose()?,
            last_error: value.last_error,
            created_at: epoch_seconds_to_datetime(value.created_at)?,
            updated_at: epoch_seconds_to_datetime(value.updated_at)?,
            completed_at: value
                .completed_at
                .map(epoch_seconds_to_datetime)
                .transpose()?,
            reported_at: value
                .reported_at
                .map(epoch_seconds_to_datetime)
                .transpose()?,
        })
    }
}

fn epoch_seconds_to_datetime(secs: i64) -> Result<DateTime<Utc>> {
    DateTime::<Utc>::from_timestamp(secs, 0)
        .ok_or_else(|| anyhow::anyhow!("invalid unix timestamp: {secs}"))
}
