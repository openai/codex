use codex_core::config::Config;
use codex_core::protocol::Op;
use codex_core::protocol::ReviewFinding;
use codex_core::protocol::ReviewOutputEvent;
use codex_core::protocol::ReviewRequest;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line as RtLine;
use ratatui::text::Span;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;
use std::path::PathBuf;
use tokio::runtime::Handle;
use tokio::spawn;
use tracing::debug;

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::bottom_pane::BottomPane;
use crate::bottom_pane::BranchPickerView;
use crate::bottom_pane::CommitPickerView;
use crate::bottom_pane::CustomPromptView;
use crate::bottom_pane::SelectionAction;
use crate::bottom_pane::SelectionItem;
use crate::git_shortstat::DiffShortStat;
use crate::git_shortstat::get_diff_shortstat;
use codex_core::review_format;

pub(crate) struct ReviewState {
    is_review_mode: bool,
    diff_shortstat: Option<DiffShortStat>,
    diff_shortstat_inflight: Option<u64>,
    diff_shortstat_generation: u64,
}

pub(crate) struct ReviewExitUpdate {
    pub banner: String,
    pub should_flush_stream: bool,
    pub result: ReviewExitResult,
}

pub(crate) enum ReviewExitResult {
    None,
    ShowMessage(Vec<RtLine<'static>>),
    ShowFindings(Vec<ReviewFinding>),
}

impl ReviewState {
    pub(crate) fn new() -> Self {
        Self {
            is_review_mode: false,
            diff_shortstat: None,
            diff_shortstat_inflight: None,
            diff_shortstat_generation: 0,
        }
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn is_review_mode(&self) -> bool {
        self.is_review_mode
    }

    pub(crate) fn enter_review_mode(&mut self, user_facing_hint: &str) -> Option<String> {
        let banner = format!(">> Code review started: {user_facing_hint} <<");
        let should_emit_banner = !self.is_review_mode;
        self.is_review_mode = true;
        should_emit_banner.then_some(banner)
    }

    pub(crate) fn exit_review_mode(
        &mut self,
        review_output: Option<ReviewOutputEvent>,
    ) -> ReviewExitUpdate {
        self.is_review_mode = false;
        let mut result = ReviewExitResult::None;
        let mut should_flush_stream = false;
        if let Some(output) = review_output {
            should_flush_stream = true;
            if output.findings.is_empty() {
                // Use the shared formatter to keep header/structure consistent
                // with other review displays. For an empty findings list this
                // yields a simple "Review comment:" header.
                let mut body_lines: Vec<RtLine<'static>> =
                    review_format::format_review_findings_block(&[], None)
                        .lines()
                        .map(|s| RtLine::from(s.to_string()))
                        .collect();

                // Add details (overall explanation or a fallback message).
                body_lines.push(RtLine::from(""));
                if !output.overall_explanation.trim().is_empty() {
                    for line in output.overall_explanation.lines() {
                        body_lines.push(RtLine::from(line.to_string()));
                    }
                } else {
                    body_lines.push(RtLine::from("Review failed -- no response found"));
                }
                result = ReviewExitResult::ShowMessage(body_lines);
            } else {
                result = ReviewExitResult::ShowFindings(output.findings);
            }
        }

        ReviewExitUpdate {
            banner: "<< Code review finished >>".to_string(),
            should_flush_stream,
            result,
        }
    }

    pub(crate) fn on_diff_shortstat_ready(
        &mut self,
        shortstat: Option<DiffShortStat>,
        request_id: u64,
    ) -> bool {
        if let Some(expected) = self.diff_shortstat_inflight {
            if expected != request_id {
                return false;
            }
        } else if request_id != self.diff_shortstat_generation {
            return false;
        }
        self.diff_shortstat_inflight = None;
        self.diff_shortstat = shortstat;
        true
    }

    fn current_changes_styled_label(&self) -> Option<(String, Vec<Span<'static>>)> {
        let stats = self.diff_shortstat?;

        let files_changed = stats.files_changed;
        let insertions = stats.insertions;
        let deletions = stats.deletions;
        let file_label = if stats.files_changed == 1 {
            "file changed"
        } else {
            "files changed"
        };

        let summary_text = format!(
            "Review current changes - {files_changed} {file_label} (+{insertions} -{deletions})"
        );

        let mut spans: Vec<Span<'static>> = Vec::new();
        spans.push("Review current changes".into());

        let prefix = format!(" - {files_changed} {file_label} (").dim();
        spans.push(prefix);

        spans.push(format!("+{insertions}").green());
        spans.push(" ".dim());
        spans.push(format!("-{deletions}").red());
        spans.push(")".dim());

        Some((summary_text, spans))
    }

    pub(crate) fn render_hint(&self, area: Rect, bottom_pane_area: Rect, buf: &mut Buffer) {
        if !self.is_review_mode {
            return;
        }

        let hint_y = bottom_pane_area
            .y
            .saturating_add(bottom_pane_area.height)
            .saturating_sub(1);
        if hint_y < bottom_pane_area.y || hint_y >= area.y.saturating_add(area.height) {
            return;
        }

        let blank_line = " ".repeat(bottom_pane_area.width as usize);
        let render_blank_line = |buf: &mut Buffer, y: u16| {
            if y < bottom_pane_area.y || y >= area.y.saturating_add(area.height) {
                return;
            }
            Paragraph::new(RtLine::from(blank_line.as_str())).render(
                Rect {
                    x: bottom_pane_area.x,
                    y,
                    width: bottom_pane_area.width,
                    height: 1,
                },
                buf,
            );
        };

        if bottom_pane_area.width > 0 && hint_y > bottom_pane_area.y {
            render_blank_line(buf, hint_y.saturating_sub(1));
        }

        let hint_area = Rect {
            x: bottom_pane_area.x,
            y: hint_y,
            width: bottom_pane_area.width,
            height: 1,
        };
        #[allow(clippy::disallowed_methods)]
        let line = RtLine::from("✎ Review in progress (esc to cancel)".yellow());
        Paragraph::new(line).render(hint_area, buf);

        let below_y = hint_y.saturating_add(1);
        if bottom_pane_area.width > 0 && below_y < area.y.saturating_add(area.height) {
            render_blank_line(buf, below_y);
        }
    }
}

pub(crate) struct ReviewController<'a> {
    state: &'a mut ReviewState,
    config: &'a Config,
    bottom_pane: &'a mut BottomPane,
    app_event_tx: &'a AppEventSender,
}

impl<'a> ReviewController<'a> {
    pub(crate) fn new(
        state: &'a mut ReviewState,
        config: &'a Config,
        bottom_pane: &'a mut BottomPane,
        app_event_tx: &'a AppEventSender,
    ) -> Self {
        Self {
            state,
            config,
            bottom_pane,
            app_event_tx,
        }
    }

