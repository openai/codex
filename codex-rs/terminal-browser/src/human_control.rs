use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::Weak;
use std::sync::atomic::Ordering;

use anyhow::Result;
use anyhow::anyhow;
use tokio::sync::mpsc;
use tokio::sync::oneshot;

use crate::actions;
use crate::actions::HumanMouseDispatchState;
use crate::input::BrowserKeyInput;
use crate::input::BrowserMouseInput;
use crate::input::BrowserMouseKind;
use crate::session::Inner;
use crate::session::TerminalBrowser;

const HUMAN_INPUT_CAPACITY: usize = 128;
const HUMAN_CONTROL_INPUT_CAPACITY: usize = 1;
const MAX_HUMAN_TEXT_BYTES: usize = 64 * 1024;

pub(crate) struct HumanInputSender {
    input: mpsc::Sender<QueuedHumanInput>,
    control: mpsc::Sender<QueuedHumanInput>,
}

pub(crate) struct HumanInputReceivers {
    input: mpsc::Receiver<QueuedHumanInput>,
    control: mpsc::Receiver<QueuedHumanInput>,
}

/// Captures the browser-control epoch at the time a UI transition is requested.
///
/// Hiding or closing the browser invalidates outstanding tokens so delayed tasks cannot retake
/// control after the user has dismissed the panel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HumanControlToken {
    generation: u64,
}

pub(crate) enum HumanControlStateTransition {
    Activate { generation: u64 },
    Deactivate { generation: u64 },
    Hide,
}

