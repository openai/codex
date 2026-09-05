//! Connect the retained question editor to ordinary input delivery and queue navigation.
//! Only an accepted local send or queue operation consumes an asynchronous answer.

use super::*;
use crate::bottom_pane::QuestionSubmission;
use codex_protocol::items::AsyncUserInputQuestion;

impl ChatWidget {
    pub(super) fn add_async_questions(
        &mut self,
        message_id: &str,
        questions: &[AsyncUserInputQuestion],
    ) {
        self.bottom_pane.push_async_questions(message_id, questions);
        self.refresh_pending_input_preview();
    }

    pub(super) fn handle_question_key(&mut self, key: KeyEvent) -> bool {
        if self.bottom_pane.has_active_view() {
            return false;
        }
        if key.kind == KeyEventKind::Release {
            return self
                .bottom_pane
                .questions
                .as_ref()
                .is_some_and(|q| q.expanded);
        }
        let expanded = self
            .bottom_pane
            .questions
            .as_ref()
            .is_some_and(|q| q.expanded);
        let editing = expanded
            && self
                .bottom_pane
                .questions
                .as_ref()
                .is_some_and(|q| q.handles_key_as_editing(key));
        let forward = self.chat_keymap.edit_queued_message.is_pressed(key);
        let backward = self.chat_keymap.prompt_stack_back.is_pressed(key);
        if key.kind == KeyEventKind::Press && !editing && (forward || backward) {
            if let Some(questions) = self.bottom_pane.questions.as_mut().filter(|q| q.expanded) {
                if questions.navigate(forward) {
                    self.request_redraw();
                    return true;
                }
                if backward {
                    questions.set_expanded(/*expanded*/ false);
                    self.request_redraw();
                    return true;
                }
            } else if forward
                && self.bottom_pane.no_modal_or_popup_active()
                && let Some(questions) = self
                    .bottom_pane
                    .questions
                    .as_mut()
                    .filter(|q| q.unanswered_count() > 0)
            {
                questions.set_expanded(/*expanded*/ true);
                self.request_redraw();
                return true;
            }
            if expanded
                && forward
                && !self.blocks_direct_input
                && let Some(composer) = self.pop_latest_queued_composer_state()
            {
                if let Some(questions) = &mut self.bottom_pane.questions {
                    questions.set_expanded(/*expanded*/ false);
                }
                self.restore_composer_state(composer);
                self.refresh_pending_input_preview();
                self.request_redraw();
                return true;
            }
            return expanded;
        }
        if !expanded {
            return false;
        }
        if key.code == KeyCode::Esc && self.local_settings.tui.question_esc_back && !editing {
            if let Some(questions) = &mut self.bottom_pane.questions {
                questions.set_expanded(/*expanded*/ false);
            }
        } else if key_hint::ctrl(KeyCode::Char('c')).is_press(key) {
            self.on_ctrl_c();
        } else {
            let interrupting = !editing && self.chat_keymap.interrupt_turn.is_pressed(key);
            self.bottom_pane.handle_key_event(key);
            if interrupting {
                self.pause_active_goal_for_interrupt();
            }
            let submission = self
                .bottom_pane
                .questions
                .as_mut()
                .and_then(|q| q.submission.take());
            if let Some(submission) = submission {
                let (text, queued) = match submission {
                    QuestionSubmission::Submit(text) => (text, false),
                    QuestionSubmission::Queue(text) => (text, true),
                };
                if self.blocks_direct_input {
                    self.add_error_message(PARENT_OWNED_INPUT_MESSAGE.to_string());
                    return true;
                }
                if self.has_misalignment_policy_violation() {
                    return true;
                }
                let main = self.bottom_pane.composer_draft_snapshot();
                let accepted = if queued
                    || self.input_queue.suppress_queue_autosend
                    || self.is_plan_streaming_in_tui()
                    || self.input_queue.user_turn_pending_start
                        && !self.turn_lifecycle.agent_turn_running
                    || self.only_user_shell_commands_running()
                {
                    self.queue_user_message(UserMessage::from(text));
                    true
                } else {
                    self.submit_user_message_with_history_and_shell_escape_policy(
                        UserMessage::from(text),
                        UserMessageHistoryRecord::UserMessageText,
                        ShellEscapePolicy::Disallow,
                    )
                    .0
                };
                if accepted {
                    if let Some(questions) = &mut self.bottom_pane.questions {
                        questions.accept_answer();
                    }
                } else if self.bottom_pane.composer_draft_snapshot() != main {
                    let cursor = main.cursor;
                    self.restore_composer_state(ThreadComposerState {
                        text: main.text,
                        text_elements: main.text_elements,
                        local_images: main.local_images,
                        remote_image_urls: main.remote_image_urls,
                        mention_bindings: main.mention_bindings,
                        pending_pastes: main.pending_pastes,
                    });
                    self.bottom_pane.set_composer_cursor(cursor);
                }
            }
        }
        self.request_redraw();
        true
    }
}
