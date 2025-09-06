use crate::app::App;
use crate::backtrack_helpers;
use crate::pager_overlay::Overlay;
use crate::tui;
use crate::tui::TuiEvent;
use codex_core::protocol::ConversationHistoryResponseEvent;
use color_eyre::eyre::Result;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
// ===== !Modify Start: 引入清空提示的时间控制 =====
// !Desc: 为“Esc 再按一次清空”增加 1s 计时窗口，需要时间类型支持。
// !AI_GUIDANCE: 若上游提供统一计时工具，请替换为上游工具，语义保持 1s 窗口。
use std::time::Duration;
use std::time::Instant;
// ===== !Modify End: 引入清空提示的时间控制 =====

// ===== !Modify Start: 文件级说明（自定义回退入口） =====
// !Modify[owner=yoyooyooo]: 自定义“用户提问节点选择器”入口：双击 Esc（第二次 Esc）在正常回溯
// 模式下打开仅含用户提问节点的列表弹窗；↑/↓ 选择，Enter 回退并预填文本，Esc 取消。
// 原 Transcript 仍可通过 Ctrl+T 打开；在转录中 Esc/Enter 的回溯预览保持可用。
// !AI_GUIDANCE: 若与上游实现冲突，优先考虑共存（如改键/提供配置开关）。若无法共存，请先询问
// 用户是否保留本自定义逻辑，再做替换或移除。
// ===== !Modify End: 文件级说明（自定义回退入口） =====
/// Aggregates all backtrack-related state used by the App.
#[derive(Default)]
pub(crate) struct BacktrackState {
    /// True when Esc has primed backtrack mode in the main view.
    pub(crate) primed: bool,
    // !Modify: 清空提示窗口截止时间（1s 内二次 Esc 才清空）
    /// If Some, indicates that within this deadline a second Esc should clear the composer.
    pub(crate) clear_primed_until: Option<Instant>,
    /// Session id of the base conversation to fork from.
    pub(crate) base_id: Option<uuid::Uuid>,
    /// Current step count (Nth last user message).
    pub(crate) count: usize,
    /// True when the transcript overlay is showing a backtrack preview.
    pub(crate) overlay_preview_active: bool,
    /// Pending fork request: (base_id, drop_count, prefill).
    pub(crate) pending: Option<(uuid::Uuid, usize, String)>,
}

impl App {
    /// Route overlay events when transcript overlay is active.
    /// - If backtrack preview is active: Esc steps selection; Enter confirms.
    /// - Otherwise: Esc begins preview; all other events forward to overlay.
    ///   interactions (Esc to step target, Enter to confirm) and overlay lifecycle.
    pub(crate) async fn handle_backtrack_overlay_event(
        &mut self,
        tui: &mut tui::Tui,
        event: TuiEvent,
    ) -> Result<bool> {
        if self.backtrack.overlay_preview_active {
            match event {
                TuiEvent::Key(KeyEvent {
                    code: KeyCode::Esc,
                    kind: KeyEventKind::Press | KeyEventKind::Repeat,
                    ..
                }) => {
                    self.overlay_step_backtrack(tui, event)?;
                    Ok(true)
                }
                TuiEvent::Key(KeyEvent {
                    code: KeyCode::Enter,
                    kind: KeyEventKind::Press,
                    ..
                }) => {
                    self.overlay_confirm_backtrack(tui);
                    Ok(true)
                }
                // Catchall: forward any other events to the overlay widget.
                _ => {
                    self.overlay_forward_event(tui, event)?;
                    Ok(true)
                }
            }
        } else if let TuiEvent::Key(KeyEvent {
            code: KeyCode::Esc,
            kind: KeyEventKind::Press | KeyEventKind::Repeat,
            ..
        }) = event
        {
            // First Esc in transcript overlay: begin backtrack preview at latest user message.
            self.begin_overlay_backtrack_preview(tui);
            Ok(true)
        } else {
            // Not in backtrack mode: forward events to the overlay widget.
            self.overlay_forward_event(tui, event)?;
            Ok(true)
        }
    }

