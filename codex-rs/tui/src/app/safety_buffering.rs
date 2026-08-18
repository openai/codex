//! Safety-buffered turn retries that preserve the source thread.

use super::session_lifecycle::ThreadAttachPresentation;
use super::*;
use crate::app_server_session::ForkGoalContinuation;
use crate::app_server_session::HISTORY_ITEM_PAGE_LIMIT;
use crate::app_server_session::turn_permissions_overrides;
use crate::chatwidget::ThreadInputState;
use crate::chatwidget::ThreadInputStateRestoreMode;
use crate::chatwidget::UserMessage;
use codex_app_server_protocol::ThreadHistoryMode;
use codex_app_server_protocol::TurnItemsView;
use codex_app_server_protocol::UserInput;

pub(super) struct SafetyBufferedRetry {
    pub(super) thread_id: ThreadId,
    pub(super) turn_id: String,
    pub(super) model: String,
    pub(super) turn: AppCommand,
    pub(super) prompt: UserMessage,
}

impl App {
    pub(super) async fn retry_safety_buffered_turn(
        &mut self,
        tui: &mut tui::Tui,
        app_server: &mut AppServerSession,
        retry: SafetyBufferedRetry,
    ) {
        let SafetyBufferedRetry {
            thread_id,
            turn_id,
            model,
            mut turn,
            prompt,
        } = retry;
        if self.active_thread_id != Some(thread_id)
            || self.chat_widget.thread_id() != Some(thread_id)
            || self.primary_thread_id != Some(thread_id)
        {
            return;
        }
        if !self.chat_widget.can_retry_safety_buffered_turn(&turn_id) {
            self.app_event_tx.send(AppEvent::UpdateModel(model));
            self.app_event_tx.send(AppEvent::UpdateReasoningEffort(Some(
                ReasoningEffortConfig::Low,
            )));
            return;
        }

        let retry_config = self.chat_widget.config_ref().clone();
        let input_state = self.chat_widget.capture_thread_input_state();

        let AppCommand::UserTurn {
            items,
            cwd,
            active_permission_profile,
            model: turn_model,
            effort,
            collaboration_mode,
            ..
        } = &mut turn
        else {
            self.chat_widget.add_error_message(
                "Failed to retry with a faster model: original turn is unavailable.".to_string(),
            );
            return;
        };
        let permissions_override = Self::turn_permissions_override_from_config(
            &retry_config,
            active_permission_profile.as_ref(),
            self.runtime_permission_profile_override
                .as_ref()
                .map(|profile| &profile.permission_profile),
        );
        if let Err(err) = turn_permissions_overrides(permissions_override, cwd.as_path()) {
            self.chat_widget
                .add_error_message(format!("Failed to retry with a faster model: {err}"));
            return;
        }
        *turn_model = model.clone();
        *effort = Some(ReasoningEffortConfig::Low);
        *collaboration_mode = collaboration_mode.as_ref().map(|mode| {
            mode.with_updates(
                Some(model),
                Some(Some(ReasoningEffortConfig::Low)),
                /*developer_instructions*/ None,
            )
        });

        if let Err(err) = app_server.turn_interrupt(thread_id, turn_id.clone()).await {
            self.chat_widget
                .add_error_message(format!("Failed to retry with a faster model: {err}"));
            return;
        }

        let thread = match async {
            let mut thread = app_server
                .thread_read(thread_id, /*include_turns*/ false)
                .await?;
            if thread.history_mode == ThreadHistoryMode::Legacy {
                app_server
                    .hydrate_initial_thread_history(
                        &mut thread,
                        /*turn_cursor*/ None,
                        /*item_cursor*/ None,
                        /*config*/ None,
                        crate::app_server_session::HistoryHydrationScope::Initial,
                    )
                    .await?;
            } else {
                let page = app_server
                    .thread_turns_page(thread_id, /*cursor*/ None)
                    .await?;
                thread.turns = page.data.into_iter().rev().collect();
                if let Some(turn_index) = thread.turns.iter().position(|turn| turn.id == turn_id) {
                    let page = app_server
                        .thread_items_page(
                            thread_id,
                            Some(&turn_id),
                            /*cursor*/ None,
                            HISTORY_ITEM_PAGE_LIMIT,
                        )
                        .await?;
                    if page.next_cursor.is_some() {
                        color_eyre::eyre::bail!(
                            "Cannot safely retry a turn whose input exceeds the bounded history page."
                        );
                    }
                    let turn = &mut thread.turns[turn_index];
                    turn.items = page
                        .data
                        .into_iter()
                        .rev()
                        .map(|entry| entry.item)
                        .collect();
                    turn.items_view = TurnItemsView::Full;
                }
            }
            Ok::<_, color_eyre::Report>(thread)
        }
        .await
        {
            Ok(thread) => thread,
            Err(err) => {
                self.fail_safety_buffered_branch(input_state, prompt, err);
                return;
            }
        };
        if let Err(err) = safety_retry_fork_point(&thread.turns, &turn_id) {
            self.fail_safety_buffered_branch(input_state, prompt, err);
            return;
        }
        items.extend(
            thread
                .turns
                .iter()
                .find(|turn| turn.id == turn_id)
                .into_iter()
                .flat_map(|turn| &turn.items)
                .filter_map(|item| match item {
                    ThreadItem::UserMessage { content, .. } => Some(content),
                    _ => None,
                })
                .skip(/*n*/ 1)
                .flat_map(|content| {
                    std::iter::once(UserInput::Text {
                        text: "\n".to_string(),
                        text_elements: Vec::new(),
                    })
                    .chain(content.iter().cloned())
                }),
        );
        let retry_display = ChatWidget::user_message_display_from_inputs(items);

        self.config = retry_config.clone();
        let started = app_server
            .fork_thread_at(
                retry_config,
                thread_id,
                /*last_turn_id*/ None,
                /*before_turn_id*/ Some(turn_id),
                ForkGoalContinuation::DeferUntilNextTurn,
            )
            .await;
        let started = match started {
            Ok(started) => started,
            Err(err) => {
                self.fail_safety_buffered_branch(input_state, prompt, err);
                return;
            }
        };
        let retry_thread_id = started.session.thread_id;

        self.shutdown_current_thread(app_server).await;
        if let Err(err) = self
            .replace_chat_widget_with_app_server_thread(
                tui,
                started,
                ThreadAttachPresentation::SessionLineage,
                /*initial_user_message*/ None,
            )
            .await
        {
            self.fail_safety_buffered_branch(input_state, prompt, err);
            return;
        }

        let failure_input_state = input_state.clone();
        self.chat_widget.restore_thread_input_state(
            input_state,
            ThreadInputStateRestoreMode {
                preserve_in_flight_turn: false,
            },
        );
        self.chat_widget
            .prepare_safety_buffered_retry_submission(prompt.clone());
        if let Err(err) = self
            .submit_thread_op(app_server, retry_thread_id, turn)
            .await
        {
            self.fail_safety_buffered_branch(failure_input_state, prompt, err);
            return;
        }
        self.chat_widget
            .commit_safety_buffered_retry_submission(retry_display);
    }

