//! Presentation-only user-verification approval prompt.
//!
//! This view intentionally does not share the approval overlay state machine. User verification
//! always requires explicit approval. The controller owns the app-server request and its proof;
//! this view only calls its decision callback and displays the pending state.

use codex_app_server_protocol::RequestId;
use crossterm::event::KeyEvent;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Wrap;

use crate::app::app_server_requests::ResolvedAppServerRequest;
use crate::app_event_sender::AppEventSender;
use crate::bottom_pane::BottomPaneView;
use crate::bottom_pane::CancellationEvent;
use crate::bottom_pane::ListSelectionView;
use crate::bottom_pane::SelectionItem;
use crate::bottom_pane::SelectionViewParams;
use crate::bottom_pane::ViewCompletion;
use crate::bottom_pane::popup_consts::accept_cancel_hint_line;
use crate::key_hint::KeyBinding;
use crate::key_hint::KeyBindingListExt;
use crate::keymap::ApprovalKeymap;
use crate::keymap::ListAction;
use crate::keymap::ListKeymap;
use crate::render::renderable::ColumnRenderable;
use crate::render::renderable::Renderable;

#[derive(Clone, Debug)]
pub(crate) struct UserVerificationRequest {
    pub title: String,
    pub description: String,
    pub thread_label: Option<String>,
    pub server_name: String,
    pub request_id: RequestId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum UserVerificationDecision {
    Verify,
    Cancel,
}

#[derive(Clone)]
struct UserVerificationOption {
    label: String,
    decision: UserVerificationDecision,
    shortcuts: Vec<KeyBinding>,
}

#[derive(Clone, Copy)]
enum UserVerificationState {
    Prompt,
    Waiting,
    Complete(ViewCompletion),
}

pub(crate) struct UserVerificationView {
    request: UserVerificationRequest,
    on_decision: Box<dyn Fn(UserVerificationDecision)>,
    list: ListSelectionView,
    options: Vec<UserVerificationOption>,
    state: UserVerificationState,
    list_keymap: ListKeymap,
    approval_keymap: ApprovalKeymap,
    app_event_tx: AppEventSender,
}

impl UserVerificationView {
    pub(crate) fn new(
        request: UserVerificationRequest,
        app_event_tx: AppEventSender,
        approval_keymap: ApprovalKeymap,
        list_keymap: ListKeymap,
        on_decision: Box<dyn Fn(UserVerificationDecision)>,
    ) -> Self {
        let options = user_verification_options(&approval_keymap);
        let items = options
            .iter()
            .map(|option| SelectionItem {
                name: option.label.clone(),
                display_shortcut: approval_keymap.hint_for_bindings(&option.shortcuts),
                dismiss_on_select: false,
                ..Default::default()
            })
            .collect();
        let header = prompt_header(&request);
        let params = SelectionViewParams {
            footer_note: Some(
                accept_cancel_hint_line(
                    list_keymap.primary_hint(ListAction::Accept),
                    "to confirm",
                    list_keymap.primary_hint(ListAction::Cancel),
                    "to cancel",
                )
                .dim(),
            ),
            items,
            header,
            header_view_all_hint: approval_keymap
                .primary_hint("open_fullscreen", &approval_keymap.open_fullscreen),
            ..SelectionViewParams::picker()
        };
        Self {
            request,
            on_decision,
            list: ListSelectionView::new(params, app_event_tx.clone(), list_keymap.clone()),
            options,
            state: UserVerificationState::Prompt,
            list_keymap,
            approval_keymap,
            app_event_tx,
        }
    }

    fn apply_selection(&mut self, actual_idx: usize) {
        if !matches!(self.state, UserVerificationState::Prompt) {
            return;
        }
        match self.options.get(actual_idx).map(|option| option.decision) {
            Some(UserVerificationDecision::Verify) => self.start_verification(),
            Some(UserVerificationDecision::Cancel) => self.cancel(),
            None => {}
        }
    }

    fn start_verification(&mut self) {
        self.state = UserVerificationState::Waiting;
        (self.on_decision)(UserVerificationDecision::Verify);
    }

    fn cancel(&mut self) {
        if matches!(self.state, UserVerificationState::Complete(_)) {
            return;
        }
        (self.on_decision)(UserVerificationDecision::Cancel);
        self.state = UserVerificationState::Complete(ViewCompletion::Cancelled);
    }

    fn try_handle_shortcut(&mut self, key_event: &KeyEvent) -> bool {
        if self.approval_keymap.open_fullscreen.is_pressed(*key_event) {
            self.app_event_tx.send(
                crate::app_event::AppEvent::FullScreenUserVerificationRequest(self.request.clone()),
            );
            return true;
        }
        if self.list_keymap.cancel.is_pressed(*key_event)
            || self.approval_keymap.cancel.is_pressed(*key_event)
        {
            self.cancel();
            return true;
        }
        if !matches!(self.state, UserVerificationState::Prompt) {
            return true;
        }
        if let Some(idx) = self
            .options
            .iter()
            .position(|option| option.shortcuts.iter().any(|key| key.is_press(*key_event)))
        {
            self.apply_selection(idx);
            return true;
        }
        false
    }
}

impl BottomPaneView for UserVerificationView {
    fn keymap_contexts(&self) -> crate::keymap::KeymapContextSet {
        crate::keymap::KeymapContextSet::new(crate::keymap::KeymapContext::Approval)
            .with(crate::keymap::KeymapContext::List)
    }

    fn handle_key_event(&mut self, key_event: KeyEvent) {
        if self.try_handle_shortcut(&key_event) {
            return;
        }
        self.list.handle_key_event(key_event);
        if let Some(idx) = self.list.take_last_selected_index() {
            self.apply_selection(idx);
        }
    }