    /// Handle global Esc presses for backtracking when no overlay is present.
    pub(crate) fn handle_backtrack_esc_key(&mut self, tui: &mut tui::Tui) {
        // ===== !Modify Start: Esc 双击清空（有输入）与回退（空输入）分派 =====
        // !Desc: 在无覆盖层时：
        //        - 输入框非空：第一次 Esc 显示 1s 清空提示窗口；1s 内第二次 Esc 清空文本。
        //        - 输入框为空：保持原回退逻辑（prime → 第二次 Esc 打开选择器）。
        //        在覆盖层回退预览中，Esc 仍然是向更早的用户消息“步进”。
        // !AI_GUIDANCE: 若上游引入新的 Esc 行为，请在“非空=清空提示/空=回退”判定上共存；
        //        无法共存时优先征询用户，或提供配置开关以切换。
        if self.overlay.is_none() {
            if !self.chat_widget.composer_is_empty() {
                // 文本非空 → 清空模式：第一次 Esc 显示提示并打开 1s 窗口；
                // 1s 内第二次 Esc 清空输入框。
                let now = Instant::now();
                if let Some(deadline) = self.backtrack.clear_primed_until
                    && now <= deadline
                {
                    // 命中第二次 Esc：清空并清除提示/状态。
                    self.chat_widget.clear_composer_text();
                    self.chat_widget.clear_esc_clear_hint();
                    self.backtrack.clear_primed_until = None;
                    // 不进入回溯 primed，直接返回。
                    return;
                }

                // 第一次 Esc：提示 1s，设置清空窗口。
                self.backtrack.clear_primed_until = Some(now + Duration::from_secs(1));
                self.chat_widget
                    .show_esc_clear_hint_for(Duration::from_secs(1));
                return;
            }

            // 文本为空 → 回溯模式：按原双击逻辑。
            if !self.backtrack.primed {
                self.prime_backtrack();
            } else {
                // 第二次 Esc → 打开用户提问节点选择器。
                self.open_user_nodes_picker();
            }
        } else if self.backtrack.overlay_preview_active {
            self.step_backtrack_and_highlight(tui);
        }
        // ===== !Modify End: Esc 双击清空（有输入）与回退（空输入）分派 =====
    }

    /// Stage a backtrack and request conversation history from the agent.
    pub(crate) fn request_backtrack(
        &mut self,
        prefill: String,
        base_id: uuid::Uuid,
        drop_last_messages: usize,
    ) {
        self.backtrack.pending = Some((base_id, drop_last_messages, prefill));
        self.app_event_tx.send(crate::app_event::AppEvent::CodexOp(
            codex_core::protocol::Op::GetHistory,
        ));
    }

    /// Open transcript overlay (enters alternate screen and shows full transcript).
    // !Modify: 避免未使用警告（开放给测试/后续扩展）
    #[allow(dead_code)]
    pub(crate) fn open_transcript_overlay(&mut self, tui: &mut tui::Tui) {
        let _ = tui.enter_alt_screen();
        self.overlay = Some(Overlay::new_transcript(self.transcript_lines.clone()));
        tui.frame_requester().schedule_frame();
    }

    /// Close transcript overlay and restore normal UI.
    pub(crate) fn close_transcript_overlay(&mut self, tui: &mut tui::Tui) {
        let _ = tui.leave_alt_screen();
        let was_backtrack = self.backtrack.overlay_preview_active;
        if !self.deferred_history_lines.is_empty() {
            let lines = std::mem::take(&mut self.deferred_history_lines);
            tui.insert_history_lines(lines);
        }
        self.overlay = None;
        self.backtrack.overlay_preview_active = false;
        if was_backtrack {
            // Ensure backtrack state is fully reset when overlay closes (e.g. via 'q').
            self.reset_backtrack_state();
        }
    }

    /// Re-render the full transcript into the terminal scrollback in one call.
    /// Useful when switching sessions to ensure prior history remains visible.
    pub(crate) fn render_transcript_once(&mut self, tui: &mut tui::Tui) {
        if !self.transcript_lines.is_empty() {
            tui.insert_history_lines(self.transcript_lines.clone());
        }
    }

