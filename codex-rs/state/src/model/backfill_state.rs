use anyhow::Result;
use chrono::DateTime;
use chrono::Utc;
use sqlx::Row;
use sqlx::sqlite::SqliteRow;

/// Persisted lifecycle state for rollout metadata backfill.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackfillState {
    /// Current lifecycle status.
    pub status: BackfillStatus,
    /// Last processed rollout watermark.
    pub last_watermark: Option<String>,
    /// Last successful completion time.
    pub last_success_at: Option<DateTime<Utc>>,
}

impl Default for BackfillState {
    fn default() -> Self {
        Self {
            status: BackfillStatus::Pending,
            last_watermark: None,
            last_success_at: None,
        }
    }
}

impl BackfillState {
    pub(crate) fn try_from_row(row: &SqliteRow) -> Result<Self> {
        let status: String = row.try_get("status")?;
        let last_success_at = row
            .try_get::<Option<i64>, _>("last_success_at")?
            .map(epoch_seconds_to_datetime)
            .transpose()?;
        Ok(Self {
            status: BackfillStatus::parse(status.as_str())?,
            last_watermark: row.try_get("last_watermark")?,
            last_success_at,
        })
    }
}

/// Backfill lifecycle status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackfillStatus {
    Pending,
    Running,
    Complete,
}

impl BackfillStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            BackfillStatus::Pending => "pending",
            BackfillStatus::Running => "running",
            BackfillStatus::Complete => "complete",
        }
    }

    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "pending" => Ok(Self::Pending),
            "running" => Ok(Self::Running),
            "complete" => Ok(Self::Complete),
            _ => Err(anyhow::anyhow!("invalid backfill status: {value}")),
        }
    }
}

fn epoch_seconds_to_datetime(secs: i64) -> Result<DateTime<Utc>> {
    DateTime::<Utc>::from_timestamp(secs, 0)
        .ok_or_else(|| anyhow::anyhow!("invalid unix timestamp: {secs}"))
}
