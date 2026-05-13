//! Streaming transcript updates for `ChatWidget`.
//!
//! This module owns assistant, plan, and reasoning deltas, including stream-tail
//! cells, commit ticks, and interrupt deferral.

use super::*;

impl ChatWidget {
    pub(super) fn restore_reasoning_status_header(&mut self) {
        if let Some(header) = extract_first_bold(&self.reasoning_buffer) {
            self.status_state.terminal_title_status_kind = TerminalTitleStatusKind::Thinking;
            self.set_status_header(header);
        } else if self.bottom_pane.is_task_running() {
            self.status_state.terminal_title_status_kind = TerminalTitleStatusKind::Working;
            self.set_status_header(String::from("Working"));
        }
    }

    pub(super) fn flush_answer_stream_with_separator(&mut self) {
        let had_stream_controller = self.stream_controller.is_some();
        if let Some(mut controller) = self.stream_controller.take() {
            let scrollback_reflow = if controller.has_live_tail() {
                crate::app_event::ConsolidationScrollbackReflow::Required
            } else {
                crate::app_event::ConsolidationScrollbackReflow::IfResizeReflowRan
            };
            self.clear_active_stream_tail();
            let (cell, source) = controller.finalize();
            let deferred_history_cell =
                if scrollback_reflow == crate::app_event::ConsolidationScrollbackReflow::Required {
                    cell
                } else {
                    if let Some(cell) = cell {
                        self.add_boxed_history(cell);
                    }
                    None
                };
            // Consolidate the run of streaming AgentMessageCells into a single AgentMarkdownCell
            // that can re-render from source on resize.
            if let Some(source) = source {
                let source = parse_assistant_markdown(&source).visible_markdown;
                self.app_event_tx.send(AppEvent::ConsolidateAgentMessage {
                    source,
                    cwd: self.config.cwd.to_path_buf(),
                    scrollback_reflow,
                    deferred_history_cell,
                });
            }
        }
        self.adaptive_chunking.reset();
        if had_stream_controller && self.stream_controllers_idle() {
            self.app_event_tx.send(AppEvent::StopCommitAnimation);
        }
    }

    pub(super) fn stream_controllers_idle(&self) -> bool {
        self.stream_controller
            .as_ref()
            .map(|controller| controller.queued_lines() == 0)
            .unwrap_or(true)
            && self
                .plan_stream_controller
                .as_ref()
                .map(|controller| controller.queued_lines() == 0)
                .unwrap_or(true)
    }

    /// Restore the status indicator only after commentary completion is pending,
    /// the turn is still running, and all stream queues have drained.
    ///
    /// This gate prevents flicker while normal output is still actively
    /// streaming, but still restores a visible "working" affordance when a
    /// commentary block ends before the turn itself has completed.
    pub(super) fn maybe_restore_status_indicator_after_stream_idle(&mut self) {
        if !self.status_state.pending_status_indicator_restore
            || !self.bottom_pane.is_task_running()
            || !self.stream_controllers_idle()
        {
            return;
        }

        self.bottom_pane.ensure_status_indicator();
        self.set_status(
            self.status_state.current_status.header.clone(),
            self.status_state.current_status.details.clone(),
            StatusDetailsCapitalization::Preserve,
            self.status_state.current_status.details_max_lines,
        );
        self.status_state.pending_status_indicator_restore = false;
    }

    pub(super) fn finalize_completed_assistant_message(&mut self, message: Option<&str>) {
        // If we have a stream_controller, the finalized message payload is redundant because the
        // visible content has already been accumulated through deltas.
        if self.stream_controller.is_none()
            && let Some(message) = message
            && !message.is_empty()
        {
            self.handle_streaming_delta(message.to_string());
        }
        self.flush_answer_stream_with_separator();
        self.handle_stream_finished();
        self.request_redraw();
    }

    pub(super) fn on_agent_message_delta(&mut self, delta: String) {
        self.handle_streaming_delta(delta);
    }

