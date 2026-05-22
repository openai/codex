use std::time::Duration;
use std::time::Instant;

use codex_analytics::ThreadStartTimingFact;

#[derive(Debug)]
pub(crate) enum ThreadStartTiming {
    Enabled {
        phase_started_at: Instant,
        prepare_duration: Option<Duration>,
        spawn_duration: Option<Duration>,
        finalize_duration: Option<Duration>,
    },
    Disabled,
}

impl ThreadStartTiming {
    pub(crate) fn start() -> Self {
        Self::Enabled {
            phase_started_at: Instant::now(),
            prepare_duration: None,
            spawn_duration: None,
            finalize_duration: None,
        }
    }

    pub(crate) fn mark_prepare_completed(&mut self) {
        let Some(duration) = self.finish_phase() else {
            return;
        };
        if let Self::Enabled {
            prepare_duration, ..
        } = self
        {
            *prepare_duration = Some(duration);
        }
    }

    pub(crate) fn mark_spawn_completed(&mut self) {
        let Some(duration) = self.finish_phase() else {
            return;
        };
        if let Self::Enabled { spawn_duration, .. } = self {
            *spawn_duration = Some(duration);
        }
    }

    pub(crate) fn mark_finalize_completed(&mut self) {
        let Some(duration) = self.finish_phase() else {
            return;
        };
        if let Self::Enabled {
            finalize_duration, ..
        } = self
        {
            *finalize_duration = Some(duration);
        }
    }

    pub(crate) fn into_fact(self, thread_id: String) -> Option<ThreadStartTimingFact> {
        match self {
            Self::Enabled {
                prepare_duration,
                spawn_duration,
                finalize_duration,
                ..
            } => {
                let prepare_duration = prepare_duration.unwrap_or_default();
                let spawn_duration = spawn_duration.unwrap_or_default();
                let finalize_duration = finalize_duration.unwrap_or_default();
                Some(ThreadStartTimingFact {
                    thread_id,
                    duration_ms: duration_ms(prepare_duration + spawn_duration + finalize_duration),
                    prepare_duration_ms: duration_ms(prepare_duration),
                    spawn_duration_ms: duration_ms(spawn_duration),
                    finalize_duration_ms: duration_ms(finalize_duration),
                })
            }
            Self::Disabled => None,
        }
    }

    fn finish_phase(&mut self) -> Option<Duration> {
        match self {
            Self::Enabled {
                phase_started_at, ..
            } => {
                let duration = phase_started_at.elapsed();
                *phase_started_at = Instant::now();
                Some(duration)
            }
            Self::Disabled => None,
        }
    }
}

fn duration_ms(duration: Duration) -> u64 {
    duration.as_millis().try_into().unwrap_or(u64::MAX)
}