enum HumanInput {
    Key(BrowserKeyInput),
    Text(String),
    Mouse(BrowserMouseInput),
    ReleaseMouseButtons {
        completion_tx: oneshot::Sender<std::result::Result<(), String>>,
        after_release: HumanControlAfterRelease,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum HumanControlAfterRelease {
    Continue,
    End,
}

impl HumanInput {
    fn complete_release(self, result: std::result::Result<(), String>) {
        if let Self::ReleaseMouseButtons { completion_tx, .. } = self {
            let _ = completion_tx.send(result);
        }
    }
}

struct QueuedHumanInput {
    generation: u64,
    input: HumanInput,
}

impl Inner {
    pub(crate) fn transition_human_control(&self, transition: HumanControlStateTransition) -> bool {
        let mut transitioned = false;
        self.update_view(|view| match transition {
            HumanControlStateTransition::Activate { generation }
                if self.human_control_generation.load(Ordering::SeqCst) == generation =>
            {
                self.human_control.store(/*val*/ true, Ordering::SeqCst);
                view.human_control = true;
                view.visible = true;
                transitioned = true;
            }
            HumanControlStateTransition::Deactivate { generation }
                if self.human_control_generation.load(Ordering::SeqCst) == generation
                    && self.human_control.load(Ordering::SeqCst) =>
            {
                self.human_control_generation
                    .fetch_add(/*val*/ 1, Ordering::SeqCst);
                if self.human_control.swap(/*val*/ false, Ordering::SeqCst) {
                    self.human_handle_invalidation_pending
                        .store(/*val*/ true, Ordering::SeqCst);
                }
                view.human_control = false;
                transitioned = true;
            }
            HumanControlStateTransition::Hide => {
                self.human_control_generation
                    .fetch_add(/*val*/ 1, Ordering::SeqCst);
                if self.human_control.swap(/*val*/ false, Ordering::SeqCst) {
                    self.human_handle_invalidation_pending
                        .store(/*val*/ true, Ordering::SeqCst);
                }
                view.human_control = false;
                view.visible = false;
                transitioned = true;
            }
            HumanControlStateTransition::Activate { .. }
            | HumanControlStateTransition::Deactivate { .. } => {}
        });
        transitioned
    }
}

impl TerminalBrowser {
    pub(crate) fn human_input_channel() -> (HumanInputSender, HumanInputReceivers) {
        let (input_tx, input_rx) = mpsc::channel(HUMAN_INPUT_CAPACITY);
        let (control_tx, control_rx) = mpsc::channel(HUMAN_CONTROL_INPUT_CAPACITY);
        (
            HumanInputSender {
                input: input_tx,
                control: control_tx,
            },
            HumanInputReceivers {
                input: input_rx,
                control: control_rx,
            },
        )
    }

    pub fn is_human_control_active(&self) -> bool {
        self.inner.human_control.load(Ordering::SeqCst)
    }

    pub fn human_control_token(&self) -> HumanControlToken {
        HumanControlToken {
            generation: self.inner.human_control_generation.load(Ordering::SeqCst),
        }
    }

    pub async fn toggle_human_control(
        &self,
        token: HumanControlToken,
    ) -> Result<HumanControlToken> {
        self.inner
            .human_control_transition
            .compare_exchange(
                /*current*/ false,
                /*new*/ true,
                Ordering::SeqCst,
                Ordering::SeqCst,
            )
            .map_err(|_| anyhow!("browser_busy: browser control is already changing"))?;
        let _transition = HumanControlTransition {
            active: &self.inner.human_control_transition,
        };
        anyhow::ensure!(
            self.inner.human_control_generation.load(Ordering::SeqCst) == token.generation,
            "browser control transition was canceled"
        );
        if self.is_human_control_active() {
            self.end_human_control(token).await
        } else {
            self.begin_human_control(token.generation).await
        }
    }

    #[expect(
        clippy::await_holding_invalid_type,
        reason = "entering human control must serialize the live-session check with model actions"
    )]
    async fn begin_human_control(&self, control_generation: u64) -> Result<HumanControlToken> {
        if let Some(receiver) = self
            .inner
            .human_input_rx
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take()
        {
            tokio::spawn(run_human_input_worker(
                Arc::downgrade(&self.inner),
                receiver,
            ));
        }
        let _operation = self
            .inner
            .operation
            .try_lock()
            .map_err(|_| anyhow!("browser_busy: another terminal browser action is running"))?;
        self.flush_human_handle_invalidation().await;
        anyhow::ensure!(
            self.inner.session.lock().await.is_some(),
            "terminal browser is not open"
        );
        let process_running = self
            .inner
            .process
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
            .is_some_and(|process| !process.has_exited());
        let view_running = matches!(
            &self
                .inner
                .view
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .status,
            crate::screen::BrowserStatus::Running
        );
        anyhow::ensure!(
            process_running && view_running,
            "terminal browser is not running"
        );
        let active_generation = control_generation.wrapping_add(/*rhs*/ 1);
        self.inner
            .human_control_generation
            .compare_exchange(
                /*current*/ control_generation,
                /*new*/ active_generation,
                Ordering::SeqCst,
                Ordering::SeqCst,
            )
            .map_err(|_| anyhow!("browser control transition was canceled"))?;
        if !self
            .inner
            .transition_human_control(HumanControlStateTransition::Activate {
                generation: active_generation,
            })
        {
            anyhow::bail!("browser control transition was canceled");
        }
        Ok(HumanControlToken {
            generation: active_generation,
        })
    }