    pub(crate) fn enter_review_mode(&mut self, user_facing_hint: &str) -> Option<String> {
        self.state.enter_review_mode(user_facing_hint)
    }

    pub(crate) fn exit_review_mode(
        &mut self,
        review_output: Option<ReviewOutputEvent>,
    ) -> ReviewExitUpdate {
        self.state.exit_review_mode(review_output)
    }

    pub(crate) fn request_diff_shortstat(&mut self, force: bool) {
        if Handle::try_current().is_err() {
            return;
        }
        if !force && self.state.diff_shortstat_inflight.is_some() {
            return;
        }
        let request_id = self.state.diff_shortstat_generation.wrapping_add(1);
        self.state.diff_shortstat_generation = request_id;
        self.state.diff_shortstat_inflight = Some(request_id);
        let tx = self.app_event_tx.clone();
        let cwd = self.config.cwd.clone();
        spawn(async move {
            let shortstat = match get_diff_shortstat(&cwd).await {
                Ok(value) => value,
                Err(err) => {
                    debug!(error = ?err, "failed to compute git shortstat");
                    None
                }
            };
            tx.send(AppEvent::DiffShortstat {
                shortstat,
                request_id,
            });
        });
    }

    pub(crate) fn on_diff_shortstat_ready(
        &mut self,
        shortstat: Option<DiffShortStat>,
        request_id: u64,
    ) -> bool {
        let updated = self.state.on_diff_shortstat_ready(shortstat, request_id);
        if updated {
            self.refresh_review_preset_label();
        }
        updated
    }

