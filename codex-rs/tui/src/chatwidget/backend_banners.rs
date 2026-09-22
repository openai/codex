//! Account banners and response-driven model transitions for the selected task.
//!
//! Only explicitly dismissible, previously shown occurrences can be dismissed. Model changes hide
//! content without dismissing it; an authoritative replacement/absence starts a new occurrence.
//! Reserve recovery uses a focused picker and a persisted task-local return model.
//! Its entry notice is shared across chats until the backend confirms ordinary usage has recovered.

use super::ChatWidget;
use super::QueuedUserMessage;
use super::luna_reserve_return::ReserveReturnModel;
use crate::app_command::AppCommand;
use crate::backend_banners::BackendBanner;
use crate::backend_banners::BannerPresentation;
use crate::backend_banners::LUNA_RESERVE_BANNER;
use crate::backend_banners::LUNA_RESERVE_RECOVERY_VIEW_ID;
use crate::bottom_pane::SelectionItem;
use crate::bottom_pane::SelectionViewParams;
use crate::bottom_pane::popup_consts::accept_cancel_hint_line;
use crate::keymap::ListAction;
use crate::model_catalog::LUNA_RESERVE_MODEL;
use codex_app_server_protocol::GetAccountRateLimitsResponse;
use codex_protocol::config_types::CollaborationMode;
use codex_protocol::config_types::ModeKind;
use codex_protocol::openai_models::ModelPreset;
use codex_protocol::openai_models::ReasoningEffort;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

#[derive(Debug, PartialEq)]
pub(crate) enum AutomaticModelSwitchReason {
    UsageLimit,
    UsageRecovered,
}

#[derive(Debug, PartialEq)]
pub(crate) struct AutomaticModelSwitch {
    pub model: ModelPreset,
    pub effort: Option<ReasoningEffort>,
    pub reason: AutomaticModelSwitchReason,
}

/// Live transition state; the return target is also persisted per task for reconstruction.
#[derive(Default)]
pub(super) struct AutomaticModelSwitchState {
    reserve_return: Option<ReserveReturnModel>,
    replaced_model: Option<String>,
}

#[derive(Default)]
pub(super) struct BackendBannerState {
    account_id: Option<String>,
    ordinary_usage_recovered: bool,
    banner: Option<BackendBanner>,
    presented: Option<BackendBanner>,
    shown: bool,
    dismissed: bool,
    picker_dismissed: Arc<AtomicBool>,
}

impl ChatWidget {
    pub(super) fn restrict_model_picker_to_luna_reserve(&self) -> bool {
        // A fresh account read can allow manual recovery even without a valid saved return model.
        self.current_model() == LUNA_RESERVE_MODEL
            && !self.backend_banner_state.ordinary_usage_recovered
    }

    pub(crate) fn invalidate_ordinary_usage_recovery(&mut self) {
        // Keep the current banner, but never reuse permission from before a new hard stop.
        self.backend_banner_state.ordinary_usage_recovered = false;
    }

    pub(super) fn waiting_for_luna_reserve(&self) -> bool {
        self.current_model() != LUNA_RESERVE_MODEL
            && self
                .backend_banner_state
                .banner
                .as_ref()
                .is_some_and(|banner| banner.banner_type == LUNA_RESERVE_BANNER)
    }

