//! Gateway sign-in uses the settings picker and form. Creating a gateway returns to the
//! picker; only explicitly selecting a saved provider completes sign-in.

use super::auth::AuthModeWidget;
use super::auth::SignInState;
use super::keys;
use super::provider_setup::ProviderInput;
use crate::app::provider::CREATE_CUSTOM_PROVIDER;
use crate::app::provider::Draft;
use crate::app::provider::ProviderForm;
use crate::app::provider::provider_item;
use crate::app::provider::rpc;
use crate::app_event_sender::AppEventSender;
use crate::bottom_pane::BottomPaneView;
use crate::bottom_pane::ListSelectionView;
use crate::bottom_pane::SelectionItem;
use crate::bottom_pane::SelectionViewParams;
use crate::bottom_pane::popup_consts::accept_cancel_hint_line;
use crate::key_hint::KeyBindingListExt;
use crate::keymap::ListAction;
use crate::keymap::RuntimeKeymap;
use crate::render::renderable::Renderable;
use codex_app_server_client::AppServerRequestHandle;
use codex_app_server_protocol::ModelProviderConfigWriteParams;
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

pub(super) const USE_CUSTOM_PROVIDER: &str = "Use a custom provider or gateway";

// Requests stay on the onboarding thread so a redraw can stop polling without losing the reply.
pub(super) enum GatewayStep {
    Loading(LocalBoxFuture<'static, Self>),
    LoadError(Box<ListSelectionView>),
    Picking(Box<Picker>),
    Editing {
        picker: Box<Picker>,
        form: Box<ProviderForm>,
    },
    Saving(LocalBoxFuture<'static, Self>),
    Selecting(LocalBoxFuture<'static, Result<String, Self>>),
}

impl GatewayStep {
    fn load(handle: AppServerRequestHandle, notice: Option<String>) -> Self {
        Self::Loading(Box::pin(async move {
            match rpc::list_providers(&handle).await {
                Ok(mut page) if page.can_change_default => {
                    page.data
                        .retain(|p| matches!(p.settings, ProviderSettings::CustomGateway { .. }));
                    Self::Picking(Box::new(Picker::new(page, /*selected*/ None, notice)))
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
            crate::app::provider::load_error(USE_CUSTOM_PROVIDER, message),
            AppEventSender::new(tx),
            RuntimeKeymap::defaults().list,
        )))
    }

    fn save(
        handle: AppServerRequestHandle,
        picker: Box<Picker>,
        mut form: Box<ProviderForm>,
        params: ModelProviderConfigWriteParams,
    ) -> Self {
        GatewayStep::Saving(Box::pin(async move {
            match rpc::write_provider(&handle, params).await {
                Ok(rpc::ProviderWrite::Saved(provider)) => {
                    let mut page = picker.page;
                    let selected = page.data.len();
                    page.data.push(provider);
                    GatewayStep::Picking(Box::new(Picker::new(
                        page,
                        Some(selected),
                        /*notice*/ None,
                    )))
                }
                Ok(rpc::ProviderWrite::UnconfirmedCreate) => {
                    GatewayStep::load(handle, Some(rpc::UNCONFIRMED_CREATE.into()))
                }
                Err(message) => {
                    form.error = Some(message);
                    GatewayStep::Editing { picker, form }
                }
            }
        }))
    }

    pub(super) fn active_input_mut(&mut self) -> Option<&mut ProviderInput> {
        match self {
            Self::Editing { form, .. } => form.active_input_mut(),
            _ => None,
        }
    }

    pub(super) fn is_text_entry_active(&self) -> bool {
        matches!(self, Self::Editing { form, .. } if form.active_input().is_some())
    }

    pub(super) fn render(&self, area: Rect, buf: &mut Buffer) {
        match self {
            Self::Loading(_) => Widget::render(Line::from("Loading gateways…".dim()), area, buf),
            Self::Editing { form, .. } => form.render(area, buf),
            Self::Saving(_) | Self::Selecting(_) => {
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
        let mut items: Vec<_> = page
            .data
            .iter()
            .map(|p| provider_item(p, &page.default_provider_id))
            .collect();
        let keymap = RuntimeKeymap::defaults().list;
        let hint = |accept| {
            accept_cancel_hint_line(
                keymap.primary_hint(ListAction::Accept),
                accept,
                keymap.primary_hint(ListAction::Cancel),
                "to go back",
            )
        };
        items.push(SelectionItem {
            name: CREATE_CUSTOM_PROVIDER.into(),
            footer_hint: Some(hint("to create")),
            ..Default::default()
        });
        // Like the other startup pickers, consume the selection directly before the app event loop exists.
        let (tx, _) = unbounded_channel();
        let list = ListSelectionView::new(
            SelectionViewParams {
                title: Some(USE_CUSTOM_PROVIDER.into()),
                footer_hint: Some(hint("to use as default")),
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
    fn activate(self: Box<Self>, index: usize, handle: AppServerRequestHandle) -> GatewayStep {
        let Some(provider) = self.page.data.get(index) else {
            let form = ProviderForm::new(
                Draft::new(/*provider*/ None, &self.page.data),
                /*error*/ None,
            );
            return GatewayStep::Editing {
                picker: self,
                form: Box::new(form),
            };
        };
        let id = provider.id.clone();
        GatewayStep::Selecting(Box::pin(async move {
            let message = match rpc::set_default_provider(&handle, &id).await {
                Ok(response) => match response.status {
                    WriteStatus::Ok => return Ok(id),
                    WriteStatus::OkOverridden => rpc::override_reason(response),
                },
                Err(err) => err.to_string(),
            };
            Err(GatewayStep::Picking(Box::new(Picker::new(
                self.page,
                Some(index),
                Some(message),
            ))))
        }))
    }
}

impl AuthModeWidget {
    pub(super) fn start_gateway_picker(&mut self) {
        self.set_error(/*message*/ None);
        self.gateway = Some(GatewayStep::load(
            self.app_server_request_handle.clone(),
            /*notice*/ None,
        ));
        self.request_frame.schedule_frame();
    }

    pub(super) async fn advance_gateway_request(&mut self) {
        self.gateway = match &mut self.gateway {
            Some(GatewayStep::Loading(request) | GatewayStep::Saving(request)) => {
                Some(request.await)
            }
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
            None
            | Some(
                GatewayStep::Picking(_) | GatewayStep::LoadError(_) | GatewayStep::Editing { .. },
            ) => std::future::pending().await,
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
            GatewayStep::Editing { picker, .. } if keys::CANCEL.is_pressed(key) => {
                Some(GatewayStep::Picking(picker))
            }
            GatewayStep::Editing { picker, mut form } => {
                if let Some(params) = form.input(key) {
                    self.guard_activation(key);
                    let handle = self.app_server_request_handle.clone();
                    Some(GatewayStep::save(handle, picker, form, params))
                } else {
                    Some(GatewayStep::Editing { picker, form })
                }
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
                    Some(picker.activate(index, self.app_server_request_handle.clone()))
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
