//! Browse and edit the connected server's providers and choose a default for new chats.

use super::App;
use crate::app_event::AppEvent;
use crate::app_server_session::AppServerSession;
use crate::backend_id::BackendId;
use crate::bottom_pane::SelectionItem;
use crate::bottom_pane::SelectionSecondaryAction;
use crate::bottom_pane::SelectionViewParams;
use crate::key_hint;
use crate::keymap::ListAction;
use crate::keymap::ListKeymap;
use crate::pager_overlay::Overlay;
use crate::pager_overlay::PagerAction;
use crate::pager_overlay::StaticOverlay;
use crate::tui::Tui;
use codex_app_server_protocol::ModelProviderListResponse;
use codex_app_server_protocol::ModelProviderSummary;
use codex_app_server_protocol::ProviderSettings;
use crossterm::event::KeyCode;
use ratatui::style::Stylize;
use ratatui::text::Line;
use std::sync::Arc;
use tokio::sync::watch;

mod editor;
pub(crate) use editor::Draft;
pub(crate) use editor::ProviderForm;
mod fields;
pub(crate) mod rpc;
mod write;
use fields::Field;
use fields::provider_fields;

const LOADING_VIEW: &str = "model_provider_loading";
pub(super) const LIST_VIEW: &str = "model_provider_list";
pub(crate) const CREATE_CUSTOM_PROVIDER: &str = "Create a custom provider or gateway";

#[derive(Debug)]
pub(crate) enum Event {
    Load {
        dismissed: watch::Receiver<()>,
        notice: Option<String>,
    },
    Loaded {
        backend: BackendId,
        dismissed: watch::Receiver<()>,
        result: Result<ModelProviderListResponse, String>,
        notice: Option<String>,
    },
    Details {
        catalog: Arc<Catalog>,
        index: usize,
        dismissed: watch::Receiver<()>,
    },
    Edit {
        catalog: Arc<Catalog>,
        index: Option<usize>,
        dismissed: watch::Receiver<()>,
    },
    Write {
        backend: BackendId,
        dismissed: watch::Receiver<()>,
        write: write::Write,
    },
    Written(write::Completion),
}

#[derive(Debug)]
pub(crate) struct Catalog {
    backend: BackendId,
    page: ModelProviderListResponse,
}

fn back() -> SelectionItem {
    SelectionItem {
        name: "Back".into(),
        dismiss_on_select: true,
        ..Default::default()
    }
}

pub(crate) fn load_error(title: &str, error: String) -> SelectionViewParams {
    SelectionViewParams {
        title: Some(title.into()),
        subtitle: Some(error),
        items: vec![back()],
        allow_input_when_disconnected: true,
        ..Default::default()
    }
}

pub(crate) fn loading() -> (SelectionViewParams, Event) {
    loading_with_notice(/*notice*/ None)
}

fn loading_with_notice(notice: Option<String>) -> (SelectionViewParams, Event) {
    let (view, dismissed) = progress(LOADING_VIEW, "Loading model providers…");
    (view, Event::Load { dismissed, notice })
}

fn progress(id: &'static str, title: &str) -> (SelectionViewParams, watch::Receiver<()>) {
    // Destroying a popup invalidates its own request, even if another popup has the same ID.
    let (dismiss, dismissed) = watch::channel(());
    let view = SelectionViewParams {
        view_id: Some(id),
        title: Some(title.into()),
        items: vec![back()],
        allow_input_when_disconnected: true,
        on_cancel: Some(Box::new(move |_| {
            dismiss.send_replace(());
        })),
        ..Default::default()
    };
    (view, dismissed)
}