    /// Initialize backtrack state and show composer hint.
    fn prime_backtrack(&mut self) {
        // ===== !Modify Start: prime 时仅在输入为空显示回退提示 =====
        // !Desc: 避免在有输入的情况下显示“Esc edit prev”，与清空提示相互覆盖导致误导。
        // !AI_GUIDANCE: 若上游统一了提示展示层，请合并逻辑，保持“仅空输入显示回退提示”。
        self.backtrack.primed = true;
        self.backtrack.count = 0;
        self.backtrack.base_id = self.chat_widget.session_id();
        if self.chat_widget.composer_is_empty() {
            self.chat_widget.show_esc_backtrack_hint();
        }
        // ===== !Modify End: prime 时仅在输入为空显示回退提示 =====
    }

    /// Open overlay and begin backtrack preview flow (first step + highlight).
    // !Modify: 避免未使用警告（开放给测试/后续扩展）
    #[allow(dead_code)]
    fn open_backtrack_preview(&mut self, tui: &mut tui::Tui) {
        self.open_transcript_overlay(tui);
        self.backtrack.overlay_preview_active = true;
        // Composer is hidden by overlay; clear its hint.
        self.chat_widget.clear_esc_backtrack_hint();
        self.step_backtrack_and_highlight(tui);
    }

    /// When overlay is already open, begin preview mode and select latest user message.
    fn begin_overlay_backtrack_preview(&mut self, tui: &mut tui::Tui) {
        self.backtrack.primed = true;
        self.backtrack.base_id = self.chat_widget.session_id();
        self.backtrack.overlay_preview_active = true;
        let sel = self.compute_backtrack_selection(tui, 1);
        self.apply_backtrack_selection(sel);
        tui.frame_requester().schedule_frame();
    }

    /// Step selection to the next older user message and update overlay.
    fn step_backtrack_and_highlight(&mut self, tui: &mut tui::Tui) {
        let next = self.backtrack.count.saturating_add(1);
        let sel = self.compute_backtrack_selection(tui, next);
        self.apply_backtrack_selection(sel);
        tui.frame_requester().schedule_frame();
    }

    /// Compute normalized target, scroll offset, and highlight for requested step.
    fn compute_backtrack_selection(
        &self,
        tui: &tui::Tui,
        requested_n: usize,
    ) -> (usize, Option<usize>, Option<(usize, usize)>) {
        let nth = backtrack_helpers::normalize_backtrack_n(&self.transcript_lines, requested_n);
        let header_idx =
            backtrack_helpers::find_nth_last_user_header_index(&self.transcript_lines, nth);
        let offset = header_idx.map(|idx| {
            backtrack_helpers::wrapped_offset_before(
                &self.transcript_lines,
                idx,
                tui.terminal.viewport_area.width,
            )
        });
        let hl = backtrack_helpers::highlight_range_for_nth_last_user(&self.transcript_lines, nth);
        (nth, offset, hl)
    }

    /// Apply a computed backtrack selection to the overlay and internal counter.
    fn apply_backtrack_selection(
        &mut self,
        selection: (usize, Option<usize>, Option<(usize, usize)>),
    ) {
        let (nth, offset, hl) = selection;
        self.backtrack.count = nth;
        if let Some(Overlay::Transcript(t)) = &mut self.overlay {
            if let Some(off) = offset {
                t.set_scroll_offset(off);
            }
            t.set_highlight_range(hl);
        }
    }

    /// Forward any event to the overlay and close it if done.
    fn overlay_forward_event(&mut self, tui: &mut tui::Tui, event: TuiEvent) -> Result<()> {
        if let Some(overlay) = &mut self.overlay {
            overlay.handle_event(tui, event)?;
            if overlay.is_done() {
                self.close_transcript_overlay(tui);
                tui.frame_requester().schedule_frame();
            }
        }
        Ok(())
    }

    /// Handle Enter in overlay backtrack preview: confirm selection and reset state.
    fn overlay_confirm_backtrack(&mut self, tui: &mut tui::Tui) {
        if let Some(base_id) = self.backtrack.base_id {
            let drop_last_messages = self.backtrack.count;
            let prefill =
                backtrack_helpers::nth_last_user_text(&self.transcript_lines, drop_last_messages)
                    .unwrap_or_default();
            self.close_transcript_overlay(tui);
            self.request_backtrack(prefill, base_id, drop_last_messages);
        }
        self.reset_backtrack_state();
    }