    fn fail_safety_buffered_branch(
        &mut self,
        input_state: Option<ThreadInputState>,
        prompt: UserMessage,
        err: impl std::fmt::Display,
    ) {
        self.chat_widget.restore_thread_input_state(
            input_state,
            ThreadInputStateRestoreMode {
                preserve_in_flight_turn: false,
            },
        );
        self.chat_widget.cancel_safety_buffered_retry_submission();
        self.chat_widget.restore_user_message_to_composer(prompt);
        self.chat_widget
            .add_error_message(format!("Failed to retry with a faster model: {err}"));
    }
}

fn safety_retry_fork_point(turns: &[Turn], turn_id: &str) -> Result<()> {
    let Some(turn_index) = turns.iter().position(|turn| turn.id == turn_id) else {
        return Err(color_eyre::eyre::eyre!(
            "interrupted turn {turn_id} is missing from the source thread"
        ));
    };
    if turn_index + 1 != turns.len() {
        return Err(color_eyre::eyre::eyre!(
            "interrupted turn {turn_id} is no longer the latest turn"
        ));
    }
    if turns[turn_index].status == TurnStatus::InProgress {
        return Err(color_eyre::eyre::eyre!(
            "interrupted turn {turn_id} is still in progress"
        ));
    }

    let Some(previous_turn) = turns[..turn_index].last() else {
        return Ok(());
    };
    if previous_turn.status == TurnStatus::InProgress {
        return Err(color_eyre::eyre::eyre!(
            "previous turn {} is still in progress",
            previous_turn.id
        ));
    }

    Ok(())
}

#[cfg(test)]
#[path = "safety_buffering_tests.rs"]
mod tests;