    pub(crate) fn open_review_popup(&mut self) {
        self.request_diff_shortstat(true);
        let mut items: Vec<SelectionItem> = Vec::new();
        let build_review_actions =
            |review_request: ReviewRequest, context_line: String| -> Vec<SelectionAction> {
                vec![Box::new(move |pane, tx: &AppEventSender| {
                    let base_prompt = review_request.prompt.clone();
                    let hint = review_request.user_facing_hint.clone();
                    // Always show custom prompt as last step; empty input submits base prompt.
                    let title = "Custom Instructions".to_string();
                    let placeholder =
                        "Add anything else the reviewer should know (optional)".to_string();
                    let context_label = context_line.clone();
                    let on_submit = {
                        let tx = tx.clone();
                        move |custom: String| {
                            let trimmed = custom.trim().to_string();
                            let prompt = if trimmed.is_empty() {
                                base_prompt.clone()
                            } else {
                                format!("{base_prompt}\n\n{trimmed}")
                            };
                            let user_facing_hint = if trimmed.is_empty() {
                                hint.clone()
                            } else {
                                format!("{hint} ({trimmed})")
                            };
                            tx.send(AppEvent::CodexOp(Op::Review {
                                review_request: ReviewRequest {
                                    prompt,
                                    user_facing_hint,
                                },
                            }));
                        }
                    };
                    let view = CustomPromptView::new(
                        title,
                        placeholder,
                        Some(context_label),
                        true, // allow empty submit to send base prompt
                        Box::new(on_submit),
                    );
                    pane.show_view(Box::new(view));
                })]
            };

        let (name, styled_label) = self
            .state
            .current_changes_styled_label()
            .map(|(summary, spans)| (summary, Some(spans)))
            .unwrap_or_else(|| ("Review current changes".to_string(), None));
        let base_prompt = "Review the current code changes (staged, unstaged, and untracked files) and provide prioritized findings.".to_string();
        let user_facing_hint = "current changes".to_string();
        let context_line = "Review current changes".to_string();
        let actions = build_review_actions(
            ReviewRequest {
                prompt: base_prompt,
                user_facing_hint,
            },
            context_line,
        );
        items.push(SelectionItem {
            name,
            description: None,
            is_current: false,
            actions,
            styled_label,
            dismiss_on_select: true,
        });

        let commit_cwd = self.config.cwd.clone();
        items.push(SelectionItem {
            name: "Review commit".to_string(),
            description: None,
            is_current: false,
            actions: vec![Self::commit_picker_action(commit_cwd)],
            styled_label: None,
            dismiss_on_select: false,
        });

        let branch_cwd = self.config.cwd.clone();
        items.push(SelectionItem {
            name: "Review against a base branch".to_string(),
            description: None,
            is_current: false,
            actions: vec![Self::branch_picker_action(branch_cwd)],
            styled_label: None,
            dismiss_on_select: false,
        });

        items.push(SelectionItem {
            name: "Custom Instructions".to_string(),
            description: None,
            is_current: false,
            actions: vec![Self::custom_prompt_action(
                "Enter custom review instructions".to_string(),
            )],
            styled_label: None,
            dismiss_on_select: false,
        });

        self.bottom_pane
            .show_selection_view("Select a review preset".into(), None, items);
    }

    fn current_changes_label(&self) -> (String, Option<Vec<Span<'static>>>) {
        self.state
            .current_changes_styled_label()
            .map(|(summary, spans)| (summary, Some(spans)))
            .unwrap_or_else(|| ("Review current changes".to_string(), None))
    }

    fn refresh_review_preset_label(&mut self) {
        let (name, styled_label) = self.current_changes_label();
        self.bottom_pane.update_active_selection_view(|view| {
            if view.title() != "Select a review preset" {
                return false;
            }
            view.update_item(0, |item| {
                item.name = name.clone();
                item.styled_label = styled_label.clone();
            });
            true
        });
    }

    fn branch_picker_action(cwd: PathBuf) -> SelectionAction {
        Box::new(move |pane, tx| {
            Self::open_branch_picker(cwd.clone(), pane, tx);
        })
    }

    fn commit_picker_action(cwd: PathBuf) -> SelectionAction {
        Box::new(move |pane, tx| {
            Self::open_commit_picker(cwd.clone(), pane, tx);
        })
    }