    pub(super) fn on_plan_delta(&mut self, delta: String) {
        if self.active_mode_kind() != ModeKind::Plan {
            return;
        }
        if !self.transcript.plan_item_active {
            self.transcript.plan_item_active = true;
            self.transcript.plan_delta_buffer.clear();
        }
        self.transcript.plan_delta_buffer.push_str(&delta);
        if self.plan_stream_controller.is_none() {
            // Before starting a plan stream, flush any active exec cell group.
            self.flush_unified_exec_wait_streak();
            self.flush_active_cell();
            self.plan_stream_controller = Some(PlanStreamController::new(
                self.current_stream_width(/*reserved_cols*/ 4),
                &self.config.cwd,
                self.history_render_mode(),
            ));
        }
        if let Some(controller) = self.plan_stream_controller.as_mut()
            && controller.push(&delta)
        {
            self.app_event_tx.send(AppEvent::StartCommitAnimation);
            self.run_catch_up_commit_tick();
        }
        self.sync_active_stream_tail();
        self.request_redraw();
    }

    pub(super) fn on_plan_item_completed(&mut self, text: String) {
        let streamed_plan = self.transcript.plan_delta_buffer.trim().to_string();
        let plan_text = if text.trim().is_empty() {
            streamed_plan
        } else {
            text
        };
        if !plan_text.trim().is_empty() {
            self.record_agent_markdown(&plan_text);
            self.transcript.latest_proposed_plan_markdown = Some(plan_text.clone());
        }
        // Plan commit ticks can hide the status row; remember whether we streamed plan output so
        // completion can restore it once stream queues are idle.
        let should_restore_after_stream = self.plan_stream_controller.is_some();
        self.transcript.plan_delta_buffer.clear();
        self.transcript.plan_item_active = false;
        self.transcript.saw_plan_item_this_turn = true;
        let (finalized_streamed_cell, consolidated_plan_source) =
            if let Some(mut controller) = self.plan_stream_controller.take() {
                let had_live_tail = controller.has_live_tail();
                self.clear_active_stream_tail();
                let (cell, source) = controller.finalize();
                if had_live_tail {
                    (None, source)
                } else {
                    (cell, source)
                }
            } else {
                (None, None)
            };
        if let Some(cell) = finalized_streamed_cell {
            self.add_boxed_history(cell);
            // TODO: Replace streamed output with the final plan item text if plan streaming is
            // removed or if we need to reconcile mismatches between streamed and final content.
            if let Some(source) = consolidated_plan_source {
                self.app_event_tx
                    .send(AppEvent::ConsolidateProposedPlan(source));
            }
        } else if !plan_text.is_empty() {
            self.add_to_history(history_cell::new_proposed_plan(plan_text, &self.config.cwd));
        } else if let Some(source) = consolidated_plan_source {
            self.app_event_tx
                .send(AppEvent::ConsolidateProposedPlan(source));
        }
        if should_restore_after_stream {
            self.status_state.pending_status_indicator_restore = true;
            self.maybe_restore_status_indicator_after_stream_idle();
        }
    }

    pub(super) fn on_agent_reasoning_delta(&mut self, delta: String) {
        // For reasoning deltas, do not stream to history. Accumulate the
        // current reasoning block and extract the first bold element
        // (between **/**) as the chunk header. Show this header as status.
        self.reasoning_buffer.push_str(&delta);

        if self.unified_exec_wait_streak.is_some() {
            // Unified exec waiting should take precedence over reasoning-derived status headers.
            self.request_redraw();
            return;
        }

        if let Some(header) = extract_first_bold(&self.reasoning_buffer) {
            // Update the shimmer header to the extracted reasoning chunk header.
            self.status_state.terminal_title_status_kind = TerminalTitleStatusKind::Thinking;
            self.set_status_header(header);
        } else {
            // Fallback while we don't yet have a bold header: leave existing header as-is.
        }
        self.request_redraw();
    }

    pub(super) fn on_agent_reasoning_final(&mut self) {
        // At the end of a reasoning block, record transcript-only content.
        self.full_reasoning_buffer.push_str(&self.reasoning_buffer);
        if !self.full_reasoning_buffer.is_empty() {
            let cell = history_cell::new_reasoning_summary_block(
                self.full_reasoning_buffer.clone(),
                &self.config.cwd,
            );
            self.add_boxed_history(cell);
        }
        self.reasoning_buffer.clear();
        self.full_reasoning_buffer.clear();
        self.request_redraw();
    }