    pub(crate) fn backend_banner_fallback(&mut self) -> Option<AutomaticModelSwitch> {
        if !self.has_chatgpt_account || !self.requires_openai_auth {
            return None;
        }
        if self.current_model() == LUNA_RESERVE_MODEL
            && self.automatic_model_switch_state.reserve_return.is_none()
        {
            self.automatic_model_switch_state.reserve_return =
                self.thread_id().and_then(|thread_id| {
                    ReserveReturnModel::load(self.config.codex_home.as_path(), thread_id).or_else(
                        || {
                            // Forks inherit Reserve settings, but need their own return target:
                            // the parent's recovery will delete the parent's saved target.
                            let previous = ReserveReturnModel::load(
                                self.config.codex_home.as_path(),
                                self.forked_from?,
                            )?;
                            if self.backend_banner_state.account_id.as_deref()
                                != Some(previous.account_id.as_str())
                            {
                                return None;
                            }
                            previous
                                .save(self.config.codex_home.as_path(), thread_id)
                                .ok()?;
                            Some(previous)
                        },
                    )
                });
        }
        if self
            .automatic_model_switch_state
            .reserve_return
            .as_ref()
            .is_some_and(|previous| {
                self.backend_banner_state
                    .account_id
                    .as_ref()
                    .is_some_and(|account_id| account_id != &previous.account_id)
            })
        {
            self.clear_reserve_return();
        }
        let models = self.model_catalog.try_list_models().ok()?;
        if self.current_model() == LUNA_RESERVE_MODEL
            && self.backend_banner_state.ordinary_usage_recovered
        {
            let previous = self.automatic_model_switch_state.reserve_return.as_ref()?;
            if self.backend_banner_state.account_id.as_deref() != Some(previous.account_id.as_str())
            {
                return None;
            }
            let model = models
                .into_iter()
                .find(|model| model.show_in_picker && model.model == previous.model)?;
            return Some(AutomaticModelSwitch {
                model,
                effort: previous.effort.clone(),
                reason: AutomaticModelSwitchReason::UsageRecovered,
            });
        }
        let banner = self.backend_banner_state.banner.as_ref()?;
        // The backend emits this banner only after ordinary usage is exhausted and Reserve
        // is available. Reserve is deliberately hidden from manual model selection.
        let model = if banner.banner_type == LUNA_RESERVE_BANNER {
            models.into_iter().find(|model| {
                model.model == LUNA_RESERVE_MODEL
                    && model.model != self.current_model()
                    && banner
                        .blocked_model_slug
                        .as_deref()
                        .is_none_or(|blocked| blocked == self.current_model())
            })
        } else {
            if banner.blocked_model_slug.as_deref() != Some(self.current_model()) {
                return None;
            }
            banner.fallback_model_slugs.iter().find_map(|candidate| {
                models
                    .iter()
                    .find(|model| {
                        model.show_in_picker
                            && model.model == *candidate
                            && model.model != self.current_model()
                    })
                    .cloned()
            })
        }?;
        Some(AutomaticModelSwitch {
            model,
            effort: self.current_reasoning_effort(),
            reason: AutomaticModelSwitchReason::UsageLimit,
        })
    }

    /// Save the return target before changing server state, including before any turn is sent.
    pub(crate) fn prepare_luna_reserve_return(&mut self) -> bool {
        let Some((account_id, thread_id)) = self
            .backend_banner_state
            .account_id
            .clone()
            .zip(self.thread_id())
        else {
            return false;
        };
        let previous = ReserveReturnModel {
            account_id,
            model: self.current_model().to_string(),
            effort: self.current_reasoning_effort(),
        };
        previous
            .save(self.config.codex_home.as_path(), thread_id)
            .is_ok()
    }

    pub(super) fn clear_reserve_return(&mut self) {
        if let Some(thread_id) = self.thread_id() {
            ReserveReturnModel::clear(self.config.codex_home.as_path(), thread_id);
        }
        self.automatic_model_switch_state = AutomaticModelSwitchState::default();
    }

    pub(crate) fn show_unavailable_reserve_recovery(&mut self) {
        self.clear_reserve_return();
        // A failed switch still needs recovery actions, even if another chat used Reserve.
        self.backend_banner_state.dismissed = false;
        self.backend_banner_state.picker_dismissed = Arc::default();
        self.backend_banner_state.presented = None;
        self.refresh_backend_banner_visibility();
    }

    /// A composer submission can already be in the app queue when a Reserve switch fails.
    /// Reuse the retained prompt/steer rather than sending it on the blocked model.
    pub(crate) fn defer_pending_turn_for_luna_reserve(&mut self) -> bool {
        if !self.waiting_for_luna_reserve() {
            return false;
        }
        if self.input_queue.user_turn_pending_start
            && let Some(prompt) = self.safety_buffering_prompt.take()
        {
            self.finalize_turn();
            self.input_queue
                .queued_user_messages
                .push_front(QueuedUserMessage {
                    source: self.safety_buffering_source,
                    ..QueuedUserMessage::from(prompt)
                });
            self.input_queue
                .queued_user_message_history_records
                .push_front(super::UserMessageHistoryRecord::UserMessageText);
            self.refresh_pending_input_preview();
            return true;
        }
        !self.input_queue.pending_steers.is_empty() && self.enqueue_rejected_steer()
    }