    fn custom_prompt_action(title: String) -> SelectionAction {
        Box::new(move |pane, tx| {
            Self::open_custom_prompt(pane, tx, title.clone());
        })
    }

    pub(crate) fn open_branch_picker(
        cwd: PathBuf,
        bottom_pane: &mut BottomPane,
        app_event_tx: &AppEventSender,
    ) {
        let view = BranchPickerView::new(
            cwd,
            app_event_tx.clone(),
            Box::new(move |tx2, bottom_pane, branch: String| {
                let prompt = format!(
                    "Review the code changes against the base branch '{branch}'. Start by running `git diff {branch}`. Provide prioritized, actionable findings."
                );
                let context = format!("changes against '{branch}'");
                let title = "Custom Instructions".to_string();
                let placeholder =
                    "Add anything else the reviewer should know (optional)".to_string();
                let context_label = format!("Review against base branch '{branch}'");
                let tx3 = tx2.clone();
                let on_submit = move |custom: String| {
                    let trimmed = custom.trim().to_string();
                    let full_prompt = if trimmed.is_empty() {
                        prompt.clone()
                    } else {
                        format!("{prompt}\n\n{trimmed}")
                    };
                    let user_facing_hint = if trimmed.is_empty() {
                        context.clone()
                    } else {
                        format!("{context} ({trimmed})")
                    };
                    tx3.send(AppEvent::CodexOp(Op::Review {
                        review_request: ReviewRequest {
                            prompt: full_prompt,
                            user_facing_hint,
                        },
                    }));
                };
                let view = CustomPromptView::new(
                    title,
                    placeholder,
                    Some(context_label),
                    true,
                    Box::new(on_submit),
                );
                bottom_pane.show_view(Box::new(view));
            }),
        );
        bottom_pane.show_view(Box::new(view));
    }

    pub(crate) fn open_commit_picker(
        cwd: PathBuf,
        bottom_pane: &mut BottomPane,
        app_event_tx: &AppEventSender,
    ) {
        let view = CommitPickerView::new(
            cwd,
            app_event_tx.clone(),
            Box::new(move |tx, bottom_pane, selection| {
                let full_sha = selection.full_sha;
                let summary_label = if selection.summary.is_empty() {
                    selection.short_sha
                } else {
                    format!("{} {}", selection.short_sha, selection.summary)
                };
                let context = format!("commit {summary_label}");
                let prompt = format!(
                    "Review commit {summary_label} and provide prioritized, actionable findings. Start by running `git show {full_sha}`."
                );
                let title = "Custom Instructions".to_string();
                let placeholder =
                    "Add anything else the reviewer should know (optional)".to_string();
                let context_label = format!("Review commit {summary_label}");
                let tx2 = tx.clone();
                let on_submit = move |custom: String| {
                    let trimmed = custom.trim().to_string();
                    let full_prompt = if trimmed.is_empty() {
                        prompt.clone()
                    } else {
                        format!("{prompt}\n\n{trimmed}")
                    };
                    let user_facing_hint = if trimmed.is_empty() {
                        context.clone()
                    } else {
                        format!("{context} ({trimmed})")
                    };
                    tx2.send(AppEvent::CodexOp(Op::Review {
                        review_request: ReviewRequest {
                            prompt: full_prompt,
                            user_facing_hint,
                        },
                    }));
                };
                let view = CustomPromptView::new(
                    title,
                    placeholder,
                    Some(context_label),
                    true,
                    Box::new(on_submit),
                );
                bottom_pane.show_view(Box::new(view));
            }),
        );
        bottom_pane.show_view(Box::new(view));
    }

    pub(crate) fn open_custom_prompt(
        bottom_pane: &mut BottomPane,
        app_event_tx: &AppEventSender,
        title: String,
    ) {
        let tx = app_event_tx.clone();
        let view = CustomPromptView::new(
            title,
            "Type instructions and press Enter".to_string(),
            None,
            false,
            Box::new(move |prompt: String| {
                let trimmed = prompt.trim().to_string();
                if trimmed.is_empty() {
                    return;
                }
                tx.send(AppEvent::CodexOp(Op::Review {
                    review_request: ReviewRequest {
                        prompt: trimmed.clone(),
                        user_facing_hint: trimmed,
                    },
                }));
            }),
        );
        bottom_pane.show_view(Box::new(view));
    }
}
