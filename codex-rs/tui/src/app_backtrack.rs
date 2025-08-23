use crate::app::App;
use crate::backtrack_helpers;
use crate::transcript_app::TranscriptApp;
use crate::tui;
use crate::tui::TuiEvent;
use color_eyre::eyre::Result;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;

impl App {
    /// Route TUI events to the overlay when present, handling backtrack preview
    /// interactions (Esc to step target, Enter to confirm) and overlay lifecycle.
    pub(crate) async fn handle_backtrack_overlay_event(
        &mut self,
        tui: &mut tui::Tui,
        event: TuiEvent,
    ) -> Result<bool> {
        // Intercept Esc/Enter when overlay is a backtrack preview.
        let mut handled = false;
        if self.transcript_overlay_is_backtrack {
            match event {
                TuiEvent::Key(KeyEvent {
                    code: KeyCode::Esc,
                    kind: KeyEventKind::Press | KeyEventKind::Repeat,
                    ..
                }) => {
                    if self.esc_backtrack_base.is_some() {
                        self.esc_backtrack_count = self.esc_backtrack_count.saturating_add(1);
                        let nth = backtrack_helpers::normalize_backtrack_n(
                            &self.transcript_lines,
                            self.esc_backtrack_count,
                        );
                        self.esc_backtrack_count = nth;
                        let header_idx = backtrack_helpers::find_nth_last_user_header_index(
                            &self.transcript_lines,
                            nth,
                        );
                        let offset = header_idx.map(|idx| {
                            backtrack_helpers::wrapped_offset_before(
                                &self.transcript_lines,
                                idx,
                                tui.terminal.viewport_area.width,
                            )
                        });
                        let hl = backtrack_helpers::highlight_range_for_nth_last_user(
                            &self.transcript_lines,
                            nth,
                        );
                        if let Some(overlay) = &mut self.transcript_overlay {
                            if let Some(off) = offset {
                                overlay.scroll_offset = off;
                            }
                            overlay.set_highlight_range(hl);
                        }
                        tui.frame_requester().schedule_frame();
                        handled = true;
                    }
                }
                TuiEvent::Key(KeyEvent {
                    code: KeyCode::Enter,
                    kind: KeyEventKind::Press,
                    ..
                }) => {
                    // Confirm the backtrack: close overlay, request fork.
                    if let Some(base_id) = self.esc_backtrack_base {
                        let drop_last_messages = self.esc_backtrack_count;
                        // Compute prefill text now from the visible transcript.
                        let prefill = backtrack_helpers::nth_last_user_text(
                            &self.transcript_lines,
                            drop_last_messages,
                        )
                        .unwrap_or_default();
                        self.close_transcript_overlay(tui);
                        self.request_backtrack(prefill, base_id, drop_last_messages);
                    }
                    // Reset backtrack state after confirming.
                    self.esc_backtrack_primed = false;
                    self.esc_backtrack_base = None;
                    self.esc_backtrack_count = 0;
                    handled = true;
                }
                _ => {}
            }
        } else if let TuiEvent::Key(KeyEvent {
            code: KeyCode::Esc,
            kind: KeyEventKind::Press | KeyEventKind::Repeat,
            ..
        }) = event
        {
            // First Esc in transcript overlay: immediately begin backtrack preview at latest user message.
            self.esc_backtrack_primed = true;
            self.esc_backtrack_base = self.chat_widget.session_id();
            self.transcript_overlay_is_backtrack = true;
            self.esc_backtrack_count = 1;
            let nth = backtrack_helpers::normalize_backtrack_n(&self.transcript_lines, 1);
            self.esc_backtrack_count = nth;
            let header_idx =
                backtrack_helpers::find_nth_last_user_header_index(&self.transcript_lines, nth);
            let offset = header_idx.map(|idx| {
                backtrack_helpers::wrapped_offset_before(
                    &self.transcript_lines,
                    idx,
                    tui.terminal.viewport_area.width,
                )
            });
            let hl =
                backtrack_helpers::highlight_range_for_nth_last_user(&self.transcript_lines, nth);
            if let Some(overlay) = &mut self.transcript_overlay {
                if let Some(off) = offset {
                    overlay.scroll_offset = off;
                }
                overlay.set_highlight_range(hl);
            }
            tui.frame_requester().schedule_frame();
            handled = true;
        }
        // Forward to overlay if not handled
        if !handled && let Some(overlay) = &mut self.transcript_overlay {
            overlay.handle_event(tui, event)?;
            if overlay.is_done {
                self.close_transcript_overlay(tui);
            }
        }
        tui.frame_requester().schedule_frame();
        Ok(true)
    }

