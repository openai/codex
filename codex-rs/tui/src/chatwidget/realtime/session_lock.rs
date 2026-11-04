//! Owns one desktop observer across a call's startup retry and widget parking.
//! Activation aborts local media before the TUI handles the wakeup; only current state may stop a call.

use super::ChatWidget;
use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::session_lock::Activation;
use codex_protocol::ThreadId;
use futures::FutureExt;
use futures::future::AbortHandle;
use futures::future::AbortRegistration;
use tokio::task::JoinHandle;

#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    Initializing,
    Ready,
    Stopped(&'static str),
}

pub(super) struct SessionLock {
    state: tokio::sync::watch::Receiver<State>,
    task: JoinHandle<()>,
}

impl SessionLock {
    pub(super) fn observe(
        thread_id: ThreadId,
        tx: AppEventSender,
        setup: impl Future<Output = anyhow::Result<Activation>> + Send + 'static,
    ) -> Self {
        let (state_tx, state) = tokio::sync::watch::channel(State::Initializing);
        let task = tokio::spawn(async move {
            let result = match setup.await {
                Ok(mut activation) => match activation.as_mut().now_or_never() {
                    Some(result) => result,
                    None => {
                        state_tx.send_replace(State::Ready);
                        activation.await
                    }
                },
                Err(error) => Err(error),
            };
            let message = match result {
                Ok(()) => "Voice stopped because your session locked.",
                Err(error) => {
                    tracing::debug!(%error, "session lock observer failed");
                    "Voice conversation stopped because session lock monitoring failed."
                }
            };
            state_tx.send_replace(State::Stopped(message));
            tx.send(AppEvent::RealtimeSessionLockStopped { thread_id });
        });
        Self { state, task }
    }

    /// Gate the attempt on the initial snapshot and abort its media actor on activation,
    /// even if the TUI is still waiting for an app-server operation.
    pub(super) fn start_media(
        &self,
        start: impl FnOnce(AbortRegistration) + Send + 'static,
    ) -> AbortHandle {
        let mut state = self.state.clone();
        let (abort, registration) = AbortHandle::new_pair();
        let media = abort.clone();
        tokio::spawn(async move {
            let ready = state
                .wait_for(|state| *state != State::Initializing)
                .await
                .is_ok_and(|state| *state == State::Ready);
            if ready {
                std::thread::spawn(move || start(registration));
                let _ = state.changed().await;
            }
            media.abort();
        });
        abort
    }
}

impl Drop for SessionLock {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl ChatWidget {
    pub(crate) fn stop_if_session_locked(&mut self) {
        let Some(observer) = &self.realtime_conversation.session_lock else {
            return;
        };
        let State::Stopped(message) = *observer.state.borrow() else {
            return;
        };
        self.stop_realtime_conversation();
        self.add_realtime_error(message.into());
    }
}
