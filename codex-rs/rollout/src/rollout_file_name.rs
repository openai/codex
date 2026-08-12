use codex_protocol::RolloutId;
use codex_protocol::ThreadId;
use time::OffsetDateTime;
use time::PrimitiveDateTime;
use time::format_description::FormatItem;
use time::macros::format_description;

use crate::compression;

/// Parsed canonical rollout basename.
///
/// Ordinary rollout filenames encode one ID, which is both the thread ID and rollout ID.
/// Filenames for reverted threads append an underscore and a distinct rollout ID after the stable
/// thread ID.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct RolloutFileName {
    timestamp: OffsetDateTime,
    thread_id: ThreadId,
    rollout_id: RolloutId,
}

#[cfg(test)]
#[path = "rollout_file_name_tests.rs"]
mod tests;

impl RolloutFileName {
    pub(crate) fn new(
        timestamp: OffsetDateTime,
        thread_id: ThreadId,
        rollout_id: RolloutId,
    ) -> Self {
        Self {
            timestamp,
            thread_id,
            rollout_id,
        }
    }

    pub(crate) fn parse(name: &str) -> Option<Self> {
        let name = compression::parse_rollout_file_name(name)?;
        let core = name.strip_prefix("rollout-")?.strip_suffix(".jsonl")?;
        let timestamp = core.get(..19)?;
        if core.get(19..20)? != "-" {
            return None;
        }
        let ids = core.get(20..)?;
        let (thread_id, rollout_id) = ids.split_once('_').unwrap_or((ids, ids));
        let thread_id = ThreadId::from_string(thread_id).ok()?;
        let rollout_id = RolloutId::from_string(rollout_id).ok()?;
        let format: &[FormatItem] =
            format_description!("[year]-[month]-[day]T[hour]-[minute]-[second]");
        let timestamp = PrimitiveDateTime::parse(timestamp, format)
            .ok()?
            .assume_utc();
        Some(Self {
            timestamp,
            thread_id,
            rollout_id,
        })
    }

    pub(crate) fn render(&self) -> Result<String, time::error::Format> {
        let format: &[FormatItem] =
            format_description!("[year]-[month]-[day]T[hour]-[minute]-[second]");
        let timestamp = self.timestamp.format(format)?;
        Ok(if self.thread_id == self.rollout_id {
            format!("rollout-{timestamp}-{}.jsonl", self.thread_id)
        } else {
            format!(
                "rollout-{timestamp}-{}_{}.jsonl",
                self.thread_id, self.rollout_id
            )
        })
    }

    pub(crate) fn timestamp(&self) -> OffsetDateTime {
        self.timestamp
    }

    pub(crate) fn thread_id(&self) -> ThreadId {
        self.thread_id
    }

    pub(crate) fn rollout_id(&self) -> RolloutId {
        self.rollout_id
    }
}
