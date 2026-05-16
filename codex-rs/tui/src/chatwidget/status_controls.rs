//! Status output and setup controls for `ChatWidget`.
//!
//! Rendering details live in `status_surfaces`; this module owns the mutable
//! widget entrypoints that apply status state, open setup views, and update the
//! history-facing `/status` surface.

use super::*;

impl ChatWidget {
    /// Update the status indicator header and details.
    ///
    /// Passing `None` clears any existing details.
    pub(super) fn set_status(
        &mut self,
        header: String,
        details: Option<String>,
        details_capitalization: StatusDetailsCapitalization,
        details_max_lines: usize,
    ) {
        let details = details
            .filter(|details| !details.is_empty())
            .map(|details| {
                let trimmed = details.trim_start();
                match details_capitalization {
                    StatusDetailsCapitalization::CapitalizeFirst => {
                        crate::text_formatting::capitalize_first(trimmed)
                    }
                    StatusDetailsCapitalization::Preserve => trimmed.to_string(),
                }
            });
        self.status_state.set_status(StatusIndicatorState {
            header: header.clone(),
            details: details.clone(),
            details_max_lines,
        });
        self.bottom_pane.update_status(
            header,
            details,
            StatusDetailsCapitalization::Preserve,
            details_max_lines,
        );
        let title_uses_status = self
            .config
            .tui_terminal_title
            .as_ref()
            .is_some_and(|items| {
                items
                    .iter()
                    .any(|item| item == "run-state" || item == "status")
            });
        if title_uses_status {
            self.refresh_status_surfaces();
        }
    }

    /// Convenience wrapper around [`Self::set_status`];
    /// updates the status indicator header and clears any existing details.
    pub(super) fn set_status_header(&mut self, header: String) {
        self.set_status(
            header,
            /*details*/ None,
            StatusDetailsCapitalization::CapitalizeFirst,
            STATUS_DETAILS_DEFAULT_MAX_LINES,
        );
    }

    /// Sets the currently rendered footer status-line value.
    pub(crate) fn set_status_line(&mut self, status_line: Option<Line<'static>>) {
        self.bottom_pane.set_status_line(status_line);
    }

    /// Sets the terminal hyperlink target for the currently rendered footer status line.
    pub(crate) fn set_status_line_hyperlink(&mut self, url: Option<String>) {
        self.bottom_pane.set_status_line_hyperlink(url);
    }

    /// Forwards the contextual active-agent label into the bottom-pane footer pipeline.
    ///
    /// `ChatWidget` stays a pass-through here so `App` remains the owner of "which thread is the
    /// user actually looking at?" and the footer stack remains a pure renderer of that decision.
    pub(crate) fn set_active_agent_label(&mut self, active_agent_label: Option<String>) {
        self.bottom_pane.set_active_agent_label(active_agent_label);
    }

    /// Recomputes footer status-line content from config and current runtime state.
    ///
    /// This method is the status-line orchestrator: it parses configured item identifiers,
    /// warns once per session about invalid items, updates whether status-line mode is enabled,
    /// schedules async git-branch lookup when needed, and renders only values that are currently
    /// available.
    ///
    /// The omission behavior is intentional. If selected items are unavailable (for example before
    /// a session id exists or before branch lookup completes), those items are skipped without
    /// placeholders so the line remains compact and stable.
    pub(crate) fn refresh_status_line(&mut self) {
        self.refresh_status_surfaces();
    }

    /// Records that status-line setup was canceled.
    ///
    /// Cancellation is intentionally side-effect free for config state; the existing configuration
    /// remains active and no persistence is attempted.
    pub(crate) fn cancel_status_line_setup(&self) {
        tracing::info!("Status line setup canceled by user");
    }

    /// Applies status-line item selection from the setup view to in-memory config.
    ///
    /// An empty selection persists as an explicit empty list.
    pub(crate) fn setup_status_line(&mut self, items: Vec<StatusLineItem>, use_theme_colors: bool) {
        tracing::info!(
            "status line setup confirmed with items: {items:#?}, use_theme_colors: {use_theme_colors}"
        );
        let ids = items.iter().map(ToString::to_string).collect::<Vec<_>>();
        self.config.tui_status_line = Some(ids);
        self.config.tui_status_line_use_colors = use_theme_colors;
        self.refresh_status_line();
    }

    /// Applies a temporary terminal-title selection while the setup UI is open.
    pub(crate) fn preview_terminal_title(&mut self, items: Vec<TerminalTitleItem>) {
        if self.terminal_title_setup_original_items.is_none() {
            self.terminal_title_setup_original_items = Some(self.config.tui_terminal_title.clone());
        }

        let ids = items.iter().map(ToString::to_string).collect::<Vec<_>>();
        self.config.tui_terminal_title = Some(ids);
        self.refresh_terminal_title();
    }