    #[expect(
        clippy::await_holding_invalid_type,
        reason = "ending human control must serialize handle invalidation with model actions"
    )]
    pub async fn end_human_control(&self, token: HumanControlToken) -> Result<HumanControlToken> {
        anyhow::ensure!(
            self.human_control_is_active_for(token.generation),
            "browser control transition was canceled"
        );
        self.release_human_mouse_buttons_for_generation(
            token.generation,
            HumanControlAfterRelease::End,
        )
        .await?;
        let _operation = self.inner.operation.lock().await;
        self.flush_human_handle_invalidation().await;
        Ok(HumanControlToken {
            generation: token.generation.wrapping_add(/*rhs*/ 1),
        })
    }

    #[expect(
        clippy::await_holding_invalid_type,
        reason = "deferred cleanup must serialize handle invalidation with model actions"
    )]
    pub async fn complete_human_control_cleanup(&self) {
        let _operation = self.inner.operation.lock().await;
        self.flush_human_handle_invalidation().await;
    }

    pub(crate) async fn flush_human_handle_invalidation(&self) {
        if !self
            .inner
            .human_handle_invalidation_pending
            .swap(/*val*/ false, Ordering::SeqCst)
        {
            return;
        }
        let mut session = self.inner.session.lock().await;
        if let Some(session) = session.as_mut() {
            session.handles.clear();
        }
    }

    pub fn send_human_key(&self, input: BrowserKeyInput) -> Result<()> {
        self.queue_human_input(HumanInput::Key(input))
    }

    pub fn send_human_text(&self, text: &str) -> Result<()> {
        anyhow::ensure!(
            text.len() <= MAX_HUMAN_TEXT_BYTES,
            "browser text input exceeds the 64 KiB limit"
        );
        self.queue_human_input(HumanInput::Text(text.to_string()))
    }

    pub fn send_human_mouse(&self, input: BrowserMouseInput) -> Result<()> {
        self.queue_human_input(HumanInput::Mouse(input))
    }

    /// Releases every mouse button currently held by the browser input worker.
    pub async fn release_human_mouse_buttons(&self) -> Result<()> {
        let generation = self.inner.human_control_generation.load(Ordering::SeqCst);
        self.release_human_mouse_buttons_for_generation(
            generation,
            HumanControlAfterRelease::Continue,
        )
        .await
    }

    async fn release_human_mouse_buttons_for_generation(
        &self,
        generation: u64,
        after_release: HumanControlAfterRelease,
    ) -> Result<()> {
        anyhow::ensure!(
            self.human_control_is_active_for(generation),
            "browser control transition was canceled"
        );
        let (completion_tx, completion_rx) = oneshot::channel();
        self.inner
            .human_input_tx
            .control
            .send(QueuedHumanInput {
                generation,
                input: HumanInput::ReleaseMouseButtons {
                    completion_tx,
                    after_release,
                },
            })
            .await
            .map_err(|_| anyhow!("terminal browser input worker stopped"))?;
        match completion_rx.await {
            Ok(Ok(())) => Ok(()),
            Ok(Err(error)) => Err(anyhow!(error)),
            Err(_) => Err(anyhow!("terminal browser input worker stopped")),
        }
    }

    fn queue_human_input(&self, input: HumanInput) -> Result<()> {
        anyhow::ensure!(
            self.is_human_control_active(),
            "human control is not active"
        );
        let is_motion = matches!(
            &input,
            HumanInput::Mouse(BrowserMouseInput {
                kind: BrowserMouseKind::Move,
                ..
            })
        );
        let queued = QueuedHumanInput {
            generation: self.inner.human_control_generation.load(Ordering::SeqCst),
            input,
        };
        match self.inner.human_input_tx.input.try_send(queued) {
            Ok(()) => Ok(()),
            Err(mpsc::error::TrySendError::Full(_)) if is_motion => Ok(()),
            Err(mpsc::error::TrySendError::Full(_)) => {
                anyhow::bail!("browser_busy: human input queue is full")
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {
                anyhow::bail!("terminal browser input worker stopped")
            }
        }
    }

    fn human_control_is_active_for(&self, generation: u64) -> bool {
        self.is_human_control_active()
            && self.inner.human_control_generation.load(Ordering::SeqCst) == generation
    }
}

struct HumanControlTransition<'a> {
    active: &'a std::sync::atomic::AtomicBool,
}

impl Drop for HumanControlTransition<'_> {
    fn drop(&mut self) {
        self.active.store(/*val*/ false, Ordering::SeqCst);
    }
}

