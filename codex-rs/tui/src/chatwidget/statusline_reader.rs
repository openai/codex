//! Poll the displayed thread’s statusline resource until a read fails or the reader is retired.

mod resource;

use self::resource::Failure;
use self::resource::ReadIssue;
use self::resource::decode_tree;
use super::ChatWidget;
use super::statusline_state::StatuslineState;
use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::app_server_session::AppServerSession;
use crate::backend_id::BackendId;
use crate::legacy_core::config::Config;
use crate::statusline_metrics::StatuslineMetrics;
use crate::statusline_provider::LOOKUP_TIMEOUT;
use crate::statusline_provider::ProviderId;
use crate::statusline_provider::ProviderLookupError;
use crate::statusline_provider::ProviderSelection;
use crate::statusline_provider::ResolvedStatuslineProvider;
use crate::statusline_provider::StatuslineProvider;
use crate::ui_tree::UiNode;
use crate::ui_tree::UiTree;
use codex_config::types::StatusLineProviderId;
use codex_config::types::StatusLineSelection;
use codex_protocol::ThreadId;
use codex_utils_path_uri::PathUri;
use std::time::Duration;
use tokio::sync::oneshot;
use tokio::sync::watch;
use tokio::time::Instant;
use tokio::time::sleep;
use tokio::time::timeout_at;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

pub(super) struct ActiveStatuslineRead {
    token: Uuid,
    thread: Option<ThreadId>,
    selection: ProviderId,
    provider_from_picker: Option<StatuslineProvider>,
    host_cwd: PathUri,
    startup: watch::Sender<()>,
    cancel: CancellationToken,
}

impl Drop for ActiveStatuslineRead {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

#[derive(Debug)]
pub(crate) struct StatuslineReadEvent {
    token: Uuid,
    step: ReadStep,
}

#[derive(derive_more::Debug)]
enum ReadStep {
    Start(watch::Receiver<()>),
    #[debug("Snapshot(..)")]
    Snapshot(BackendId, oneshot::Sender<StatuslineState>),
    #[debug("Finished(..)")]
    Finished(BackendId, Result<UiTree, Failure>),
}

impl StatuslineReadEvent {
    fn send(sender: &AppEventSender, token: Uuid, step: ReadStep) {
        sender.send(AppEvent::StatuslineRead(Self { token, step }));
    }
}

impl ChatWidget {
    pub(crate) fn selected_statusline_provider(&self) -> Option<&StatusLineProviderId> {
        if self.statusline_disconnected
            || self.read_only_history
            || self
                .app_event_tx
                .voice_only
                .load(std::sync::atomic::Ordering::Relaxed)
            || !self.custom_statusline_enabled()
        {
            return None;
        }
        self.configured_statusline_provider()
    }

    pub(crate) fn configured_statusline_provider(&self) -> Option<&StatusLineProviderId> {
        match self.local_settings.tui.status_line_provider.as_deref()? {
            StatusLineSelection::Builtin(_) => None,
            StatusLineSelection::Mcp(id) => Some(id),
        }
    }

    pub(crate) fn reselect_statusline_provider(
        &mut self,
        provider_from_picker: Option<StatuslineProvider>,
    ) {
        self.statusline_read = None;
        if let Some(selection) = self.selected_statusline_provider() {
            self.start_statusline_read(selection.into(), provider_from_picker);
        }
        self.refresh_status_line();
    }

    pub(super) fn refresh_provider_footer(&mut self) -> bool {
        let Some(selection) = self.selected_statusline_provider() else {
            self.statusline_read = None;
            return false;
        };
        if self
            .statusline_read
            .as_ref()
            .is_some_and(|read| read.is_current(self))
        {
            return true;
        }
        let selection = ProviderId::from(selection);
        // An explicit choice may precede the first thread ID for this widget.
        let provider_from_picker = self
            .statusline_read
            .as_mut()
            .filter(|read| read.thread.is_none() && read.selection == selection)
            .and_then(|read| read.provider_from_picker.take());
        self.start_statusline_read(selection, provider_from_picker);
        true
    }

    fn start_statusline_read(
        &mut self,
        selection: ProviderId,
        provider_from_picker: Option<StatuslineProvider>,
    ) {
        let token = Uuid::new_v4();
        let (startup, ready) = watch::channel(());
        let provider_from_picker = provider_from_picker.filter(|provider| provider.id == selection);
        self.statusline_read = Some(ActiveStatuslineRead {
            token,
            thread: self.thread_id,
            selection,
            provider_from_picker,
            host_cwd: self.session_cwd().clone(),
            startup,
            cancel: CancellationToken::new(),
        });
        self.bottom_pane.set_status_line_enabled(/*enabled*/ true);
        self.set_status_line(Some(
            UiTree(Some(UiNode::String("Initializing…".into()))).into(),
        ));
        if self.thread_id.is_some() {
            StatuslineReadEvent::send(&self.app_event_tx, token, ReadStep::Start(ready));
        }
    }

    pub(super) fn provider_needs_snapshot(&self) -> bool {
        if self
            .statusline_read
            .as_ref()
            .is_some_and(|read| read.cancel.is_cancelled() && read.is_current(self))
        {
            return false;
        }
        self.selected_statusline_provider().is_some()
    }