    /// Commit a successfully applied task setting and keep its recovery notice visible together.
    pub(crate) fn finish_backend_banner_fallback(&mut self, mode: CollaborationMode) {
        let previous_model = self.current_model().to_string();
        let reserve_return = (mode.model() == LUNA_RESERVE_MODEL)
            .then(|| self.backend_banner_state.account_id.clone())
            .flatten()
            .map(|account_id| ReserveReturnModel {
                account_id,
                model: previous_model.clone(),
                effort: self.current_reasoning_effort(),
            });
        let involves_reserve =
            mode.model() == LUNA_RESERVE_MODEL || previous_model == LUNA_RESERVE_MODEL;
        self.set_model(mode.model());
        self.set_reasoning_effort(mode.reasoning_effort());
        if mode.mode == ModeKind::Plan {
            self.set_plan_mode_reasoning_effort(mode.reasoning_effort());
        }
        self.set_effective_collaboration_mode(mode);
        self.automatic_model_switch_state = AutomaticModelSwitchState {
            reserve_return,
            replaced_model: involves_reserve.then_some(previous_model),
        };
        // Showing the post-switch notice is distinct from having seen the original blocked state.
        self.backend_banner_state.shown = false;
        self.backend_banner_state.dismissed = self.reserve_notice_already_shown();
        self.backend_banner_state.picker_dismissed = Arc::default();
        self.bottom_pane
            .dismiss_view_by_id(LUNA_RESERVE_RECOVERY_VIEW_ID);
        self.backend_banner_state.presented = None;
        self.backend_banner_notice_model = Some(self.current_model().to_string());
        self.refresh_backend_banner_visibility();
    }

    /// A queued command may have been composed before the account read switched this task.
    pub(crate) fn apply_reserve_fallback_to_pending_turn(&self, op: &mut AppCommand) {
        let AppCommand::UserTurn {
            model,
            effort,
            service_tier,
            collaboration_mode,
            ..
        } = op
        else {
            return;
        };
        if self.automatic_model_switch_state.replaced_model.as_deref() != Some(model.as_str()) {
            return;
        }
        *model = self.current_model().to_string();
        *effort = self.current_reasoning_effort();
        *service_tier = self.service_tier_update_for_core();
        if let Some(mode) = collaboration_mode {
            mode.settings.model = model.clone();
            mode.settings.reasoning_effort = effort.clone();
        }
    }

    pub(crate) fn inherit_backend_banner_state(&mut self, previous: &mut ChatWidget) {
        previous.observe_backend_banner_view();
        self.backend_banner_state = std::mem::take(&mut previous.backend_banner_state);
        self.luna_reserve_notice_account_id = previous.luna_reserve_notice_account_id.take();
        self.backend_banner_state.dismissed |= self.reserve_notice_already_shown();
        // Usage belongs to the account, so switching tasks must not reset the polling cadence
        // or discard the already-known limits shown by /status.
        self.rate_limit_snapshots_by_limit_id =
            std::mem::take(&mut previous.rate_limit_snapshots_by_limit_id);
        self.usage_notice_state = std::mem::take(&mut previous.usage_notice_state);
        self.codex_rate_limit_reached_type = previous.codex_rate_limit_reached_type;
        self.codex_spend_control_reached = previous.codex_spend_control_reached;
        self.backend_banner_state.presented = None;
        self.refresh_backend_banner_visibility();
    }