    /// Handle Esc in overlay backtrack preview: step selection if armed, else forward.
    fn overlay_step_backtrack(&mut self, tui: &mut tui::Tui, event: TuiEvent) -> Result<()> {
        if self.backtrack.base_id.is_some() {
            self.step_backtrack_and_highlight(tui);
        } else {
            self.overlay_forward_event(tui, event)?;
        }
        Ok(())
    }

    /// Confirm a primed backtrack from the main view (no overlay visible).
    /// Computes the prefill from the selected user message and requests history.
    pub(crate) fn confirm_backtrack_from_main(&mut self) {
        if let Some(base_id) = self.backtrack.base_id {
            let drop_last_messages = self.backtrack.count;
            let prefill =
                backtrack_helpers::nth_last_user_text(&self.transcript_lines, drop_last_messages)
                    .unwrap_or_default();
            self.request_backtrack(prefill, base_id, drop_last_messages);
        }
        self.reset_backtrack_state();
    }

    // ===== !Modify Start: 用户提问节点选择器 - 构建与打开 =====
    /// 打开“用户提问节点选择器”弹窗：仅列出用户消息节点，最近在上。
    /// !Modify[owner=yoyooyooo]
    /// !AI_GUIDANCE: 若上游改变用户消息头渲染/识别方式（"user" 头行），请同步更新查询逻辑；
    /// 遇到不兼容变更请先询问用户。
    pub(crate) fn open_user_nodes_picker(&mut self) {
        use crate::bottom_pane::SelectionAction;
        use crate::bottom_pane::SelectionItem;
        // 进入列表弹窗后，清除底部的 Esc 提示以免干扰。
        self.chat_widget.clear_esc_backtrack_hint();

        // 枚举从最近到更早的用户消息，构造选择项。
        let mut items: Vec<SelectionItem> = Vec::new();
        let mut n = 1usize; // 1 = 最近一次用户消息
        loop {
            if backtrack_helpers::find_nth_last_user_header_index(&self.transcript_lines, n)
                .is_none()
            {
                break;
            }
            let preview = backtrack_helpers::nth_last_user_text(&self.transcript_lines, n)
                .unwrap_or_default();
            let first_line = preview.lines().next().unwrap_or("").trim().to_string();
            let name = if first_line.is_empty() {
                format!("(空消息) [{n}]")
            } else {
                first_line
            };
            let desc = if n == 1 {
                Some("最近".to_string())
            } else {
                Some(format!("{n} 条之前"))
            };
            let drop_count = n;
            let actions: Vec<SelectionAction> = vec![Box::new(move |tx| {
                tx.send(crate::app_event::AppEvent::BacktrackTo(drop_count));
            })];

            items.push(SelectionItem {
                name,
                description: desc,
                is_current: n == 1,
                actions,
            });
            n += 1;
        }

        if items.is_empty() {
            // 没有可回退的用户消息，取消 primed 状态即可。
            self.reset_backtrack_state();
            return;
        }

        self.chat_widget.open_backtrack_picker(items);
    }

    /// 处理来自选择器的确认：直接按所选 N（从最近起算）发起回溯。
    /// !Modify[owner=yoyooyooo]: 自定义选择器的回调路径，与转录 overlay 的确认逻辑并行存在。
    /// !AI_GUIDANCE: 若上游改变回溯协议/消息格式，保持语义一致；有不兼容需先询问用户。
    pub(crate) fn confirm_backtrack_from_picker(&mut self, n: usize) {
        if n == 0 {
            return;
        }
        let Some(base_id) = self.chat_widget.session_id() else {
            return;
        };
        let prefill =
            backtrack_helpers::nth_last_user_text(&self.transcript_lines, n).unwrap_or_default();
        self.request_backtrack(prefill, base_id, n);
        self.reset_backtrack_state();
    }
    // ===== !Modify End: 用户提问节点选择器 - 构建与打开 =====

