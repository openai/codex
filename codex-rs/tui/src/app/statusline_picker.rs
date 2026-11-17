//! Discover providers for the open statusline picker and apply explicit selections.

use super::*;
use crate::backend_id::BackendId;
use crate::bottom_pane::ColumnWidthMode;
use crate::bottom_pane::SelectionKeyAction;
use crate::config_update::format_config_error;
use crate::statusline_provider::McpServerName;
use crate::statusline_provider::ProviderId;
use crate::statusline_provider::ProviderLookupError;
use crate::statusline_provider::StatuslineProvider;
use crate::statusline_provider::discovery::DiscoveryUpdate;
use crate::statusline_provider::discovery::refresh_statusline_providers;
use codex_config::types::BuiltinStatusLine;
use codex_config::types::StatusLineProviderId;
use codex_config::types::StatusLineSelection;
use futures::StreamExt;
use tokio_util::task::AbortOnDropHandle;

pub(super) const PICKER: &str = "statusline-providers";

pub(super) struct StatuslinePicker {
    pub(super) id: Uuid,
    backend: BackendId,
    thread: Option<ThreadId>,
    selected: Option<ProviderId>,
    providers: Vec<StatuslineProvider>,
    offline: bool,
    worker: Option<AbortOnDropHandle<()>>,
}

#[derive(Default)]
struct DiscoveryFailures {
    servers: Vec<String>,
    omitted: usize,
    inventory: Option<&'static str>,
}

impl DiscoveryFailures {
    fn record_server(&mut self, server: &McpServerName, error: &ProviderLookupError) {
        if self.servers.len() < 4 {
            let name: String = label(server.as_str().to_owned()).chars().take(80).collect();
            let reason = discovery_failure_reason(error);
            self.servers.push(format!("{name}: {reason}."));
        } else {
            self.omitted += 1;
        }
    }

    fn into_details(mut self) -> Option<String> {
        if self.servers.is_empty() && self.inventory.is_none() {
            return None;
        }
        tracing::warn!(
            affected_servers = self.servers.len() + self.omitted,
            inventory = self.inventory,
            "Status line provider discovery was incomplete"
        );
        if let Some(reason) = self.inventory {
            self.servers
                .insert(0, format!("Global provider inventory: {reason}."));
        }
        if self.omitted > 0 {
            self.servers
                .push(format!("Additional affected servers: {}.", self.omitted));
        }
        Some(self.servers.join("\n"))
    }
}

fn discovery_failure_reason(error: &ProviderLookupError) -> &'static str {
    match error {
        ProviderLookupError::TimedOut => "no result by the 5-second discovery deadline",
        ProviderLookupError::Request(_) => "request failed",
    }
}

fn label(text: String) -> String {
    crate::ui_tree::sanitize_leaf(text).replace(['\n', '\t'], " ")
}

impl StatuslinePicker {
    fn provider(&self, id: &ProviderId) -> Option<&StatuslineProvider> {
        self.providers.iter().find(|provider| provider.id == *id)
    }

    fn choose(&self, id: Option<ProviderId>) -> SelectionKeyAction {
        let scan = self.id;
        Box::new(move |tx, key| {
            tx.send(AppEvent::ChooseStatuslineProvider {
                scan,
                id: id.clone(),
                key,
            })
        })
    }

    fn provider_row(
        &self,
        id: &ProviderId,
        provider: Option<&StatuslineProvider>,
    ) -> SelectionItem {
        let saved = StatusLineProviderId::from(id);
        let current = self.selected.as_ref() == Some(id);
        let mut name = match provider {
            Some(provider) => provider.title.clone(),
            None => label(saved.tool.clone()),
        };
        if let Some(plugin) = provider.and_then(|p| p.plugin_label.as_ref()) {
            name = format!("{name} · {plugin}");
        }
        if self.offline || (self.worker.is_none() && provider.is_none()) {
            name.push_str(" (Unavailable)");
        }
        SelectionItem {
            name,
            description: Some(label(format!("{} / {}", saved.server, saved.tool))),
            is_current: current,
            is_disabled: self.offline,
            on_accept_key: Some(self.choose(Some(id.clone()))),
            ..Default::default()
        }
    }

    fn view(&self, initial_selected_idx: Option<usize>) -> SelectionViewParams {
        let mut items = Vec::new();
        if let Some(id) = &self.selected {
            items.push(self.provider_row(id, self.provider(id)));
        }
        items.push(SelectionItem {
            name: "Default status line".into(),
            is_current: self.selected.is_none(),
            on_accept_key: Some(self.choose(/*id*/ None)),
            ..Default::default()
        });
        for provider in &self.providers {
            if Some(&provider.id) != self.selected.as_ref() {
                items.push(self.provider_row(&provider.id, Some(provider)));
            }
        }
        let subtitle = if self.offline {
            Some("Plugin providers are unavailable while disconnected.")
        } else {
            self.worker
                .as_ref()
                .map(|_| "Discovering MCP status line providers…")
        };
        let scan = self.id;
        SelectionViewParams {
            view_id: Some(PICKER),
            title: Some("Select a status line provider".into()),
            subtitle: subtitle.map(str::to_owned),
            items,
            initial_selected_idx,
            col_width_mode: ColumnWidthMode::AutoVisible,
            allow_input_when_disconnected: self.offline,
            on_cancel: Some(Box::new(move |tx| {
                tx.send(AppEvent::CloseStatuslinePicker(scan))
            })),
            ..SelectionViewParams::picker()
        }
    }