#[expect(
    clippy::await_holding_invalid_type,
    reason = "human input must remain serialized with model actions until its CDP call completes"
)]
async fn run_human_input_worker(inner: Weak<Inner>, mut receivers: HumanInputReceivers) {
    let mut mouse_state = HumanMouseDispatchState::default();
    let mut mouse_generation = None;
    let mut deferred = VecDeque::new();
    while let Some(queued) = next_human_input(&mut receivers, &mut deferred).await {
        let Some(inner) = inner.upgrade() else {
            return;
        };
        let QueuedHumanInput { generation, input } = queued;
        if !inner.human_control.load(Ordering::SeqCst)
            || inner.human_control_generation.load(Ordering::SeqCst) != generation
        {
            input.complete_release(Err("browser control transition was canceled".to_string()));
            continue;
        }
        if matches!(&input, HumanInput::ReleaseMouseButtons { .. }) {
            drain_human_inputs_for_generation(&mut receivers, generation, &mut deferred);
        }
        if mouse_generation != Some(generation) {
            mouse_state = HumanMouseDispatchState::default();
            mouse_generation = Some(generation);
        }
        let (result, completion_tx, crash_on_error) = {
            let _operation = inner.operation.lock().await;
            if !inner.human_control.load(Ordering::SeqCst)
                || inner.human_control_generation.load(Ordering::SeqCst) != generation
            {
                input.complete_release(Err("browser control transition was canceled".to_string()));
                continue;
            }
            let cdp = {
                let session = inner.session.lock().await;
                let Some(session) = session.as_ref() else {
                    input.complete_release(Err("terminal browser is not open".to_string()));
                    continue;
                };
                session.cdp.clone()
            };
            match input {
                HumanInput::Key(input) => {
                    (actions::dispatch_human_key(&cdp, &input).await, None, true)
                }
                HumanInput::Text(text) => {
                    (actions::insert_human_text(&cdp, &text).await, None, true)
                }
                HumanInput::Mouse(input) => (
                    actions::dispatch_human_mouse(&cdp, input, &mut mouse_state).await,
                    None,
                    true,
                ),
                HumanInput::ReleaseMouseButtons {
                    completion_tx,
                    after_release,
                } => {
                    let result = actions::release_human_mouse_buttons(&cdp, &mut mouse_state).await;
                    let crash_on_error = result.is_err();
                    let result = result.and_then(|()| {
                        finish_human_control_after_release(&inner, generation, after_release)
                    });
                    (result, Some(completion_tx), crash_on_error)
                }
            }
        };
        let completion_result = match result {
            Ok(()) => Ok(()),
            Err(error) => {
                let error = error.to_string();
                tracing::warn!(%error, "terminal-browser human input failed");
                if crash_on_error {
                    inner.set_crashed("Terminal browser input connection failed".to_string());
                }
                Err(error)
            }
        };
        if let Some(completion_tx) = completion_tx {
            let _ = completion_tx.send(completion_result);
        }
    }
}

async fn next_human_input(
    receivers: &mut HumanInputReceivers,
    deferred: &mut VecDeque<QueuedHumanInput>,
) -> Option<QueuedHumanInput> {
    if let Ok(queued) = receivers.control.try_recv() {
        return Some(queued);
    }
    if let Some(queued) = deferred.pop_front() {
        return Some(queued);
    }
    tokio::select! {
        biased;
        queued = receivers.control.recv() => queued,
        queued = receivers.input.recv() => queued,
    }
}

fn drain_human_inputs_for_generation(
    receivers: &mut HumanInputReceivers,
    generation: u64,
    deferred: &mut VecDeque<QueuedHumanInput>,
) {
    deferred.retain(|queued| queued.generation != generation);
    while let Ok(queued) = receivers.input.try_recv() {
        if queued.generation != generation {
            deferred.push_back(queued);
        }
    }
}

fn finish_human_control_after_release(
    inner: &Inner,
    generation: u64,
    after_release: HumanControlAfterRelease,
) -> Result<()> {
    match after_release {
        HumanControlAfterRelease::Continue => Ok(()),
        HumanControlAfterRelease::End => {
            anyhow::ensure!(
                inner.transition_human_control(HumanControlStateTransition::Deactivate {
                    generation,
                }),
                "browser control transition was canceled"
            );
            Ok(())
        }
    }
}

#[cfg(test)]
#[path = "human_control_tests.rs"]
mod tests;
