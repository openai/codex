use std::sync::Arc;
use std::sync::Weak;
use std::sync::atomic::Ordering;

use anyhow::Result;
use anyhow::anyhow;
use tokio::sync::mpsc;

use crate::actions;
use crate::actions::HumanMouseDispatchState;
use crate::input::BrowserKeyInput;
use crate::input::BrowserMouseInput;
use crate::input::BrowserMouseKind;
use crate::session::Inner;
use crate::session::TerminalBrowser;

const HUMAN_INPUT_CAPACITY: usize = 128;
const MAX_HUMAN_TEXT_BYTES: usize = 64 * 1024;

pub(crate) struct HumanInputSender(mpsc::Sender<QueuedHumanInput>);

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
}

pub(crate) struct QueuedHumanInput {
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
    pub(crate) fn human_input_channel() -> (HumanInputSender, mpsc::Receiver<QueuedHumanInput>) {
        let (sender, receiver) = mpsc::channel(HUMAN_INPUT_CAPACITY);
        (HumanInputSender(sender), receiver)
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
            self.inner
                .transition_human_control(HumanControlStateTransition::Deactivate {
                    generation: token.generation,
                }),
            "browser control transition was canceled"
        );
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

    fn queue_human_input(&self, input: HumanInput) -> Result<()> {
        anyhow::ensure!(
            self.is_human_control_active(),
            "human control is not active"
        );
        let is_motion = matches!(
            input,
            HumanInput::Mouse(BrowserMouseInput {
                kind: BrowserMouseKind::Move,
                ..
            })
        );
        let queued = QueuedHumanInput {
            generation: self.inner.human_control_generation.load(Ordering::SeqCst),
            input,
        };
        match self.inner.human_input_tx.0.try_send(queued) {
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
async fn run_human_input_worker(
    inner: Weak<Inner>,
    mut receiver: mpsc::Receiver<QueuedHumanInput>,
) {
    let mut mouse_state = HumanMouseDispatchState::default();
    let mut mouse_generation = None;
    while let Some(queued) = receiver.recv().await {
        let Some(inner) = inner.upgrade() else {
            return;
        };
        if !inner.human_control.load(Ordering::SeqCst)
            || inner.human_control_generation.load(Ordering::SeqCst) != queued.generation
        {
            continue;
        }
        if mouse_generation != Some(queued.generation) {
            mouse_state = HumanMouseDispatchState::default();
            mouse_generation = Some(queued.generation);
        }
        let result = {
            let _operation = inner.operation.lock().await;
            if !inner.human_control.load(Ordering::SeqCst)
                || inner.human_control_generation.load(Ordering::SeqCst) != queued.generation
            {
                continue;
            }
            let cdp = {
                let session = inner.session.lock().await;
                let Some(session) = session.as_ref() else {
                    continue;
                };
                session.cdp.clone()
            };
            match queued.input {
                HumanInput::Key(input) => actions::dispatch_human_key(&cdp, &input).await,
                HumanInput::Text(text) => actions::insert_human_text(&cdp, &text).await,
                HumanInput::Mouse(input) => {
                    actions::dispatch_human_mouse(&cdp, input, &mut mouse_state).await
                }
            }
        };
        if let Err(error) = result {
            tracing::warn!(%error, "terminal-browser human input failed");
            inner.set_crashed("Terminal browser input connection failed".to_string());
        }
    }
}