    fn on_ctrl_c(&mut self) -> CancellationEvent {
        self.cancel();
        CancellationEvent::Handled
    }

    fn is_complete(&self) -> bool {
        matches!(self.state, UserVerificationState::Complete(_))
    }

    fn completion(&self) -> Option<ViewCompletion> {
        match self.state {
            UserVerificationState::Complete(completion) => Some(completion),
            UserVerificationState::Prompt | UserVerificationState::Waiting => None,
        }
    }

    fn matches_app_server_request(&self, request: &ResolvedAppServerRequest) -> bool {
        matches!(request, ResolvedAppServerRequest::McpElicitation { server_name, request_id }
            if server_name == &self.request.server_name && request_id == &self.request.request_id)
    }

    fn dismiss_app_server_request(&mut self, request: &ResolvedAppServerRequest) -> bool {
        if self.matches_app_server_request(request) {
            self.state = UserVerificationState::Complete(ViewCompletion::Accepted);
            true
        } else {
            false
        }
    }

    fn terminal_title_requires_action(&self) -> bool {
        !self.is_complete()
    }
}

impl Renderable for UserVerificationView {
    fn desired_height(&self, width: u16) -> u16 {
        if matches!(self.state, UserVerificationState::Waiting) {
            waiting_view(&self.request, &self.list_keymap).desired_height(width)
        } else {
            self.list.desired_height(width)
        }
    }

    fn render(&self, area: Rect, buf: &mut Buffer) {
        if matches!(self.state, UserVerificationState::Waiting) {
            waiting_view(&self.request, &self.list_keymap).render(area, buf);
        } else {
            self.list.render(area, buf);
        }
    }

    fn cursor_pos(&self, area: Rect) -> Option<(u16, u16)> {
        matches!(self.state, UserVerificationState::Prompt)
            .then(|| self.list.cursor_pos(area))
            .flatten()
    }
}

fn user_verification_options(keymap: &ApprovalKeymap) -> Vec<UserVerificationOption> {
    vec![
        UserVerificationOption {
            label: "Verify and approve".to_string(),
            decision: UserVerificationDecision::Verify,
            shortcuts: keymap.approve.clone(),
        },
        UserVerificationOption {
            label: "Cancel this request".to_string(),
            decision: UserVerificationDecision::Cancel,
            shortcuts: keymap.cancel.clone(),
        },
    ]
}

pub(crate) fn prompt_header(request: &UserVerificationRequest) -> Box<dyn Renderable> {
    let mut header = ColumnRenderable::new();
    header.push(Paragraph::new(request.title.clone().bold()).wrap(Wrap { trim: false }));
    header.push(Line::from(""));
    header.push(request_details(request));
    Box::new(header)
}

fn request_details(request: &UserVerificationRequest) -> Box<dyn Renderable> {
    let mut lines = Vec::new();
    if let Some(thread_label) = &request.thread_label {
        lines.push(Line::from(vec![
            "Thread: ".into(),
            thread_label.clone().bold(),
        ]));
        lines.push(Line::from(""));
    }
    lines.extend([
        Line::from(vec!["Server: ".into(), request.server_name.clone().bold()]),
        Line::from(""),
        Line::from(request.description.clone()),
    ]);
    Box::new(Paragraph::new(lines).wrap(Wrap { trim: false }))
}

fn waiting_view(request: &UserVerificationRequest, keymap: &ListKeymap) -> Box<dyn Renderable> {
    let mut view = ColumnRenderable::new();
    view.push(Paragraph::new("Waiting for verification…".bold()).wrap(Wrap { trim: false }));
    view.push(Line::from(""));
    view.push(request_details(request));
    view.push(Line::from(""));
    view.push(
        Paragraph::new(accept_cancel_hint_line(
            /*accept*/ None,
            "",
            keymap.primary_hint(ListAction::Cancel),
            "to cancel this request",
        ))
        .wrap(Wrap { trim: false }),
    );
    Box::new(view)
}

#[cfg(test)]
#[path = "user_verification_tests.rs"]
mod tests;

impl super::BottomPane {
    pub(crate) fn push_user_verification_request(
        &mut self,
        thread_id: codex_protocol::ThreadId,
        request: UserVerificationRequest,
    ) {
        // App-server request IDs are shared across threads, unlike raw MCP request IDs.
        let request_key = ResolvedAppServerRequest::McpElicitation {
            server_name: request.server_name.clone(),
            request_id: request.request_id.clone(),
        };
        if self
            .view_stack
            .iter()
            .any(|view| view.matches_app_server_request(&request_key))
        {
            return;
        }
        let app_event_tx = self.app_event_tx.clone();
        let server_name = request.server_name.clone();
        let request_id = request.request_id.clone();
        let view = UserVerificationView::new(
            request,
            self.app_event_tx.clone(),
            self.keymap.approval.clone(),
            self.keymap.list.clone(),
            Box::new(move |decision| match decision {
                UserVerificationDecision::Verify => {
                    app_event_tx.send(crate::app_event::AppEvent::UserVerificationApproved {
                        thread_id,
                        server_name: server_name.clone(),
                        request_id: request_id.clone(),
                    });
                }
                UserVerificationDecision::Cancel => app_event_tx.resolve_user_verification(
                    thread_id,
                    server_name.clone(),
                    request_id.clone(),
                    crate::app_command::UserVerificationResponse::Cancel,
                ),
            }),
        );
        self.pause_status_timer_for_modal();
        self.set_composer_input_enabled(
            /*enabled*/ false,
            Some("Complete user verification to continue.".to_string()),
        );
        self.push_view(Box::new(view));
    }
}
