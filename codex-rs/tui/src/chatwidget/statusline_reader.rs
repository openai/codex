//! Poll the displayed thread’s statusline resource until a read fails or the reader is retired.

use super::ChatWidget;
use super::statusline_state::StatuslineState;
use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::app_server_session::AppServerSession;
use crate::backend_id::BackendId;
use crate::statusline_provider::ProviderId;
use crate::statusline_provider::ProviderSelection;
use crate::statusline_provider::ResolvedStatuslineProvider;
use crate::statusline_provider::StatuslineProvider;
use crate::ui_tree::UiNode;
use crate::ui_tree::UiTree;
use codex_app_server_protocol::McpResourceContent;
use codex_config::types::StatusLineProviderId;
use codex_config::types::StatusLineSelection;
use codex_features::Feature;
use codex_protocol::ThreadId;
use codex_utils_path_uri::LegacyAppPathString;
use color_eyre::eyre::Result;
use color_eyre::eyre::WrapErr;
use color_eyre::eyre::bail;
use color_eyre::eyre::eyre;
use std::path::PathBuf;
use std::time::Duration;
use tokio::sync::oneshot;
use tokio::sync::watch;
use tokio::time::Instant;
use tokio::time::sleep;
use tokio::time::timeout;
use tokio::time::timeout_at;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

pub(super) struct ActiveStatuslineRead {
    token: Uuid,
    thread: Option<ThreadId>,
    selection: ProviderId,
    provider_from_picker: Option<StatuslineProvider>,
    host_cwd: PathBuf,
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
    Finished(BackendId, Result<UiTree, String>),
}

impl StatuslineReadEvent {
    fn send(sender: &AppEventSender, token: Uuid, step: ReadStep) {
        sender.send(AppEvent::StatuslineRead(Self { token, step }));
    }
}

impl ChatWidget {
    pub(super) fn selected_statusline_provider(&self) -> Option<&StatusLineProviderId> {
        if self.statusline_disconnected
            || self.read_only_history
            || self
                .app_event_tx
                .voice_only
                .load(std::sync::atomic::Ordering::Relaxed)
            || !self
                .config
                .features
                .enabled(Feature::TuiExtensionsStatusLine)
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
            host_cwd: self.status_line_cwd().to_owned(),
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

    pub(crate) fn handle_statusline_read(
        &mut self,
        event: StatuslineReadEvent,
        owner: &AppServerSession,
    ) {
        let Some(footer) = self.statusline_read.as_ref() else {
            return;
        };
        if event.token != footer.token {
            return;
        }
        if !footer.is_current(self) {
            self.refresh_status_line();
            return;
        }
        match event.step {
            ReadStep::Start(ready) => footer.start(owner, &self.app_event_tx, ready),
            ReadStep::Snapshot(backend, reply) => {
                if backend == owner.backend_id()
                    && let Some(state) = self.statusline_state()
                {
                    let _ = reply.send(state);
                }
            }
            ReadStep::Finished(backend, result) => {
                if backend != owner.backend_id() {
                    return;
                }
                let tree = result.unwrap_or_else(|name| {
                    footer.cancel.cancel();
                    UiTree(Some(UiNode::String(format!(
                        "Error loading statusline contents from {name}. Use /statusline to (re)select your provider and try again."
                    ))))
                });
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
            && self.host_cwd == chat.status_line_cwd()
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
        let mut name = provider_from_picker.as_ref().map_or_else(
            || id.tool.as_str().to_owned(),
            |provider| provider.title.clone(),
        );
        let token = self.token;
        let events = events.clone();
        let client = owner.request_handle();
        let backend = owner.backend_id();
        let cwd = LegacyAppPathString::from_path(&self.host_cwd).to_inferred_path_uri();
        let cancel = self.cancel.clone();
        tokio::spawn(async move {
            let result: Option<Result<()>> = cancel
                .run_until_cancelled(async {
                    let cwd = cwd.ok_or_else(|| eyre!("invalid app-server working directory"))?;
                    let provider = timeout(Duration::from_secs(/*secs*/ 5), async {
                        loop {
                            if let Ok(Some(provider)) = ResolvedStatuslineProvider::lookup(
                                &client,
                                thread,
                                &cwd,
                                provider_from_picker.as_ref().map_or(
                                    ProviderSelection::Saved(&id),
                                    ProviderSelection::Discovered,
                                ),
                            )
                            .await
                            {
                                return Ok::<_, watch::error::RecvError>(provider);
                            }
                            // A cold catalog may arrive before the selected MCP server finishes starting.
                            ready.changed().await?;
                        }
                    })
                    .await??;
                    name.clone_from(&provider.provider().title);
                    loop {
                        let (reply, snapshot) = oneshot::channel();
                        StatuslineReadEvent::send(
                            &events,
                            token,
                            ReadStep::Snapshot(backend, reply),
                        );
                        let snapshot = serde_json::to_value(snapshot.await?)?;
                        let deadline = Instant::now() + Duration::from_secs(1);
                        // A ready reply can win the final poll of timeout_at after the deadline.
                        let response = match timeout_at(deadline, provider.read(snapshot)).await {
                            Ok(response) if Instant::now() < deadline => response?,
                            Ok(_) | Err(_) => {
                                bail!("resource read timed out");
                            }
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
            if let Some(Err(_)) = result {
                StatuslineReadEvent::send(&events, token, ReadStep::Finished(backend, Err(name)));
            }
        });
    }
}

fn decode_tree(uri: &str, contents: &[McpResourceContent]) -> Result<UiTree> {
    let [
        McpResourceContent::Text {
            uri: returned_uri,
            mime_type: Some(mime),
            text,
            ..
        },
    ] = contents
    else {
        bail!("expected one text resource");
    };
    let media_type = mime.split(';').next().unwrap_or_default().trim();
    if uri != returned_uri || !media_type.eq_ignore_ascii_case("application/json") {
        bail!("expected application/json at the advertised resource URI");
    }
    text.parse().wrap_err("invalid UI tree")
}

#[cfg(test)]
#[path = "statusline_reader_tests.rs"]
mod tests;
