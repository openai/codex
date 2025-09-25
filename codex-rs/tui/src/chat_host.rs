use crate::chatwidget::ChatWidget;
use crate::shims::HostApi;
use crate::shims::thread::manager::SessionManager;
use crate::shims::thread::session_id::SessionId;
use codex_core::config::Config;
use codex_core::protocol::AskForApproval;
use codex_core::protocol::Event;
use codex_core::protocol::Op;
use codex_core::protocol::SandboxPolicy;
use codex_core::protocol::TokenUsage;
use codex_file_search::FileMatch;
use codex_protocol::mcp_protocol::ConversationId;
use crossterm::event::KeyEvent;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::WidgetRef;

/// Thin adapter that lets the App talk to either a single ChatWidget or a
/// multi-session manager. Default is Single; Multi is enabled behind `--threads`.
#[allow(clippy::large_enum_variant)]
pub(crate) enum ChatHost {
    Single(ChatWidget),
    Multi(SessionManager),
}

impl ChatHost {
    pub(crate) fn single(widget: ChatWidget) -> Self {
        ChatHost::Single(widget)
    }

    pub(crate) fn from_initial(widget: ChatWidget, enable_multi: bool) -> Self {
        if enable_multi {
            ChatHost::Multi(SessionManager::single(widget, SessionId::new(1)))
        } else {
            ChatHost::Single(widget)
        }
    }

    fn active_widget(&self) -> &ChatWidget {
        match self {
            ChatHost::Single(w) => w,
            ChatHost::Multi(m) => {
                let active = m.active_index();
                let idx = if active < m.len() { active } else { 0 };
                m.session(idx)
                    .map(|h| &h.widget)
                    .unwrap_or_else(|| unreachable!("no sessions available"))
            }
        }
    }

    fn active_widget_mut(&mut self) -> &mut ChatWidget {
        match self {
            ChatHost::Single(w) => w,
            ChatHost::Multi(m) => {
                let active = m.active_index();
                let idx = if active < m.len() { active } else { 0 };
                m.session_mut(idx)
                    .map(|h| &mut h.widget)
                    .unwrap_or_else(|| unreachable!("no sessions available"))
            }
        }
    }

    pub(crate) fn desired_height(&self, width: u16) -> u16 {
        match self {
            ChatHost::Single(w) => w.desired_height(width),
            ChatHost::Multi(_) => self.active_widget().desired_height(width),
        }
    }

    pub(crate) fn maybe_post_pending_notification(&mut self, tui: &mut crate::tui::Tui) {
        match self {
            ChatHost::Single(w) => w.maybe_post_pending_notification(tui),
            ChatHost::Multi(_) => self
                .active_widget_mut()
                .maybe_post_pending_notification(tui),
        }
    }

    pub(crate) fn handle_paste_burst_tick(
        &mut self,
        frame_requester: crate::tui::FrameRequester,
    ) -> bool {
        match self {
            ChatHost::Single(w) => w.handle_paste_burst_tick(frame_requester),
            ChatHost::Multi(_) => self
                .active_widget_mut()
                .handle_paste_burst_tick(frame_requester),
        }
    }

    pub(crate) fn cursor_pos(&self, area: Rect) -> Option<(u16, u16)> {
        match self {
            ChatHost::Single(w) => w.cursor_pos(area),
            ChatHost::Multi(_) => self.active_widget().cursor_pos(area),
        }
    }

    pub(crate) fn handle_key_event(&mut self, key_event: KeyEvent) {
        match self {
            ChatHost::Single(w) => w.handle_key_event(key_event),
            ChatHost::Multi(_) => self.active_widget_mut().handle_key_event(key_event),
        }
    }

    pub(crate) fn on_commit_tick(&mut self) {
        match self {
            ChatHost::Single(w) => w.on_commit_tick(),
            ChatHost::Multi(_) => self.active_widget_mut().on_commit_tick(),
        }
    }

    pub(crate) fn handle_codex_event(&mut self, event: Event) {
        match self {
            ChatHost::Single(w) => w.handle_codex_event(event),
            ChatHost::Multi(_) => self.active_widget_mut().handle_codex_event(event),
        }
    }

    pub(crate) fn apply_file_search_result(&mut self, query: String, matches: Vec<FileMatch>) {
        match self {
            ChatHost::Single(w) => w.apply_file_search_result(query, matches),
            ChatHost::Multi(_) => self
                .active_widget_mut()
                .apply_file_search_result(query, matches),
        }
    }

    pub(crate) fn on_diff_complete(&mut self) {
        match self {
            ChatHost::Single(w) => w.on_diff_complete(),
            ChatHost::Multi(_) => self.active_widget_mut().on_diff_complete(),
        }
    }