    pub(crate) fn update_backend_banner(&mut self, response: &GetAccountRateLimitsResponse) {
        self.observe_backend_banner_view();
        self.backend_banner_state.account_id = response.account_id.clone();
        // Only a full, identity-validated backend read can authorize recovery. Unknown banners
        // still block it; percentages, sparse notifications and reset timestamps cannot prove it.
        let has_usable_credits = response
            .rate_limits
            .credits
            .as_ref()
            .is_some_and(|credits| credits.unlimited || credits.has_credits);
        self.backend_banner_state.ordinary_usage_recovered =
            response.ordinary_usage_allowed.is_some()
                && (response.ordinary_usage_allowed == Some(true) || has_usable_credits)
                && response.rate_limit_upsell.is_none()
                && response.rate_limits.spend_control_reached != Some(true)
                && response.rate_limits.rate_limit_reached_type.is_none();
        if self.backend_banner_state.ordinary_usage_recovered {
            self.luna_reserve_notice_account_id = None;
        }
        let banner = response
            .rate_limit_upsell
            .as_ref()
            .and_then(BackendBanner::parse)
            .map(|mut banner| {
                banner.account_id = response.account_id.clone().unwrap_or_default();
                banner.plan_type = response.rate_limits.plan_type;
                banner
            });
        let same_occurrence = self
            .backend_banner_state
            .banner
            .as_ref()
            .zip(banner.as_ref())
            .is_some_and(|(old, new)| {
                old.account_id == new.account_id
                    && old.banner_type == new.banner_type
                    && old.reset_at == new.reset_at
                    && old.presentation == new.presentation
                    && old.blocked_model_slug.as_ref().or(old.model_slug.as_ref())
                        == new.blocked_model_slug.as_ref().or(new.model_slug.as_ref())
                    && old.fallback_model_slugs == new.fallback_model_slugs
            });
        self.backend_banner_state.banner = banner;
        if !same_occurrence {
            self.backend_banner_state.shown = false;
            self.backend_banner_state.dismissed = self.reserve_notice_already_shown();
            self.backend_banner_state.picker_dismissed = Arc::default();
            self.backend_banner_notice_model = None;
        }
        if self.waiting_for_luna_reserve() {
            self.hold_rate_limit_recovery();
        }
        self.refresh_backend_banner_visibility();
    }

    fn reserve_notice_already_shown(&self) -> bool {
        self.backend_banner_state
            .banner
            .as_ref()
            .is_some_and(|banner| {
                banner.banner_type == LUNA_RESERVE_BANNER
                    && self.luna_reserve_notice_account_id.as_deref()
                        == Some(banner.account_id.as_str())
            })
    }

    fn observe_backend_banner_view(&mut self) {
        let (shown, dismissed) = self.bottom_pane.inline_banner_lifecycle();
        self.backend_banner_state.shown |= shown;
        self.backend_banner_state.dismissed |= dismissed;
        self.backend_banner_state.dismissed |= self
            .backend_banner_state
            .picker_dismissed
            .load(Ordering::Relaxed);
    }