    pub(super) fn on_reasoning_section_break(&mut self) {
        // Start a new reasoning block for header extraction and accumulate transcript.
        self.full_reasoning_buffer.push_str(&self.reasoning_buffer);
        self.full_reasoning_buffer.push_str("\n\n");
        self.reasoning_buffer.clear();
    }

    pub(super) fn on_stream_error(&mut self, message: String, additional_details: Option<String>) {
        self.status_state.remember_retry_status_header();
        self.bottom_pane.ensure_status_indicator();
        self.status_state.terminal_title_status_kind = TerminalTitleStatusKind::Thinking;
        self.set_status(
            message,
            additional_details,
            StatusDetailsCapitalization::CapitalizeFirst,
            STATUS_DETAILS_DEFAULT_MAX_LINES,
        );
    }

    /// Handle completion of an `AgentMessage` turn item.
    ///
    /// Commentary completion sets a deferred restore flag so the status row
    /// returns once stream queues are idle. Final-answer completion (or absent
    /// phase for legacy models) clears the flag to preserve historical behavior.
    pub(super) fn on_agent_message_item_completed(
        &mut self,
        item: AgentMessageItem,
        from_replay: bool,
    ) {
        let mut message = String::new();
        for content in &item.content {
            match content {
                AgentMessageContent::Text { text } => message.push_str(text),
            }
        }
        let parsed = parse_assistant_markdown(&message);
        self.finalize_completed_assistant_message(
            (!parsed.visible_markdown.is_empty()).then_some(parsed.visible_markdown.as_str()),
        );
        if matches!(item.phase, Some(MessagePhase::FinalAnswer) | None)
            && !parsed.visible_markdown.is_empty()
        {
            self.record_agent_markdown(&parsed.visible_markdown);
        }
        if !from_replay
            && let Some(cwd) = parsed.last_created_branch_cwd()
            && let Some(thread_id) = self.thread_id
            && let Some(runner) = self.workspace_command_runner.clone()
        {
            let cwd = PathBuf::from(cwd);
            let tx = self.app_event_tx.clone();
            tokio::spawn(async move {
                if let Some(branch) =
                    crate::branch_summary::current_branch_name(runner.as_ref(), &cwd).await
                {
                    tx.send(AppEvent::SyncThreadGitBranch { thread_id, branch });
                }
            });
        }
        self.status_state.pending_status_indicator_restore = match item.phase {
            // Models that don't support preambles only output AgentMessageItems on turn completion.
            Some(MessagePhase::FinalAnswer) | None => !self.input_queue.pending_steers.is_empty(),
            Some(MessagePhase::Commentary) => true,
        };
        self.maybe_restore_status_indicator_after_stream_idle();
    }

    /// Periodic tick for stream commits. In smooth mode this preserves one-line pacing, while
    /// catch-up mode drains larger batches to reduce queue lag.
    pub(crate) fn on_commit_tick(&mut self) {
        self.run_commit_tick();
    }

    /// Runs a regular periodic commit tick.
    pub(super) fn run_commit_tick(&mut self) {
        self.run_commit_tick_with_scope(CommitTickScope::AnyMode);
    }

    /// Runs an opportunistic commit tick only if catch-up mode is active.
    pub(super) fn run_catch_up_commit_tick(&mut self) {
        self.run_commit_tick_with_scope(CommitTickScope::CatchUpOnly);
    }

    /// Runs a commit tick for the current stream queue snapshot.
    ///
    /// `scope` controls whether this call may commit in smooth mode or only when catch-up
    /// is currently active. While lines are actively streaming we hide the status row to avoid
    /// duplicate "in progress" affordances. Restoration is gated separately so we only re-show
    /// the row after commentary completion once stream queues are idle.
    pub(super) fn run_commit_tick_with_scope(&mut self, scope: CommitTickScope) {
        let now = Instant::now();
        let outcome = run_commit_tick(
            &mut self.adaptive_chunking,
            self.stream_controller.as_mut(),
            self.plan_stream_controller.as_mut(),
            scope,
            now,
        );
        for cell in outcome.cells {
            self.bottom_pane.hide_status_indicator();
            self.add_boxed_history(cell);
        }
        self.sync_active_stream_tail();

        if outcome.has_controller && outcome.all_idle {
            self.maybe_restore_status_indicator_after_stream_idle();
            self.app_event_tx.send(AppEvent::StopCommitAnimation);
        }

        if self.turn_lifecycle.agent_turn_running {
            self.refresh_runtime_metrics();
        }
    }