    pub(crate) fn add_info_message(&mut self, message: String, hint: Option<String>) {
        match self {
            ChatHost::Single(w) => w.add_info_message(message, hint),
            ChatHost::Multi(_) => self.active_widget_mut().add_info_message(message, hint),
        }
    }

    pub(crate) fn add_error_message(&mut self, message: String) {
        match self {
            ChatHost::Single(w) => w.add_error_message(message),
            ChatHost::Multi(_) => self.active_widget_mut().add_error_message(message),
        }
    }

    pub(crate) fn submit_op(&self, op: Op) {
        match self {
            ChatHost::Single(w) => w.submit_op(op),
            ChatHost::Multi(_) => self.active_widget().submit_op(op),
        }
    }

    pub(crate) fn set_model(&mut self, model: &str) {
        match self {
            ChatHost::Single(w) => w.set_model(model),
            ChatHost::Multi(_) => self.active_widget_mut().set_model(model),
        }
    }

    pub(crate) fn token_usage(&self) -> TokenUsage {
        match self {
            ChatHost::Single(w) => w.token_usage(),
            ChatHost::Multi(_) => self.active_widget().token_usage(),
        }
    }

    pub(crate) fn set_approval_policy(&mut self, policy: AskForApproval) {
        match self {
            ChatHost::Single(w) => w.set_approval_policy(policy),
            ChatHost::Multi(_) => self.active_widget_mut().set_approval_policy(policy),
        }
    }

    pub(crate) fn set_sandbox_policy(&mut self, policy: SandboxPolicy) {
        match self {
            ChatHost::Single(w) => w.set_sandbox_policy(policy),
            ChatHost::Multi(_) => self.active_widget_mut().set_sandbox_policy(policy),
        }
    }

    pub(crate) fn set_reasoning_effort(
        &mut self,
        effort: Option<codex_core::protocol_config_types::ReasoningEffort>,
    ) {
        match self {
            ChatHost::Single(w) => w.set_reasoning_effort(effort),
            ChatHost::Multi(_) => self.active_widget_mut().set_reasoning_effort(effort),
        }
    }

    pub(crate) fn composer_is_empty(&self) -> bool {
        match self {
            ChatHost::Single(w) => w.composer_is_empty(),
            ChatHost::Multi(_) => self.active_widget().composer_is_empty(),
        }
    }

    pub(crate) fn is_normal_backtrack_mode(&self) -> bool {
        match self {
            ChatHost::Single(w) => w.is_normal_backtrack_mode(),
            ChatHost::Multi(_) => self.active_widget().is_normal_backtrack_mode(),
        }
    }

    pub(crate) fn handle_paste(&mut self, text: String) {
        match self {
            ChatHost::Single(w) => w.handle_paste(text),
            ChatHost::Multi(_) => self.active_widget_mut().handle_paste(text),
        }
    }

    pub(crate) fn config_ref(&self) -> &Config {
        match self {
            ChatHost::Single(w) => w.config_ref(),
            ChatHost::Multi(_) => self.active_widget().config_ref(),
        }
    }

    pub(crate) fn conversation_id(&self) -> Option<ConversationId> {
        match self {
            ChatHost::Single(w) => w.conversation_id(),
            ChatHost::Multi(m) => m.conversation_id(),
        }
    }

    pub(crate) async fn show_review_branch_picker(&mut self, cwd: &std::path::Path) {
        match self {
            ChatHost::Single(w) => w.show_review_branch_picker(cwd).await,
            ChatHost::Multi(_) => {
                self.active_widget_mut()
                    .show_review_branch_picker(cwd)
                    .await
            }
        }
    }

    pub(crate) async fn show_review_commit_picker(&mut self, cwd: &std::path::Path) {
        match self {
            ChatHost::Single(w) => w.show_review_commit_picker(cwd).await,
            ChatHost::Multi(_) => {
                self.active_widget_mut()
                    .show_review_commit_picker(cwd)
                    .await
            }
        }
    }

    pub(crate) fn show_review_custom_prompt(&mut self) {
        match self {
            ChatHost::Single(w) => w.show_review_custom_prompt(),
            ChatHost::Multi(_) => self.active_widget_mut().show_review_custom_prompt(),
        }
    }

    pub(crate) fn set_shim_decorations(
        &mut self,
        header_lines: Vec<ratatui::text::Line<'static>>,
        status_line: Option<ratatui::text::Line<'static>>,
    ) {
        match self {
            ChatHost::Single(w) => w.set_shim_decorations(header_lines, status_line),
            ChatHost::Multi(_) => self
                .active_widget_mut()
                .set_shim_decorations(header_lines, status_line),
        }
    }

    pub(crate) fn status_snapshot_at(&self, idx: usize) -> Option<String> {
        match self {
            ChatHost::Single(w) => (idx == 0).then(|| w.status_snapshot()).flatten(),
            ChatHost::Multi(m) => m.session(idx).and_then(|h| h.widget.status_snapshot()),
        }
    }

