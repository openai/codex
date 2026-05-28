use std::time::Duration;
use std::time::Instant;

use codex_analytics::ThreadInitializationTimingFact;

#[derive(Debug)]
pub(crate) struct ThreadInitializationTiming {
    phase_started_at: Instant,
    prepare_duration: Option<Duration>,
    spawn_duration: Option<Duration>,
    finalize_duration: Option<Duration>,
}

impl ThreadInitializationTiming {
    pub(crate) fn start() -> Self {
        Self {
            phase_started_at: Instant::now(),
            prepare_duration: None,
            spawn_duration: None,
            finalize_duration: None,
        }
    }

    pub(crate) fn mark_prepare_completed(&mut self) {
        self.prepare_duration = Some(self.finish_phase());
    }

    pub(crate) fn mark_spawn_completed(&mut self) {
        self.spawn_duration = Some(self.finish_phase());
    }

    pub(crate) fn mark_finalize_completed(&mut self) {
        self.finalize_duration = Some(self.finish_phase());
    }

    pub(crate) fn into_fact(self, thread_id: String) -> ThreadInitializationTimingFact {
        let prepare_duration = self.prepare_duration.unwrap_or_default();
        let spawn_duration = self.spawn_duration.unwrap_or_default();
        let finalize_duration = self.finalize_duration.unwrap_or_default();
        ThreadInitializationTimingFact {
            thread_id,
            duration_ms: duration_ms(prepare_duration + spawn_duration + finalize_duration),
            prepare_duration_ms: duration_ms(prepare_duration),
            spawn_duration_ms: duration_ms(spawn_duration),
            finalize_duration_ms: duration_ms(finalize_duration),
        }
    }

    fn finish_phase(&mut self) -> Duration {
        let duration = self.phase_started_at.elapsed();
        self.phase_started_at = Instant::now();
        duration
    }
}

fn duration_ms(duration: Duration) -> u64 {
    duration.as_millis().try_into().unwrap_or(u64::MAX)
}
