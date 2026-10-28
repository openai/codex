//! Google Cloud setup collects provider settings and delegates validation and persistence to
//! app-server. Setup stays in progress until the server finishes writing the configuration.

use super::auth::AuthModeWidget;
use super::auth::SignInState;
use super::auth::onboarding_request_id;
use super::keys;
use super::provider_setup::FieldDisplay;
use super::provider_setup::ProviderInput;
use crate::key_hint::KeyBindingListExt;
use crate::wrapping::word_wrap_lines;
use codex_app_server_client::TypedRequestError;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::GcpSetupParams;
use codex_app_server_protocol::GcpSetupResponse;
use codex_app_server_protocol::RequestId;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::prelude::Widget;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use std::sync::PoisonError;

#[derive(Clone, Default)]
pub(super) struct GcpState {
    view: GcpView,
    project_id: ProviderInput,
    location: ProviderInput,
    credential_file: ProviderInput,
}

#[derive(Clone, Default)]
enum GcpView {
    #[default]
    Project,
    Location,
    Credentials,
    Configuring(RequestId),
}

impl GcpState {
    pub(super) fn is_text_entry_active(&self) -> bool {
        !matches!(self.view, GcpView::Configuring(_))
    }

    pub(super) fn is_configuring(&self, id: &RequestId) -> bool {
        matches!(&self.view, GcpView::Configuring(active) if active == id)
    }

    pub(super) fn active_input_mut(&mut self) -> Option<&mut ProviderInput> {
        match self.view {
            GcpView::Project => Some(&mut self.project_id),
            GcpView::Location => Some(&mut self.location),
            GcpView::Credentials => Some(&mut self.credential_file),
            GcpView::Configuring(_) => None,
        }
    }

    pub(super) fn render(&self, area: Rect, buf: &mut Buffer, error: Option<String>) {
        let mut lines: Vec<Line> = vec!["> Set up Google Cloud Vertex AI".bold().into(), "".into()];
        let (guidance, field) = match &self.view {
            GcpView::Project => (
                "Enter the Google Cloud project ID to use with Vertex AI.",
                Some((&self.project_id, "  Project ID: ", "my-project")),
            ),
            GcpView::Location => (
                "Press Enter to use global, or enter a Vertex AI location.",
                Some((&self.location, "  Location (optional): ", "global")),
            ),
            GcpView::Credentials => (
                "Enter an absolute file path on the host running Codex, or press Enter to use Application Default Credentials (ADC).",
                Some((&self.credential_file, "  Credential file (optional): ", "")),
            ),
            GcpView::Configuring(_) => ("Validating credentials and saving configuration…", None),
        };
        lines.push(format!("  {guidance}").into());
        if let Some((input, label, placeholder)) = field {
            lines.push("".into());
            lines.push(input.line(label, placeholder, FieldDisplay::Focused, area.width));
        }
        if matches!(self.view, GcpView::Credentials) {
            lines.push("".into());
            lines.push(
                "  To set up ADC with the Google Cloud CLI, run"
                    .dim()
                    .into(),
            );
            lines.push("  gcloud auth application-default login".cyan().into());
        }
        let mut footer = if self.is_text_entry_active() {
            vec![Line::from(vec![
                "  ".into(),
                keys::CONFIRM[0].into(),
                if matches!(self.view, GcpView::Credentials) {
                    " set up"
                } else {
                    " continue"
                }
                .dim(),
                " · ".dim(),
                keys::CANCEL[0].into(),
                " back".dim(),
            ])]
        } else {
            Vec::new()
        };
        if let Some(error) = error {
            footer.push("".into());
            footer.push(error.red().into());
        }
        let footer = word_wrap_lines(footer, usize::from(area.width));
        let footer_height = u16::try_from(footer.len())
            .unwrap_or(u16::MAX)
            .min(area.height);
        Paragraph::new(word_wrap_lines(lines, usize::from(area.width))).render(
            Rect {
                height: area.height.saturating_sub(footer_height),
                ..area
            },
            buf,
        );
        Paragraph::new(footer).render(
            Rect {
                y: area.bottom().saturating_sub(footer_height),
                height: footer_height,
                ..area
            },
            buf,
        );
    }
}

impl AuthModeWidget {
    pub(super) fn handle_gcp_key_event(&mut self, key_event: &KeyEvent) -> bool {
        let mut guard = self
            .sign_in_state
            .write()
            .unwrap_or_else(PoisonError::into_inner);
        let SignInState::Gcp(state) = &mut *guard else {
            return false;
        };
        // A dispatched setup can persist settings. Do not offer cancellation or allow a
        // second submission until its result arrives.
        if key_event.kind == KeyEventKind::Release || !state.is_text_entry_active() {
            return true;
        }
        if keys::CANCEL.is_pressed(*key_event) {
            match state.view {
                GcpView::Project => *guard = SignInState::PickMode,
                GcpView::Location => state.view = GcpView::Project,
                GcpView::Credentials => state.view = GcpView::Location,
                GcpView::Configuring(_) => {}
            }
        } else if keys::CONFIRM.is_pressed(*key_event) {
            if key_event.kind != KeyEventKind::Press {
                return true;
            }
            match state.view {
                GcpView::Project if state.project_id.value().is_none() => {
                    *self.error.write().unwrap_or_else(PoisonError::into_inner) =
                        Some("Enter a Google Cloud project ID.".to_string());
                    self.request_frame.schedule_frame();
                    return true;
                }
                GcpView::Project => state.view = GcpView::Location,
                GcpView::Location => state.view = GcpView::Credentials,
                GcpView::Credentials => {
                    let request_id = onboarding_request_id();
                    let retry = SignInState::Gcp(state.clone());
                    state.view = GcpView::Configuring(request_id.clone());
                    let params = GcpSetupParams {
                        project_id: state.project_id.value().unwrap_or_default(),
                        location: state.location.value(),
                        credential_file: state.credential_file.value(),
                    };
                    let request_handle = self.app_server_request_handle.clone();
                    self.spawn_provider_setup(request_id.clone(), retry, async move {
                        request_handle
                            .request_typed::<GcpSetupResponse>(ClientRequest::GcpSetup {
                                request_id,
                                params,
                            })
                            .await
                            .map(|_| ())
                            .map_err(|err| match err {
                                TypedRequestError::Server { source, .. } => source.message,
                                err => err.to_string(),
                            })
                    });
                }
                GcpView::Configuring(_) => {}
            }
        } else if let Some(input) = state.active_input_mut() {
            input.input(*key_event);
        }
        *self.error.write().unwrap_or_else(PoisonError::into_inner) = None;
        drop(guard);
        self.request_frame.schedule_frame();
        true
    }
}