    pub(crate) fn show_esc_backtrack_hint(&mut self) {
        match self {
            ChatHost::Single(w) => w.show_esc_backtrack_hint(),
            ChatHost::Multi(_) => self.active_widget_mut().show_esc_backtrack_hint(),
        }
    }

    pub(crate) fn clear_esc_backtrack_hint(&mut self) {
        match self {
            ChatHost::Single(w) => w.clear_esc_backtrack_hint(),
            ChatHost::Multi(_) => self.active_widget_mut().clear_esc_backtrack_hint(),
        }
    }

    pub(crate) fn set_composer_text(&mut self, text: String) {
        match self {
            ChatHost::Single(w) => w.set_composer_text(text),
            ChatHost::Multi(_) => self.active_widget_mut().set_composer_text(text),
        }
    }

    pub(crate) fn open_thread_popup_with_hint(
        &mut self,
        items: Vec<crate::bottom_pane::SelectionItem>,
        footer_hint: Option<String>,
    ) {
        match self {
            ChatHost::Single(w) => w.open_thread_popup_with_hint(items, footer_hint),
            ChatHost::Multi(_) => self
                .active_widget_mut()
                .open_thread_popup_with_hint(items, footer_hint),
        }
    }

    pub(crate) fn refresh_thread_popup(
        &mut self,
        items: Vec<crate::bottom_pane::SelectionItem>,
    ) -> bool {
        match self {
            ChatHost::Single(w) => w.refresh_thread_popup(items),
            ChatHost::Multi(_) => self.active_widget_mut().refresh_thread_popup(items),
        }
    }

    // header placement toggle removed (no-op retained intentionally)

    pub(crate) fn switch_to(&mut self, index: usize) -> bool {
        match self {
            ChatHost::Single(_) => index == 0,
            ChatHost::Multi(m) => m.switch_active(index),
        }
    }
}

impl HostApi for ChatHost {
    fn session_count(&self) -> usize {
        match self {
            ChatHost::Single(_) => 1,
            ChatHost::Multi(m) => m.len(),
        }
    }

    fn active_index(&self) -> usize {
        match self {
            ChatHost::Single(_) => 0,
            ChatHost::Multi(m) => m.active_index(),
        }
    }

    fn switch_next(&mut self) -> bool {
        match self {
            ChatHost::Single(_) => false,
            ChatHost::Multi(m) => {
                let len = m.len();
                if len <= 1 {
                    return false;
                }
                let next = (m.active_index() + 1) % len;
                m.switch_active(next)
            }
        }
    }

    fn switch_prev(&mut self) -> bool {
        match self {
            ChatHost::Single(_) => false,
            ChatHost::Multi(m) => {
                let len = m.len();
                if len <= 1 {
                    return false;
                }
                let prev = if m.active_index() == 0 {
                    len - 1
                } else {
                    m.active_index() - 1
                };
                m.switch_active(prev)
            }
        }
    }

    fn active_display_name(&self) -> String {
        match self {
            ChatHost::Single(_) => "main".to_string(),
            ChatHost::Multi(m) => {
                if let Some(h) = m.session(m.active_index()) {
                    h.display_title.clone().unwrap_or_else(|| {
                        if m.active_index() == 0 {
                            "main".to_string()
                        } else {
                            format!("main-{}", m.active_index())
                        }
                    })
                } else {
                    "main".to_string()
                }
            }
        }
    }

    fn active_title(&self) -> Option<String> {
        match self {
            ChatHost::Single(_) => None,
            ChatHost::Multi(m) => m
                .session(m.active_index())
                .map(|h| h.title.clone())
                .filter(|t| !t.is_empty()),
        }
    }

    fn set_active_title(&mut self, title: String) {
        match self {
            ChatHost::Single(_) => {}
            ChatHost::Multi(m) => {
                if let Some(h) = m.session_mut(m.active_index()) {
                    h.title = title;
                }
            }
        }
    }

    fn session_parent_index(&self, idx: usize) -> Option<usize> {
        match self {
            ChatHost::Single(_) => None,
            ChatHost::Multi(m) => m.parent_index_of(idx),
        }
    }

    fn session_title_at(&self, idx: usize) -> Option<String> {
        match self {
            ChatHost::Single(_) => None,
            ChatHost::Multi(m) => m.title_of(idx).map(std::string::ToString::to_string),
        }
    }

    fn submit_op(&mut self, op: Op) {
        ChatHost::submit_op(self, op)
    }
}

impl WidgetRef for &ChatHost {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        match self {
            ChatHost::Single(w) => w.render_ref(area, buf),
            ChatHost::Multi(m) => {
                if let Some(h) = m.session(m.active_index()) {
                    (&h.widget).render_ref(area, buf);
                }
            }
        }
    }
}