    /// Restores the terminal-title config that was active before the setup UI
    /// opened, undoing any preview changes. No-op if no setup session is active.
    pub(crate) fn revert_terminal_title_setup_preview(&mut self) {
        let Some(original_items) = self.terminal_title_setup_original_items.take() else {
            return;
        };

        self.config.tui_terminal_title = original_items;
        self.refresh_terminal_title();
    }

    /// Dismisses the terminal-title setup UI and reverts to the pre-setup config.
    pub(crate) fn cancel_terminal_title_setup(&mut self) {
        tracing::info!("Terminal title setup canceled by user");
        self.revert_terminal_title_setup_preview();
    }

    /// Commits a confirmed terminal-title selection, ending the setup session.
    ///
    /// After this call, `revert_terminal_title_setup_preview` becomes a no-op
    /// because the original config snapshot is discarded.
    pub(crate) fn setup_terminal_title(&mut self, items: Vec<TerminalTitleItem>) {
        tracing::info!("terminal title setup confirmed with items: {items:#?}");
        let ids = items.iter().map(ToString::to_string).collect::<Vec<_>>();
        self.terminal_title_setup_original_items = None;
        self.config.tui_terminal_title = Some(ids);
        self.refresh_terminal_title();
    }

    /// Stores async git-branch lookup results for the current status-line cwd.
    ///
    /// Results are dropped when they target an out-of-date cwd to avoid rendering stale branch
    /// names after directory changes.
    pub(crate) fn set_status_line_branch(&mut self, cwd: PathBuf, branch: Option<String>) {
        if self.status_line_branch_cwd.as_ref() != Some(&cwd) {
            self.status_line_branch_pending = false;
            return;
        }
        self.status_line_branch = branch;
        self.status_line_branch_pending = false;
        self.status_line_branch_lookup_complete = true;
        self.refresh_status_surfaces();
    }

    /// Stores async Git summary lookup results for the current status-line cwd.
    pub(crate) fn set_status_line_git_summary(
        &mut self,
        cwd: PathBuf,
        summary: StatusLineGitSummary,
    ) {
        if self.status_line_git_summary_cwd.as_ref() != Some(&cwd) {
            self.status_line_git_summary_pending = false;
            return;
        }
        self.status_line_git_summary = Some(summary);
        self.status_line_git_summary_pending = false;
        self.status_line_git_summary_lookup_complete = true;
        self.refresh_status_surfaces();
    }

    pub(crate) fn add_status_output(
        &mut self,
        refreshing_rate_limits: bool,
        request_id: Option<u64>,
    ) {
        let default_usage = TokenUsage::default();
        let token_info = self.token_info.as_ref();
        let total_usage = token_info
            .map(|ti| &ti.total_token_usage)
            .unwrap_or(&default_usage);
        let collaboration_mode = self.collaboration_mode_label();
        let model = self.current_model().to_string();
        let model_default_reasoning_effort =
            self.model_catalog
                .try_list_models()
                .ok()
                .and_then(|models| {
                    models
                        .into_iter()
                        .find(|preset| preset.model == model)
                        .map(|preset| preset.default_reasoning_effort)
                });
        let reasoning_effort_override = Some(
            self.effective_reasoning_effort()
                .or(self.config.model_reasoning_effort)
                .or(model_default_reasoning_effort),
        );
        let rate_limit_snapshots: Vec<RateLimitSnapshotDisplay> = self
            .rate_limit_snapshots_by_limit_id
            .values()
            .cloned()
            .collect();
        let agents_summary =
            crate::status::compose_agents_summary(&self.config, &self.instruction_source_paths);
        let (cell, handle) = crate::status::new_status_output_with_rate_limits_handle(
            &self.config,
            self.runtime_model_provider_base_url.as_deref(),
            self.status_account_display.as_ref(),
            token_info,
            total_usage,
            &self.thread_id,
            self.thread_name.clone(),
            self.forked_from,
            rate_limit_snapshots.as_slice(),
            self.plan_type,
            Local::now(),
            self.model_display_name(),
            collaboration_mode,
            reasoning_effort_override,
            agents_summary,
            refreshing_rate_limits,
        );
        if let Some(request_id) = request_id {
            self.refreshing_status_outputs.push((request_id, handle));
        }
        self.add_to_history(cell);
    }