    pub(super) fn refresh_backend_banner_visibility(&mut self) {
        let banner = self.backend_banner_state.banner.as_ref().filter(|banner| {
            // Keep recovery actions available while switching, including on older servers
            // without settings/update. The copy below describes the accepted model only.
            if banner.banner_type == LUNA_RESERVE_BANNER {
                return !self.backend_banner_state.dismissed;
            }
            // Explicit fallback payloads describe the selected replacement, not a pending switch.
            let matches_selected_model = match banner.blocked_model_slug.as_deref() {
                Some(blocked) if !banner.fallback_model_slugs.is_empty() => {
                    blocked != self.current_model()
                        && banner
                            .fallback_model_slugs
                            .iter()
                            .any(|model| model == self.current_model())
                }
                Some(_) | None => {
                    banner
                        .model_slug
                        .as_deref()
                        .is_none_or(|model| model == self.current_model())
                        || self.backend_banner_notice_model.as_deref() == Some(self.current_model())
                }
            };
            !self.backend_banner_state.dismissed && matches_selected_model
        });
        if banner == self.backend_banner_state.presented.as_ref() {
            return;
        }
        let is_reserve = banner.is_some_and(|banner| banner.banner_type == LUNA_RESERVE_BANNER);
        let content = banner.map(|banner| {
            let mut content = banner.actionable_banner(self.clock_format);
            if banner.banner_type == LUNA_RESERVE_BANNER
                && self.current_model() != LUNA_RESERVE_MODEL
            {
                content.title = "Usage limit reached".to_string();
                content.description =
                    "Your included usage is exhausted. Choose an option below to continue."
                        .to_string();
            }
            content
        });
        self.backend_banner_state.presented = banner.cloned();
        if content.is_some() {
            self.bottom_pane
                .dismiss_view_by_id(super::rate_limits::WORKSPACE_NUDGE_VIEW_ID);
            if self
                .bottom_pane
                .dismiss_view_by_id(super::rate_limits::RATE_LIMIT_SWITCH_PROMPT_VIEW_ID)
            {
                // Replacing the view is not the user's decision to dismiss the prompt.
                self.rate_limit_switch_prompt = super::RateLimitSwitchPromptState::Pending;
            }
        }
        self.bottom_pane
            .dismiss_view_by_id(LUNA_RESERVE_RECOVERY_VIEW_ID);
        match (is_reserve, content) {
            (true, Some(content)) => {
                self.bottom_pane.set_inline_banner(/*banner*/ None);
                let mut params: SelectionViewParams = content.into();
                params.view_id = Some(LUNA_RESERVE_RECOVERY_VIEW_ID);
                if self.current_model() == LUNA_RESERVE_MODEL {
                    self.luna_reserve_notice_account_id =
                        self.backend_banner_state.account_id.clone();
                    // Continuing is a local dismissal, separate from the backend's purchase CTAs.
                    params.items.push(SelectionItem {
                        name: "Continue with Luna Reserve".to_string(),
                        ..Default::default()
                    });
                    let list_keymap = self.bottom_pane.list_keymap();
                    params.footer_hint = Some(accept_cancel_hint_line(
                        list_keymap.primary_hint(ListAction::Accept),
                        "to confirm",
                        list_keymap.primary_hint(ListAction::Cancel),
                        "to continue working",
                    ));
                }
                // Use the standard focused picker: arrows/Enter and numeric shortcuts select,
                // Escape returns to the unchanged composer and its saved draft.
                let dismissed = Arc::clone(&self.backend_banner_state.picker_dismissed);
                params.on_cancel =
                    Some(Box::new(move |_| dismissed.store(true, Ordering::Relaxed)));
                for item in &mut params.items {
                    item.dismiss_on_select = true;
                    let dismissed = Arc::clone(&self.backend_banner_state.picker_dismissed);
                    item.actions
                        .push(Box::new(move |_| dismissed.store(true, Ordering::Relaxed)));
                }
                self.backend_banner_state.shown = true;
                self.bottom_pane.show_selection_view(params);
            }
            (_, content) => self.bottom_pane.set_inline_banner(content),
        }
        if self.backend_banner_state.presented.is_none()
            && !self.bottom_pane.is_task_running()
            && self.bottom_pane.no_modal_or_popup_active()
        {
            self.maybe_show_pending_rate_limit_prompt();
        }
    }

    pub(super) fn sync_backend_banner_view(&mut self) {
        self.observe_backend_banner_view();
        self.refresh_backend_banner_visibility();
    }

    pub(super) fn dismiss_backend_banner_for_new_turn(&mut self) {
        self.observe_backend_banner_view();
        if self.backend_banner_state.shown
            && self
                .backend_banner_state
                .presented
                .as_ref()
                .is_some_and(|banner| banner.presentation == BannerPresentation::Dismissible)
        {
            self.backend_banner_state.dismissed = true;
            self.refresh_backend_banner_visibility();
        }
    }

    pub(super) fn has_applicable_backend_banner(&self) -> bool {
        !self.backend_banner_state.dismissed && self.backend_banner_state.presented.is_some()
    }

    pub(crate) fn clear_backend_banner(&mut self) {
        self.backend_banner_state = BackendBannerState::default();
        self.backend_banner_notice_model = None;
        self.bottom_pane
            .dismiss_view_by_id(LUNA_RESERVE_RECOVERY_VIEW_ID);
        self.bottom_pane.set_inline_banner(/*banner*/ None);
    }
}
