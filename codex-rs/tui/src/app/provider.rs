//! Browse the connected server's provider catalog without changing the active chat.

use super::App;
use crate::app_event::AppEvent;
use crate::app_server_session::AppServerSession;
use crate::backend_id::BackendId;
use crate::bottom_pane::SelectionItem;
use crate::bottom_pane::SelectionViewParams;
use crate::pager_overlay::Overlay;
use crate::tui::Tui;
use codex_app_server_client::TypedRequestError;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::ModelProviderListParams;
use codex_app_server_protocol::ModelProviderListResponse;
use codex_app_server_protocol::ModelProviderSummary;
use codex_app_server_protocol::ProviderSettings;
use codex_app_server_protocol::RequestId;
use ratatui::style::Stylize;
use ratatui::text::Line;
use std::sync::Arc;
use tokio::sync::watch;
use uuid::Uuid;

mod fields;
use fields::Field;
use fields::provider_fields;

const LOADING_VIEW: &str = "model_provider_loading";
pub(super) const LIST_VIEW: &str = "model_provider_list";

#[derive(Debug)]
pub(crate) enum Event {
    Load(watch::Receiver<()>),
    Loaded {
        backend: BackendId,
        dismissed: watch::Receiver<()>,
        result: Result<ModelProviderListResponse, String>,
    },
    Details {
        backend: BackendId,
        provider: Arc<ModelProviderSummary>,
    },
}

fn back() -> SelectionItem {
    SelectionItem {
        name: "Back".into(),
        dismiss_on_select: true,
        ..Default::default()
    }
}

pub(crate) fn loading() -> (SelectionViewParams, Event) {
    // Each popup owns its sender. Dismissal, widget replacement, and reopening
    // invalidate only that popup's request, independent of initial thread attachment.
    let (dismiss, dismissed) = watch::channel(());
    let view = SelectionViewParams {
        view_id: Some(LOADING_VIEW),
        title: Some("Loading model providers…".into()),
        items: vec![back()],
        allow_input_when_disconnected: true,
        on_cancel: Some(Box::new(move |_| {
            dismiss.send_replace(());
        })),
        ..Default::default()
    };
    (view, Event::Load(dismissed))
}

pub(super) fn handle(app: &mut App, tui: &mut Tui, server: &AppServerSession, event: Event) {
    let backend = server.backend_id();
    match event {
        Event::Load(mut dismissed) => {
            let tx = app.app_event_tx.clone();
            let handle = server.request_handle();
            let offline = app.reconnect.offline;
            tokio::spawn(async move {
                let request = ClientRequest::ModelProviderList {
                    request_id: RequestId::String(Uuid::new_v4().to_string()),
                    params: ModelProviderListParams {},
                };
                let result = if offline {
                    Err("Reconnect to the app server to browse model providers.".into())
                } else {
                    tokio::select! {
                        _ = dismissed.changed() => return,
                        result = handle.request_typed(request) => result.map_err(request_error),
                    }
                };
                tx.send(AppEvent::Provider(Event::Loaded {
                    backend,
                    dismissed,
                    result,
                }));
            });
        }
        Event::Loaded {
            backend: origin,
            dismissed,
            result,
        } => {
            if origin != backend || !matches!(dismissed.has_changed(), Ok(false)) {
                return;
            }
            let chat = &mut app.chat_widget;
            let view = match result {
                Ok(page) => list(
                    chat.thread_id()
                        .map(|_| chat.config_ref().model_provider_id.as_str()),
                    backend,
                    page,
                ),
                Err(error) => SelectionViewParams {
                    title: Some("Model providers".into()),
                    subtitle: Some(error),
                    items: vec![back()],
                    allow_input_when_disconnected: true,
                    ..Default::default()
                },
            };
            chat.replace_selection_view_if_present(LOADING_VIEW, view);
        }
        Event::Details {
            backend: origin,
            provider,
        } => {
            if origin == backend
                && app.overlay.is_none()
                && app.chat_widget.active_view_is(LIST_VIEW)
            {
                let _ = tui.enter_alt_screen();
                app.overlay = Some(Overlay::new_static_with_lines(
                    details(&provider),
                    "Model provider details".into(),
                    app.keymap.pager.clone(),
                ));
                tui.frame_requester().schedule_frame();
            }
        }
    }
}

/// Refresh a list opened before the first chat acquired a server-side provider.
pub(crate) fn refresh_list_if_present(chat: &mut crate::chatwidget::ChatWidget) -> Option<Event> {
    if chat.no_modal_or_popup_active() {
        return None;
    }
    let (view, event) = loading();
    chat.replace_selection_view_if_present(LIST_VIEW, view)
        .then_some(event)
}

fn request_error(err: TypedRequestError) -> String {
    if let TypedRequestError::Server { method, source } = &err
        && (source.code == -32601
            || (source.code == -32600
                && source
                    .message
                    .starts_with(&format!("Invalid request: unknown variant `{method}`"))))
    {
        return "Update the connected app server to browse model providers.".into();
    }
    format!("Could not load model providers: {err}")
}

fn list(
    current: Option<&str>,
    backend: BackendId,
    page: ModelProviderListResponse,
) -> SelectionViewParams {
    let current_label = current.unwrap_or("No active chat");
    let items = page
        .data
        .into_iter()
        .map(|provider| {
            let provider = Arc::new(provider);
            SelectionItem {
                name: provider.name.clone(),
                is_current: current == Some(provider.id.as_str()),
                description: Some(provider.id.clone()),
                actions: vec![Box::new(move |tx| {
                    tx.send(AppEvent::Provider(Event::Details {
                        backend,
                        provider: Arc::clone(&provider),
                    }));
                })],
                ..Default::default()
            }
        })
        .collect();
    SelectionViewParams {
        view_id: Some(LIST_VIEW),
        title: Some("Model providers".into()),
        subtitle: Some(format!(
            "Current chat: {current_label} · Default for new chats: {}",
            page.default_provider_id
        )),
        items,
        allow_input_when_disconnected: true,
        ..Default::default()
    }
}

fn details(provider: &ModelProviderSummary) -> Vec<Line<'static>> {
    let mut lines = vec![Line::from(provider.name.clone().bold()), Line::default()];
    let mut field = |label: &str, value: &str| {
        lines.push(Line::from(vec![
            format!("{label}  ").bold(),
            value.to_owned().into(),
        ]));
    };
    field("Provider ID", &provider.id);
    if matches!(provider.settings, ProviderSettings::OpenAi) {
        field("Type", "Built-in OpenAI provider");
    }
    for (kind, value) in provider_fields(provider) {
        let absent = match kind {
            Field::Url | Field::ManagedUrl => "Not reported",
            Field::ExternalAuth => "Managed externally",
            Field::Variable => "None",
            _ => "Not set",
        };
        field(kind.label(), value.unwrap_or(absent));
    }
    lines
}

#[cfg(test)]
#[path = "provider_tests.rs"]
mod tests;