pub(super) fn handle(app: &mut App, tui: &mut Tui, server: &AppServerSession, event: Event) {
    let backend = server.backend_id();
    match event {
        Event::Load {
            mut dismissed,
            notice,
        } => {
            let tx = app.app_event_tx.clone();
            let handle = server.request_handle();
            let offline = app.reconnect.offline;
            tokio::spawn(async move {
                let result = if offline {
                    Err("Reconnect to the app server to browse model providers.".into())
                } else {
                    tokio::select! {
                        _ = dismissed.changed() => return,
                        result = rpc::list_providers(&handle) => result,
                    }
                };
                tx.send(AppEvent::Provider(Event::Loaded {
                    backend,
                    dismissed,
                    result,
                    notice,
                }));
            });
        }
        Event::Loaded {
            backend: origin,
            dismissed,
            result,
            notice,
        } => {
            if origin != backend || !matches!(dismissed.has_changed(), Ok(false)) {
                return;
            }
            let chat = &mut app.chat_widget;
            let view = match result {
                Ok(page) => list(
                    chat.thread_id()
                        .map(|_| chat.config_ref().model_provider_id.as_str()),
                    Arc::new(Catalog { backend, page }),
                    server.client_provider_override_for_new_chats(&app.config),
                    &app.keymap.list,
                    notice,
                ),
                Err(error) => load_error("Model providers", error),
            };
            chat.replace_selection_view_if_present(LOADING_VIEW, view);
        }
        Event::Details {
            catalog,
            index,
            dismissed,
        } => {
            if catalog.backend == backend
                && matches!(dismissed.has_changed(), Ok(false))
                && app.overlay.is_none()
                && app.chat_widget.active_view_is(LIST_VIEW)
            {
                let provider = &catalog.page.data[index];
                let mut pager = StaticOverlay::with_title(
                    details(provider),
                    "Model provider details".into(),
                    app.keymap.pager.clone(),
                );
                if provider.can_edit && !matches!(provider.settings, ProviderSettings::OpenAi) {
                    let tx = app.app_event_tx.clone();
                    pager.action = Some(PagerAction {
                        key: key_hint::plain(KeyCode::Enter),
                        label: "Edit configuration",
                        run: Box::new(move || {
                            tx.send(AppEvent::Provider(Event::Edit {
                                catalog,
                                index: Some(index),
                                dismissed,
                            }));
                        }),
                    });
                }
                let _ = tui.enter_alt_screen();
                app.overlay = Some(Overlay::Static(pager));
                tui.frame_requester().schedule_frame();
            }
        }
        Event::Edit {
            catalog,
            index,
            dismissed,
        } => {
            if backend == catalog.backend
                && matches!(dismissed.has_changed(), Ok(false))
                && app.overlay.is_none()
                && app.chat_widget.active_view_is(LIST_VIEW)
            {
                let draft = editor::Draft::new(
                    index.map(|i| catalog.page.data[i].clone()),
                    &catalog.page.data,
                );
                let view = edit_view(app, backend, draft, /*error*/ None);
                app.chat_widget.show_bottom_pane_view(view);
            }
        }
        Event::Write {
            backend,
            dismissed,
            write,
        } => {
            if backend == server.backend_id() && matches!(dismissed.has_changed(), Ok(false)) {
                write.start(app, server);
            }
        }
        Event::Written(completion) => completion.finish(app, backend),
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

pub(crate) fn provider_item(provider: &ModelProviderSummary, default_id: &str) -> SelectionItem {
    let is_default = provider.id == default_id;
    SelectionItem {
        name: provider.name.clone(),
        name_prefix_spans: vec![if is_default {
            "● ".green()
        } else {
            "○ ".dim()
        }],
        description: Some(provider.id.clone()),
        ..Default::default()
    }
}

fn list(
    current: Option<&str>,
    catalog: Arc<Catalog>,
    client_override: Option<String>,
    keymap: &ListKeymap,
    notice: Option<String>,
) -> SelectionViewParams {
    let current_label = current.unwrap_or("No active chat");
    let page = &catalog.page;
    let can_select = page.can_change_default && client_override.is_none();
    let default_label = if client_override.is_some() {
        "Server default"
    } else {
        "Default for new chats"
    };
    let note = match client_override {
        Some(id) => format!("This client's launch settings select {id} for new chats."),
        None if !page.can_change_default => {
            "The server configuration fixes the default provider.".into()
        }
        None => "Use /new to start a chat with the selected provider.".into(),
    };
    let (dismiss, dismissed) = watch::channel(());
    let mut items: Vec<_> = page
        .data
        .iter()
        .enumerate()
        .map(|(index, provider)| {
            let is_default = provider.id == page.default_provider_id;
            let secondary_action = (can_select && !is_default).then(|| {
                let backend = catalog.backend;
                let id = provider.id.clone();
                let name = provider.name.clone();
                let dismissed = dismissed.clone();
                SelectionSecondaryAction {
                    key: key_hint::plain(KeyCode::Char(' ')),
                    footer_hint: provider_hint(keymap, /*select*/ true),
                    action: Box::new(move |tx| {
                        tx.send(AppEvent::Provider(Event::Write {
                            backend,
                            dismissed: dismissed.clone(),
                            write: write::Write::Default {
                                id: id.clone(),
                                name: name.clone(),
                            },
                        }))
                    }),
                }
            });
            let catalog = Arc::clone(&catalog);
            let dismissed = dismissed.clone();
            SelectionItem {
                is_current: current == Some(provider.id.as_str()),
                secondary_action,
                actions: vec![Box::new(move |tx| {
                    tx.send(AppEvent::Provider(Event::Details {
                        catalog: Arc::clone(&catalog),
                        index,
                        dismissed: dismissed.clone(),
                    }));
                })],
                ..provider_item(provider, &page.default_provider_id)
            }
        })
        .collect();
    items.push(SelectionItem {
        name: CREATE_CUSTOM_PROVIDER.into(),
        footer_hint: Some(crate::bottom_pane::popup_consts::picker_hint_line_for_keymap(keymap)),
        actions: vec![Box::new({
            let catalog = Arc::clone(&catalog);
            move |tx| {
                tx.send(AppEvent::Provider(Event::Edit {
                    catalog: Arc::clone(&catalog),
                    index: None,
                    dismissed: dismissed.clone(),
                }));
            }
        })],
        ..Default::default()
    });
    SelectionViewParams {
        view_id: Some(LIST_VIEW),
        title: Some("Model providers".into()),
        subtitle: Some(format!(
            "Current chat: {current_label} · {default_label}: {}",
            page.default_provider_id
        )),
        footer_note: Some(notice.unwrap_or(note).into()),
        footer_hint: Some(provider_hint(keymap, /*select*/ false)),
        items,
        on_cancel: Some(Box::new(move |_| {
            dismiss.send_replace(());
        })),
        allow_input_when_disconnected: true,
        ..Default::default()
    }
}

fn edit_view(
    app: &App,
    backend: BackendId,
    draft: editor::Draft,
    error: Option<String>,
) -> Box<dyn crate::bottom_pane::BottomPaneView> {
    let tx = app.app_event_tx.clone();
    let (owner, _) = watch::channel(());
    editor::view(app, draft, error, move |draft, params| {
        tx.send(AppEvent::Provider(Event::Write {
            backend,
            dismissed: owner.subscribe(),
            write: write::Write::Settings(Box::new((draft, params))),
        }));
    })
}

fn provider_hint(keymap: &ListKeymap, select: bool) -> Line<'static> {
    let keys = [
        (keymap.primary_hint(ListAction::Accept), " details"),
        (
            select.then(|| key_hint::plain(KeyCode::Char(' ')).into()),
            " set default",
        ),
        (keymap.primary_hint(ListAction::Cancel), " back"),
    ];
    let mut hints = Vec::new();
    for (key, label) in keys {
        if let Some(key) = key {
            if !hints.is_empty() {
                hints.push(" · ".dim());
            }
            hints.extend(key.spans());
            hints.push(label.dim());
        }
    }
    hints.into()
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