    pub(crate) async fn handle_statusline_read(
        &mut self,
        event: StatuslineReadEvent,
        owner: &AppServerSession,
        metrics: &StatuslineMetrics,
        config: &Config,
    ) {
        let Some(footer) = self
            .statusline_read
            .as_ref()
            .filter(|read| read.token == event.token)
        else {
            return;
        };
        if !footer.is_current(self) {
            self.refresh_status_line();
            return;
        }
        match event.step {
            ReadStep::Start(ready) => {
                metrics.record(config, "codex.statusline.provider_started", &[]);
                footer.start(owner, &self.app_event_tx, ready);
            }
            ReadStep::Snapshot(backend, reply) => {
                if backend == owner.backend_id()
                    && let Some(state) = self.provider_statusline_state(owner).await
                {
                    let _ = reply.send(state);
                }
            }
            ReadStep::Finished(backend, result) => {
                if backend != owner.backend_id() {
                    return;
                }
                let tree = match result {
                    Ok(tree) => tree,
                    Err(failure) => {
                        metrics.record(
                            config,
                            "codex.statusline.provider_failed",
                            &[("kind", failure.metric_kind())],
                        );
                        footer.cancel.cancel();
                        tracing::warn!(?failure, "Status line provider stopped");
                        let message = match failure {
                            Failure::LookupTimedOut | Failure::Read(ReadIssue::TimedOut) => {
                                "Status line timed out"
                            }
                            Failure::LookupUnavailable => "Status line unavailable",
                            Failure::Snapshot | Failure::Read(_) => "Status line failed",
                        };
                        UiTree(Some(UiNode::String(format!("{message} · /statusline"))))
                    }
                };
                self.set_status_line(Some(tree.into()));
            }
        }
    }
}

impl ActiveStatuslineRead {
    fn is_current(&self, chat: &ChatWidget) -> bool {
        self.thread == chat.thread_id
            && chat.selected_statusline_provider().is_some_and(|id| {
                self.selection.server.as_str() == id.server
                    && self.selection.tool.as_str() == id.tool
            })
            && &self.host_cwd == chat.session_cwd()
    }

    pub(super) fn server_ready(&self, server: &str) {
        if self.selection.server.as_str() == server {
            self.startup.send_replace(());
        }
    }

    fn start(
        &self,
        owner: &AppServerSession,
        events: &AppEventSender,
        mut ready: watch::Receiver<()>,
    ) {
        let Some(thread) = self.thread else { return };
        let id = self.selection.clone();
        let provider_from_picker = self.provider_from_picker.clone();
        let token = self.token;
        let events = events.clone();
        let client = owner.request_handle();
        let backend = owner.backend_id();
        let cwd = self.host_cwd.clone();
        let cancel = self.cancel.clone();
        tokio::spawn(async move {
            let result: Option<Result<(), Failure>> = cancel
                .run_until_cancelled(async {
                    let deadline = Instant::now() + LOOKUP_TIMEOUT;
                    let provider = loop {
                        let choice = provider_from_picker
                            .as_ref()
                            .map_or(ProviderSelection::Saved(&id), ProviderSelection::Discovered);
                        match ResolvedStatuslineProvider::lookup(
                            &client, thread, &cwd, choice, deadline,
                        )
                        .await
                        {
                            Ok(Some(provider)) => break provider,
                            Err(ProviderLookupError::TimedOut) => {
                                return Err(Failure::LookupTimedOut);
                            }
                            Ok(None) | Err(ProviderLookupError::Request(_)) => {}
                        }
                        // A cold catalog can arrive before this server finishes starting.
                        if !matches!(timeout_at(deadline, ready.changed()).await, Ok(Ok(())))
                            || Instant::now() >= deadline
                        {
                            return Err(Failure::LookupUnavailable);
                        }
                    };
                    loop {
                        let (reply, snapshot) = oneshot::channel();
                        StatuslineReadEvent::send(
                            &events,
                            token,
                            ReadStep::Snapshot(backend, reply),
                        );
                        let state = snapshot.await.map_err(|_| Failure::Snapshot)?;
                        let snapshot =
                            serde_json::to_value(state).map_err(|_| Failure::Snapshot)?;
                        let deadline = Instant::now() + Duration::from_secs(/*secs*/ 1);
                        // A ready reply can win the final poll of timeout_at after the deadline.
                        let response = match timeout_at(deadline, provider.read(snapshot)).await {
                            Ok(response) if Instant::now() < deadline => {
                                response.map_err(ReadIssue::from)?
                            }
                            Ok(_) | Err(_) => return Err(ReadIssue::TimedOut.into()),
                        };
                        let tree = decode_tree(&provider.provider().uri, &response.contents)?;
                        StatuslineReadEvent::send(
                            &events,
                            token,
                            ReadStep::Finished(backend, Ok(tree)),
                        );
                        sleep(Duration::from_secs(/*secs*/ 1)).await;
                    }
                })
                .await;
            if let Some(Err(failure)) = result {
                StatuslineReadEvent::send(
                    &events,
                    token,
                    ReadStep::Finished(backend, Err(failure)),
                );
            }
        });
    }
}

#[cfg(test)]
#[path = "statusline_reader_tests.rs"]
mod tests;