    /// Handle global Esc presses for backtracking when no overlay is present.
    pub(crate) fn handle_backtrack_esc_key(&mut self, tui: &mut tui::Tui) {
        // Only handle backtracking when composer is empty to avoid clobbering edits.
        if self.chat_widget.composer_is_empty() {
            if !self.esc_backtrack_primed {
                // Arm backtracking and record base conversation.
                self.esc_backtrack_primed = true;
                self.esc_backtrack_count = 0;
                self.esc_backtrack_base = self.chat_widget.session_id();
                // Show hint in composer to guide Esc-then-Esc flow.
                self.chat_widget.show_esc_backtrack_hint();
            } else if self.transcript_overlay.is_none() {
                // Open transcript overlay in backtrack preview mode and jump to the target message.
                self.open_transcript_overlay(tui);
                self.transcript_overlay_is_backtrack = true;
                // Overlay footer already shows the backtrack guidance unconditionally.
                // Composer is hidden by overlay; clear its hint.
                self.chat_widget.clear_esc_backtrack_hint();
                self.esc_backtrack_count = self.esc_backtrack_count.saturating_add(1);
                let nth = backtrack_helpers::normalize_backtrack_n(
                    &self.transcript_lines,
                    self.esc_backtrack_count,
                );
                self.esc_backtrack_count = nth;
                let header_idx =
                    backtrack_helpers::find_nth_last_user_header_index(&self.transcript_lines, nth);
                let offset = header_idx.map(|idx| {
                    backtrack_helpers::wrapped_offset_before(
                        &self.transcript_lines,
                        idx,
                        tui.terminal.viewport_area.width,
                    )
                });
                let hl = backtrack_helpers::highlight_range_for_nth_last_user(
                    &self.transcript_lines,
                    nth,
                );
                if let Some(overlay) = &mut self.transcript_overlay {
                    if let Some(off) = offset {
                        overlay.scroll_offset = off;
                    }
                    overlay.set_highlight_range(hl);
                }
            } else if self.transcript_overlay_is_backtrack {
                // Already previewing: step to the next older message.
                self.esc_backtrack_count = self.esc_backtrack_count.saturating_add(1);
                let nth = backtrack_helpers::normalize_backtrack_n(
                    &self.transcript_lines,
                    self.esc_backtrack_count,
                );
                self.esc_backtrack_count = nth;
                let header_idx =
                    backtrack_helpers::find_nth_last_user_header_index(&self.transcript_lines, nth);
                let offset = header_idx.map(|idx| {
                    backtrack_helpers::wrapped_offset_before(
                        &self.transcript_lines,
                        idx,
                        tui.terminal.viewport_area.width,
                    )
                });
                let hl = backtrack_helpers::highlight_range_for_nth_last_user(
                    &self.transcript_lines,
                    nth,
                );
                if let Some(overlay) = &mut self.transcript_overlay {
                    if let Some(off) = offset {
                        overlay.scroll_offset = off;
                    }
                    overlay.set_highlight_range(hl);
                }
            }
        }
    }

    /// Stage a backtrack and request conversation history from the agent.
    pub(crate) fn request_backtrack(
        &mut self,
        prefill: String,
        base_id: uuid::Uuid,
        drop_last_messages: usize,
    ) {
        self.pending_backtrack = Some((base_id, drop_last_messages, prefill));
        self.app_event_tx.send(crate::app_event::AppEvent::CodexOp(
            codex_core::protocol::Op::GetHistory,
        ));
    }

    /// Open transcript overlay (enters alternate screen and shows full transcript).
    pub(crate) fn open_transcript_overlay(&mut self, tui: &mut tui::Tui) {
        let _ = tui.enter_alt_screen();
        self.transcript_overlay = Some(TranscriptApp::new(self.transcript_lines.clone()));
        tui.frame_requester().schedule_frame();
    }

    /// Close transcript overlay and restore normal UI.
    pub(crate) fn close_transcript_overlay(&mut self, tui: &mut tui::Tui) {
        let _ = tui.leave_alt_screen();
        let was_backtrack = self.transcript_overlay_is_backtrack;
        if !self.deferred_history_lines.is_empty() {
            let lines = std::mem::take(&mut self.deferred_history_lines);
            tui.insert_history_lines(lines);
        }
        self.transcript_overlay = None;
        self.transcript_overlay_is_backtrack = false;
        if was_backtrack {
            // Ensure backtrack state is fully reset when overlay closes (e.g. via 'q').
            self.esc_backtrack_primed = false;
            self.esc_backtrack_base = None;
            self.esc_backtrack_count = 0;
            self.chat_widget.clear_esc_backtrack_hint();
        }
    }

    /// Re-render the full transcript into the terminal scrollback in one call.
    /// Useful when switching sessions to ensure prior history remains visible.
    pub(crate) fn render_transcript_once(&mut self, tui: &mut tui::Tui) {
        if !self.transcript_lines.is_empty() {
            tui.insert_history_lines(self.transcript_lines.clone());
        }
    }
}
