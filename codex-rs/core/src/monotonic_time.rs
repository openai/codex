use std::time::Duration;

#[cfg(not(target_arch = "wasm32"))]
pub(crate) use std::time::Instant;

#[cfg(target_arch = "wasm32")]
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub(crate) struct Instant {
    millis_since_epoch: f64,
}

#[cfg(target_arch = "wasm32")]
impl Instant {
    pub(crate) fn now() -> Self {
        Self {
            millis_since_epoch: js_sys::Date::now(),
        }
    }

    pub(crate) fn elapsed(self) -> Duration {
        Self::now().saturating_duration_since(self)
    }

    pub(crate) fn duration_since(self, earlier: Self) -> Duration {
        self.saturating_duration_since(earlier)
    }

    pub(crate) fn saturating_duration_since(self, earlier: Self) -> Duration {
        duration_from_millis((self.millis_since_epoch - earlier.millis_since_epoch).max(0.0))
    }
}

#[cfg(target_arch = "wasm32")]
fn duration_from_millis(millis: f64) -> Duration {
    if !millis.is_finite() || millis <= 0.0 {
        return Duration::ZERO;
    }

    Duration::from_secs_f64(millis / 1_000.0)
}