    /// Clear all backtrack-related state and composer hints.
    pub(crate) fn reset_backtrack_state(&mut self) {
        // ===== !Modify Start: 同步清理清空提示窗口与回退提示 =====
        // !Desc: 复位时同时清理：回退 prime、清空提示窗口截止时间，以及界面提示。
        // !AI_GUIDANCE: 若上游集中管理提示状态，请在统一入口执行同等清理。
        self.backtrack.primed = false;
        self.backtrack.clear_primed_until = None;
        self.backtrack.base_id = None;
        self.backtrack.count = 0;
        self.chat_widget.clear_esc_backtrack_hint();
        self.chat_widget.clear_esc_clear_hint();
        // ===== !Modify End: 同步清理清空提示窗口与回退提示 =====
    }

    /// Handle a ConversationHistory response while a backtrack is pending.
    /// If it matches the primed base session, fork and switch to the new conversation.
    pub(crate) async fn on_conversation_history_for_backtrack(
        &mut self,
        tui: &mut tui::Tui,
        ev: ConversationHistoryResponseEvent,
    ) -> Result<()> {
        if let Some((base_id, _, _)) = self.backtrack.pending.as_ref()
            && ev.conversation_id == *base_id
            && let Some((_, drop_count, prefill)) = self.backtrack.pending.take()
        {
            self.fork_and_switch_to_new_conversation(tui, ev, drop_count, prefill)
                .await;
        }
        Ok(())
    }

    /// Fork the conversation using provided history and switch UI/state accordingly.
    async fn fork_and_switch_to_new_conversation(
        &mut self,
        tui: &mut tui::Tui,
        ev: ConversationHistoryResponseEvent,
        drop_count: usize,
        prefill: String,
    ) {
        let cfg = self.chat_widget.config_ref().clone();
        // Perform the fork via a thin wrapper for clarity/testability.
        let result = self
            .perform_fork(ev.entries.clone(), drop_count, cfg.clone())
            .await;
        match result {
            Ok(new_conv) => {
                self.install_forked_conversation(tui, cfg, new_conv, drop_count, &prefill)
            }
            Err(e) => tracing::error!("error forking conversation: {e:#}"),
        }
    }

    /// Thin wrapper around ConversationManager::fork_conversation.
    async fn perform_fork(
        &self,
        entries: Vec<codex_protocol::models::ResponseItem>,
        drop_count: usize,
        cfg: codex_core::config::Config,
    ) -> codex_core::error::Result<codex_core::NewConversation> {
        self.server
            .fork_conversation(entries, drop_count, cfg)
            .await
    }

    /// Install a forked conversation into the ChatWidget and update UI to reflect selection.
    fn install_forked_conversation(
        &mut self,
        tui: &mut tui::Tui,
        cfg: codex_core::config::Config,
        new_conv: codex_core::NewConversation,
        drop_count: usize,
        prefill: &str,
    ) {
        let conv = new_conv.conversation;
        let session_configured = new_conv.session_configured;
        let init = crate::chatwidget::ChatWidgetInit {
            config: cfg,
            frame_requester: tui.frame_requester(),
            app_event_tx: self.app_event_tx.clone(),
            initial_prompt: None,
            initial_images: Vec::new(),
            enhanced_keys_supported: self.enhanced_keys_supported,
        };
        self.chat_widget =
            crate::chatwidget::ChatWidget::new_from_existing(init, conv, session_configured);
        // Trim transcript up to the selected user message and re-render it.
        self.trim_transcript_for_backtrack(drop_count);
        self.render_transcript_once(tui);
        if !prefill.is_empty() {
            self.chat_widget.insert_str(prefill);
        }
        tui.frame_requester().schedule_frame();
    }

    /// Trim transcript_lines to preserve only content up to the selected user message.
    fn trim_transcript_for_backtrack(&mut self, drop_count: usize) {
        if let Some(cut_idx) =
            backtrack_helpers::find_nth_last_user_header_index(&self.transcript_lines, drop_count)
        {
            self.transcript_lines.truncate(cut_idx);
        } else {
            self.transcript_lines.clear();
        }
    }
}
