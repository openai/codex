//! Sign in by selecting a saved custom gateway as the default provider.

use super::auth::AuthModeWidget;
use super::auth::SignInState;
use super::keys;
use crate::app::provider::provider_item;
use crate::app::provider::rpc;
use crate::app_event_sender::AppEventSender;
use crate::bottom_pane::BottomPaneView;
use crate::bottom_pane::ListSelectionView;
use crate::bottom_pane::SelectionViewParams;
use crate::bottom_pane::popup_consts::accept_cancel_hint_line;
use crate::key_hint::KeyBindingListExt;
use crate::keymap::ListAction;
use crate::keymap::RuntimeKeymap;
use crate::render::renderable::Renderable;
use codex_app_server_client::AppServerRequestHandle;
use codex_app_server_protocol::ModelProviderListResponse;
use codex_app_server_protocol::ProviderSettings;
use codex_app_server_protocol::WriteStatus;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use futures::future::LocalBoxFuture;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Widget;
use tokio::sync::mpsc::unbounded_channel;

// Requests stay on the onboarding thread so a redraw can stop polling without losing the reply.
pub(super) enum GatewayStep {
    Loading(LocalBoxFuture<'static, Self>),
    LoadError(Box<ListSelectionView>),
    Picking(Box<Picker>),
    Selecting(LocalBoxFuture<'static, Result<String, Self>>),
}

impl GatewayStep {
    fn load(handle: AppServerRequestHandle) -> Self {
        Self::Loading(Box::pin(async move {
            match rpc::list_providers(&handle).await {
                Ok(mut page) if page.can_change_default => {
                    page.data
                        .retain(|p| matches!(p.settings, ProviderSettings::CustomGateway { .. }));
                    if page.data.is_empty() {
                        Self::load_error("No custom gateways are configured.".into())
                    } else {
                        Self::Picking(Box::new(Picker::new(
                            page, /*selected*/ None, /*notice*/ None,
                        )))
                    }
                }
                Ok(_) => Self::load_error(
                    "The default provider is set by configuration policy or a launch override."
                        .into(),
                ),
                Err(message) => Self::load_error(message),
            }
        }))
    }

    fn load_error(message: String) -> Self {
        let (tx, _) = unbounded_channel();
        Self::LoadError(Box::new(ListSelectionView::new(
            crate::app::provider::load_error("Use a custom gateway", message),
            AppEventSender::new(tx),
            RuntimeKeymap::defaults().list,
        )))
    }

    pub(super) fn render(&self, area: Rect, buf: &mut Buffer) {
        match self {
            Self::Loading(_) => Widget::render(Line::from("Loading gateways…".dim()), area, buf),
            Self::Selecting(_) => {
                Widget::render(Line::from("Saving gateway settings…".dim()), area, buf)
            }
            Self::LoadError(list) => list.render(area, buf),
            Self::Picking(picker) => picker.list.render(area, buf),
        }
    }
}

pub(super) struct Picker {
    page: ModelProviderListResponse,
    list: ListSelectionView,
}

impl Picker {
    fn new(
        page: ModelProviderListResponse,
        selected: Option<usize>,
        notice: Option<String>,
    ) -> Self {
        let items = page
            .data
            .iter()
            .map(|p| provider_item(p, &page.default_provider_id))
            .collect();
        let keymap = RuntimeKeymap::defaults().list;
        // Like the other startup pickers, consume the selection directly before the app event loop exists.
        let (tx, _) = unbounded_channel();
        let list = ListSelectionView::new(
            SelectionViewParams {
                title: Some("Use a custom gateway".into()),
                footer_hint: Some(accept_cancel_hint_line(
                    keymap.primary_hint(ListAction::Accept),
                    "to use as default",
                    keymap.primary_hint(ListAction::Cancel),
                    "to go back",
                )),
                footer_note: notice.map(Into::into),
                initial_selected_idx: selected,
                auto_number_shortcuts: false,
                items,
                ..Default::default()
            },
            AppEventSender::new(tx),
            keymap,
        );
        Self { page, list }
    }
}

impl AuthModeWidget {
    pub(super) fn start_gateway_picker(&mut self) {
        self.set_error(/*message*/ None);
        self.gateway = Some(GatewayStep::load(self.app_server_request_handle.clone()));
        self.request_frame.schedule_frame();
    }

    pub(super) async fn advance_gateway_request(&mut self) {
        self.gateway = match &mut self.gateway {
            Some(GatewayStep::Loading(request)) => Some(request.await),
            Some(GatewayStep::Selecting(request)) => match request.await {
                Ok(id) => {
                    *self
                        .sign_in_state
                        .write()
                        .unwrap_or_else(std::sync::PoisonError::into_inner) =
                        SignInState::GatewaySelected(id);
                    None
                }
                Err(picker) => Some(picker),
            },
            None | Some(GatewayStep::Picking(_) | GatewayStep::LoadError(_)) => {
                std::future::pending().await
            }
        };
        self.request_frame.schedule_frame();
    }

    pub(super) fn handle_gateway_key_event(&mut self, key: KeyEvent) -> bool {
        let Some(step) = self.gateway.take() else {
            return false;
        };
        self.gateway = match step {
            GatewayStep::Loading(_) | GatewayStep::Picking(_) | GatewayStep::LoadError(_)
                if keys::CANCEL.is_pressed(key) =>
            {
                None
            }
            GatewayStep::LoadError(mut view) => {
                view.handle_key_event(key);
                if view.take_last_selected_index().is_some() {
                    self.guard_activation(key);
                    None
                } else {
                    Some(GatewayStep::LoadError(view))
                }
            }
            GatewayStep::Picking(mut picker)
                if key.kind == KeyEventKind::Press || !keys::CONFIRM.is_pressed(key) =>
            {
                picker.list.handle_key_event(key);
                if let Some(index) = picker.list.take_last_selected_index() {
                    self.guard_activation(key);
                    let id = picker.page.data[index].id.clone();
                    let handle = self.app_server_request_handle.clone();
                    Some(GatewayStep::Selecting(Box::pin(async move {
                        let message = match rpc::set_default_provider(&handle, &id).await {
                            Ok(response) => match response.status {
                                WriteStatus::Ok => return Ok(id),
                                WriteStatus::OkOverridden => rpc::override_reason(response),
                            },
                            Err(err) => err.to_string(),
                        };
                        Err(GatewayStep::Picking(Box::new(Picker::new(
                            picker.page,
                            Some(index),
                            Some(message),
                        ))))
                    })))
                } else {
                    Some(GatewayStep::Picking(picker))
                }
            }
            step => Some(step),
        };
        self.request_frame.schedule_frame();
        true
    }
}
