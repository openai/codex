use std::sync::Arc;
use std::sync::Mutex;

use tokio::sync::watch;

/// Coordinates user elicitations that pause tool-result delivery for a session.
///
/// Registrations are counted so concurrent elicitations keep the session paused until all of them
/// finish. Consumers can subscribe to pause timeout progress or wait before returning an already
/// captured result.
#[derive(Clone)]
pub(crate) struct ElicitationService {
    inner: Arc<Inner>,
}

struct Inner {
    state: Mutex<State>,
    paused: watch::Sender<bool>,
}

#[derive(Default)]
struct State {
    outstanding: i64,
}

pub(crate) struct ElicitationRegistration {
    service: ElicitationService,
}

impl ElicitationService {
    pub(crate) fn new() -> Self {
        let (paused, _paused_rx) = watch::channel(false);
        Self {
            inner: Arc::new(Inner {
                state: Mutex::new(State::default()),
                paused,
            }),
        }
    }

    pub(crate) fn register(&self) -> ElicitationRegistration {
        self.increment();
        ElicitationRegistration {
            service: self.clone(),
        }
    }

    fn increment(&self) {
        let mut state = self
            .inner
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let was_clear = state.outstanding == 0;
        assert_ne!(
            state.outstanding,
            i64::MAX,
            "outstanding elicitation count overflowed"
        );
        state.outstanding += 1;
        if was_clear {
            self.inner.paused.send_replace(true);
        }
    }

    pub(crate) fn subscribe(&self) -> watch::Receiver<bool> {
        self.inner.paused.subscribe()
    }

    pub(crate) async fn wait_until_clear(&self) {
        let mut paused = self.subscribe();
        let _ = paused.wait_for(|paused| !*paused).await;
    }

    fn decrement(&self) {
        let mut state = self
            .inner
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        assert!(
            state.outstanding > 0,
            "elicitation registration count underflowed"
        );
        state.outstanding -= 1;
        if state.outstanding == 0 {
            self.inner.paused.send_replace(false);
        }
    }
}

impl Drop for ElicitationRegistration {
    fn drop(&mut self) {
        self.service.decrement();
    }
}

#[cfg(test)]
#[path = "elicitation_tests.rs"]
mod tests;