    pub(super) fn flush_interrupt_queue(&mut self) {
        let mut mgr = std::mem::take(&mut self.interrupts);
        mgr.flush_all(self);
        self.interrupts = mgr;
    }

    #[inline]
    pub(super) fn defer_or_handle(
        &mut self,
        push: impl FnOnce(&mut InterruptManager),
        handle: impl FnOnce(&mut Self),
    ) {
        // Preserve deterministic FIFO across queued interrupts: once anything
        // is queued due to an active write cycle, continue queueing until the
        // queue is flushed to avoid reordering (e.g., ExecEnd before ExecBegin).
        if self.stream_controller.is_some() || !self.interrupts.is_empty() {
            push(&mut self.interrupts);
        } else {
            handle(self);
        }
    }

    pub(super) fn handle_stream_finished(&mut self) {
        if self.task_complete_pending {
            self.bottom_pane.hide_status_indicator();
            self.task_complete_pending = false;
        }
        // A completed stream indicates non-exec content was just inserted.
        self.flush_interrupt_queue();
    }

    #[inline]
    pub(super) fn handle_streaming_delta(&mut self, delta: String) {
        if self.stream_controller.is_none() {
            // Before starting an agent stream, flush any active exec cell group.
            self.flush_unified_exec_wait_streak();
            self.flush_active_cell();
            // If the previous turn inserted non-stream history (exec output, patch status, MCP
            // calls), render a separator before starting the next streamed assistant message.
            if self.transcript.needs_final_message_separator && self.transcript.had_work_activity {
                self.add_to_history(history_cell::FinalMessageSeparator::new(
                    /*elapsed_seconds*/ None, /*runtime_metrics*/ None,
                ));
                self.transcript.needs_final_message_separator = false;
            } else if self.transcript.needs_final_message_separator {
                // Reset the flag even if we don't show separator (no work was done)
                self.transcript.needs_final_message_separator = false;
            }
            self.stream_controller = Some(StreamController::new(
                self.current_stream_width(/*reserved_cols*/ 2),
                &self.config.cwd,
                self.history_render_mode(),
            ));
        }
        if let Some(controller) = self.stream_controller.as_mut()
            && controller.push(&delta)
        {
            self.app_event_tx.send(AppEvent::StartCommitAnimation);
            self.run_catch_up_commit_tick();
        }
        self.sync_active_stream_tail();
        self.request_redraw();
    }

    pub(super) fn active_cell_is_stream_tail(&self) -> bool {
        self.transcript.active_cell.as_ref().is_some_and(|cell| {
            cell.as_any().is::<history_cell::StreamingAgentTailCell>()
                || cell.as_any().is::<history_cell::StreamingPlanTailCell>()
        })
    }

    pub(super) fn has_active_stream_tail(&self) -> bool {
        (self.stream_controller.is_some() || self.plan_stream_controller.is_some())
            && self.active_cell_is_stream_tail()
    }

    pub(super) fn sync_active_stream_tail(&mut self) {
        if let Some(controller) = self.stream_controller.as_ref() {
            let tail_lines = controller.current_tail_lines();
            if tail_lines.is_empty() {
                self.clear_active_stream_tail();
                return;
            }

            self.bottom_pane.hide_status_indicator();
            self.transcript.active_cell =
                Some(Box::new(history_cell::StreamingAgentTailCell::new(
                    tail_lines,
                    controller.tail_starts_stream(),
                )));
            self.bump_active_cell_revision();
            return;
        }

        if let Some(controller) = self.plan_stream_controller.as_ref() {
            let tail_lines = controller.current_tail_display_lines();
            if tail_lines.is_empty() {
                self.clear_active_stream_tail();
                return;
            }

            self.bottom_pane.hide_status_indicator();
            self.transcript.active_cell = Some(Box::new(history_cell::StreamingPlanTailCell::new(
                tail_lines,
                !controller.tail_starts_stream(),
            )));
            self.bump_active_cell_revision();
            return;
        }

        self.clear_active_stream_tail();
    }

    pub(super) fn clear_active_stream_tail(&mut self) {
        if self.active_cell_is_stream_tail() {
            self.transcript.active_cell = None;
            self.bump_active_cell_revision();
        }
    }
}
