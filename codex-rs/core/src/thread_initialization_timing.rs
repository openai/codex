use std::time::Duration;
use std::time::Instant;

use codex_analytics::ThreadInitializationMode;
use codex_analytics::ThreadInitializationTimingFact;

#[derive(Debug)]
pub(crate) struct ThreadInitializationTiming {
    started_at: Instant,
    phase_started_at: Instant,
    prepare_duration_ms: u64,
    spawn_duration_ms: u64,
}

impl ThreadInitializationTiming {
    pub(crate) fn start() -> Self {
        let started_at = Instant::now();
        Self {
            started_at,
            phase_started_at: started_at,
            prepare_duration_ms: 0,
            spawn_duration_ms: 0,
        }
    }

    pub(crate) fn mark_prepare_completed(&mut self) {
        self.prepare_duration_ms = self.finish_phase();
    }

    pub(crate) fn mark_spawn_completed(&mut self) {
        self.spawn_duration_ms = self.finish_phase();
    }

    pub(crate) fn finish(
        mut self,
        thread_id: String,
        initialization_mode: ThreadInitializationMode,
    ) -> ThreadInitializationTimingFact {
        let finalize_duration_ms = self.finish_phase();
        ThreadInitializationTimingFact {
            thread_id,
            initialization_mode,
            duration_ms: duration_ms(self.started_at.elapsed()),
            prepare_duration_ms: self.prepare_duration_ms,
            spawn_duration_ms: self.spawn_duration_ms,
            finalize_duration_ms,
        }
    }

    fn finish_phase(&mut self) -> u64 {
        let duration_ms = duration_ms(self.phase_started_at.elapsed());
        self.phase_started_at = Instant::now();
        duration_ms
    }
}

fn duration_ms(duration: Duration) -> u64 {
    duration.as_millis().try_into().unwrap_or(u64::MAX)
}