    fn applies_to(
        &mut self,
        thread: Option<ThreadId>,
        backend: BackendId,
        config: &Config,
    ) -> bool {
        if self.backend != backend || !config.features.enabled(Feature::TuiExtensionsStatusLine) {
            return false;
        }
        // The picker may open before the displayed thread receives its ID.
        self.thread = self.thread.or(thread);
        self.thread == thread
    }
}

impl App {
    pub(super) fn open_statusline_picker(&mut self, owner: &AppServerSession) {
        if !self
            .config
            .features
            .enabled(Feature::TuiExtensionsStatusLine)
        {
            return;
        }
        let selected = self
            .chat_widget
            .configured_statusline_provider()
            .map(ProviderId::from);
        let id = Uuid::new_v4();
        let worker = (!self.reconnect.offline).then(|| {
            let known = selected.iter().map(|id| id.server.clone()).collect();
            let stream = refresh_statusline_providers(owner.request_handle(), known);
            let events = self.app_event_tx.clone();
            AbortOnDropHandle::new(tokio::spawn(async move {
                events.send(AppEvent::StatuslinePicker {
                    scan: id,
                    updates: stream.collect().await,
                });
            }))
        });
        let picker = StatuslinePicker {
            id,
            backend: owner.backend_id(),
            thread: self.chat_widget.thread_id(),
            selected,
            providers: Vec::new(),
            offline: self.reconnect.offline,
            worker,
        };
        self.chat_widget
            .show_selection_view(picker.view(/*initial_selected_idx*/ None));
        self.statusline_picker = Some(picker);
    }

    fn statusline_picker_for(
        &mut self,
        scan: Uuid,
        owner: &AppServerSession,
    ) -> Option<&mut StatuslinePicker> {
        let picker = self
            .statusline_picker
            .as_mut()
            .filter(|picker| picker.id == scan)?;
        if self
            .chat_widget
            .selected_index_for_present_view(PICKER)
            .is_none()
            || !picker.applies_to(
                self.chat_widget.thread_id(),
                owner.backend_id(),
                &self.config,
            )
        {
            self.statusline_picker = None;
            return None;
        }
        self.statusline_picker.as_mut()
    }

    pub(super) fn finish_statusline_discovery(
        &mut self,
        scan: Uuid,
        updates: Vec<DiscoveryUpdate>,
        owner: &AppServerSession,
    ) {
        let selected_index = self.chat_widget.selected_index_for_present_view(PICKER);
        let Some(picker) = self.statusline_picker_for(scan, owner) else {
            return;
        };
        if picker.worker.take().is_none() {
            return;
        }
        let mut failures = DiscoveryFailures::default();
        for update in updates {
            let (server, result) = match update {
                DiscoveryUpdate::Server(server, result) => (server, result),
                DiscoveryUpdate::InventoryFailed(error) => {
                    failures.inventory = Some(discovery_failure_reason(&error));
                    continue;
                }
            };
            let mut providers = result.unwrap_or_else(|error| {
                failures.record_server(&server, &error);
                Vec::new()
            });
            for provider in &mut providers {
                provider.title = label(std::mem::take(&mut provider.title));
                provider.plugin_label = provider.plugin_label.take().map(label);
            }
            picker.providers.extend(providers);
        }
        picker
            .providers
            .sort_unstable_by(|a, b| (&a.title, &a.id).cmp(&(&b.title, &b.id)));
        let view = picker.view(selected_index);
        self.chat_widget
            .set_statusline_discovery_warning(scan, failures.into_details());
        self.chat_widget
            .replace_selection_view_if_present(PICKER, view);
    }

    pub(super) async fn choose_statusline_provider(
        &mut self,
        scan: Uuid,
        id: Option<ProviderId>,
        key: crate::key_hint::KeyBinding,
        owner: &AppServerSession,
    ) {
        if !self.chat_widget.active_view_is(PICKER) {
            return;
        }
        let offline = self.reconnect.offline;
        let Some(picker) = self.statusline_picker_for(scan, owner) else {
            return;
        };
        let (selection, provider_from_picker) = match id {
            None => (
                StatusLineSelection::Builtin(BuiltinStatusLine::Default),
                None,
            ),
            Some(id) => {
                let provider = picker.provider(&id);
                if offline || (provider.is_none() && Some(&id) != picker.selected.as_ref()) {
                    return;
                }
                (StatusLineSelection::Mcp((&id).into()), provider.cloned())
            }
        };
        if let Err(error) = self
            .save_status_line_provider(selection, provider_from_picker)
            .await
        {
            self.chat_widget.report_config_save_error(format!(
                "Failed to save status line selection: {}",
                format_config_error(&error)
            ));
            return;
        }
        self.statusline_picker = None;
        self.chat_widget.dismiss_provider_view(PICKER);
        self.chat_widget.suppress_current_view_key_until_release(
            key,
            Instant::now(),
            crate::activation_key_guard::ActivationKeySuppression::ViewTransition,
        );
    }
}

#[cfg(test)]
#[path = "statusline_picker_tests.rs"]
mod tests;
