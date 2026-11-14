//! Assigns first-observed timestamps to realtime records for a single thread owner.

use codex_history::RolloutItem;

#[derive(Debug, Default)]
pub(crate) struct ObservationClock {
    last_assigned_ms: Option<i64>,
}

impl ObservationClock {
    pub(crate) fn restore(&mut self, items: &[RolloutItem]) {
        let floor = items
            .iter()
            .filter_map(|item| match item {
                RolloutItem::RealtimeItem(item) => item.observed_at_ms,
                RolloutItem::Compacted(compacted) => compacted
                    .resume_metadata
                    .as_ref()
                    .and_then(|metadata| metadata.observation_clock_floor_ms),
                _ => None,
            })
            .max();
        if let Some(floor) = floor {
            self.observe(floor);
        }
    }

    pub(crate) fn now_ms(&mut self) -> i64 {
        self.observe(crate::turn_timing::now_unix_timestamp_ms())
    }

    pub(crate) fn floor_ms(&self) -> Option<i64> {
        self.last_assigned_ms
    }

    pub(crate) fn observe(&mut self, wall_clock_ms: i64) -> i64 {
        let time = self
            .last_assigned_ms
            .unwrap_or(wall_clock_ms)
            .max(wall_clock_ms);
        self.last_assigned_ms = Some(time);
        time
    }
}

#[cfg(test)]
#[path = "observation_clock_tests.rs"]
mod tests;