    pub(crate) fn finish_status_rate_limit_refresh(&mut self, request_id: u64) {
        if self.refreshing_status_outputs.is_empty() {
            return;
        }

        let rate_limit_snapshots: Vec<RateLimitSnapshotDisplay> = self
            .rate_limit_snapshots_by_limit_id
            .values()
            .cloned()
            .collect();
        let now = Local::now();
        let mut remaining = Vec::with_capacity(self.refreshing_status_outputs.len());
        let mut updated_any = false;
        for (pending_request_id, handle) in self.refreshing_status_outputs.drain(..) {
            if pending_request_id == request_id {
                updated_any = true;
                handle.finish_rate_limit_refresh(rate_limit_snapshots.as_slice(), now);
            } else {
                remaining.push((pending_request_id, handle));
            }
        }
        self.refreshing_status_outputs = remaining;
        if updated_any {
            self.request_redraw();
        }
    }

    pub(super) fn open_status_line_setup(&mut self) {
        let configured_status_line_items = self.configured_status_line_items();
        let view = StatusLineSetupView::new(
            Some(configured_status_line_items.as_slice()),
            self.config.tui_status_line_use_colors,
            self.status_surface_preview_data(),
            self.app_event_tx.clone(),
            self.bottom_pane.list_keymap(),
        );
        self.bottom_pane.show_view(Box::new(view));
    }

    pub(super) fn open_terminal_title_setup(&mut self) {
        let configured_terminal_title_items = self.configured_terminal_title_items();
        self.terminal_title_setup_original_items = Some(self.config.tui_terminal_title.clone());
        let view = TerminalTitleSetupView::new(
            Some(configured_terminal_title_items.as_slice()),
            self.terminal_title_preview_data(),
            self.app_event_tx.clone(),
            self.bottom_pane.list_keymap(),
        );
        self.bottom_pane.show_view(Box::new(view));
    }

    pub(super) fn status_surface_preview_data(&mut self) -> StatusSurfacePreviewData {
        StatusSurfacePreviewData::from_iter(StatusSurfacePreviewItem::iter().filter_map(|item| {
            self.status_surface_preview_value_for_item(item)
                .map(|value| (item, value))
        }))
    }

    pub(super) fn terminal_title_preview_data(&mut self) -> StatusSurfacePreviewData {
        let mut preview_data = self.status_surface_preview_data();
        let now = Instant::now();
        for item in TerminalTitleItem::iter() {
            let Some(preview_item) = item.preview_item() else {
                continue;
            };
            let Some(value) = self.terminal_title_value_for_item(item, now) else {
                continue;
            };
            preview_data.set_live(preview_item, value);
        }
        preview_data
    }

    pub(super) fn status_line_context_window_size(&self) -> Option<i64> {
        self.token_info
            .as_ref()
            .and_then(|info| info.model_context_window)
            .or(self.config.model_context_window)
    }

    pub(super) fn status_line_context_remaining_percent(&self) -> Option<i64> {
        let Some(context_window) = self.status_line_context_window_size() else {
            return Some(100);
        };
        let default_usage = TokenUsage::default();
        let usage = self
            .token_info
            .as_ref()
            .map(|info| &info.last_token_usage)
            .unwrap_or(&default_usage);
        Some(
            usage
                .percent_of_context_window_remaining(context_window)
                .clamp(0, 100),
        )
    }

    pub(super) fn status_line_context_used_percent(&self) -> Option<i64> {
        let remaining = self.status_line_context_remaining_percent().unwrap_or(100);
        Some((100 - remaining).clamp(0, 100))
    }

    pub(super) fn status_line_total_usage(&self) -> TokenUsage {
        self.token_info
            .as_ref()
            .map(|info| info.total_token_usage.clone())
            .unwrap_or_default()
    }

    pub(super) fn status_line_limit_display(
        &self,
        window: Option<&RateLimitWindowDisplay>,
        label: &str,
    ) -> Option<String> {
        let window = window?;
        let remaining = (100.0f64 - window.used_percent).clamp(0.0f64, 100.0f64);
        Some(format!("{label} {remaining:.0}%"))
    }

    pub(super) fn status_line_reasoning_effort_label(
        effort: Option<ReasoningEffortConfig>,
    ) -> &'static str {
        match effort {
            Some(ReasoningEffortConfig::Minimal) => "minimal",
            Some(ReasoningEffortConfig::Low) => "low",
            Some(ReasoningEffortConfig::Medium) => "medium",
            Some(ReasoningEffortConfig::High) => "high",
            Some(ReasoningEffortConfig::XHigh) => "xhigh",
            None | Some(ReasoningEffortConfig::None) => "default",
        }
    }
}
