//! Random foreground-turn tips with separate working and completion frequency rules.
//! A FIFO completion event anchors the tip after queued history; replay never creates tips.

use super::*;
use crate::terminal_hyperlinks::prefix_hyperlink_lines;
use crate::turn_tip::TurnTip;
use codex_protocol::models::MessagePhase;
use rand::seq::SliceRandom;
use std::sync::Weak;

const WORKING_DELAY: Duration = Duration::from_secs(/*secs*/ 30);
const COMPLETION_INTERVAL: usize = 3;
const COMPLETION_LIMIT: usize = 2;

#[derive(Default)]
pub(super) struct TurnTips {
    starts: usize,
    completions_shown: usize,
    next_completion: usize,
    previous: Option<&'static str>,
    current: Option<TurnTipState>,
}

struct TurnTipState {
    thread_id: ThreadId,
    turn_id: String,
    started_at: Instant,
    phase: Phase,
    has_final_answer: bool,
    template: Option<&'static str>,
    shown: bool,
}

enum Phase {
    Working,
    WaitingForHistory,
    Complete(Weak<dyn HistoryCell>),
    Finished,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum TipSurface {
    Working,
    Completion,
}

fn is_final_answer(item: &ThreadItem) -> bool {
    matches!(item, ThreadItem::AgentMessage { text, phase: Some(MessagePhase::FinalAnswer) | None, .. }
        if !text.trim().is_empty())
}

impl TurnTips {
    pub(super) fn dismiss(&mut self) {
        self.current = None;
    }

    pub(super) fn observe(
        &mut self,
        notification: &ServerNotification,
        now: Instant,
    ) -> Option<AppEvent> {
        if let ServerNotification::TurnStarted(started) = notification {
            let thread_id = ThreadId::from_string(&started.thread_id).ok()?;
            if self.current.as_ref().is_some_and(|current| {
                current.thread_id == thread_id && current.turn_id == started.turn.id
            }) {
                return None;
            }
            self.starts = self.starts.saturating_add(/*rhs*/ 1);
            self.current = Some(TurnTipState {
                thread_id,
                turn_id: started.turn.id.clone(),
                started_at: now,
                phase: Phase::Working,
                has_final_answer: false,
                template: None,
                shown: false,
            });
            return None;
        }
        let current = self.current.as_mut()?;
        match notification {
            ServerNotification::ItemCompleted(item)
                if item.thread_id == current.thread_id.to_string()
                    && item.turn_id == current.turn_id =>
            {
                current.has_final_answer |= is_final_answer(&item.item);
            }
            ServerNotification::TurnCompleted(completed)
                if completed.thread_id == current.thread_id.to_string()
                    && completed.turn.id == current.turn_id
                    && matches!(current.phase, Phase::Working) =>
            {
                current.has_final_answer |= completed.turn.items.iter().any(is_final_answer);
                if completed.turn.status == TurnStatus::Completed
                    && current.has_final_answer
                    && !current.shown
                    && self.completions_shown < COMPLETION_LIMIT
                    && self.starts >= self.next_completion.max(COMPLETION_INTERVAL)
                {
                    current.phase = Phase::WaitingForHistory;
                    return Some(AppEvent::TurnTipReady {
                        thread_id: current.thread_id,
                        turn_id: current.turn_id.clone(),
                    });
                }
                current.phase = Phase::Finished;
            }
            _ => {}
        }
        None
    }

    pub(super) fn ready(
        &mut self,
        thread_id: ThreadId,
        turn_id: &str,
        tail: Option<&Arc<dyn HistoryCell>>,
    ) {
        if let Some(current) = self.current.as_mut()
            && current.thread_id == thread_id
            && current.turn_id == turn_id
            && matches!(current.phase, Phase::WaitingForHistory)
        {
            current.phase = tail.map_or(Phase::Finished, |tail| {
                Phase::Complete(Arc::downgrade(tail))
            });
        }
    }

    pub(super) fn acknowledge(&mut self, surface: TipSurface) {
        if let Some(current) = self.current.as_mut()
            && !current.shown
        {
            current.shown = true;
            self.previous = current.template;
            if surface == TipSurface::Completion {
                self.completions_shown += 1;
                self.next_completion = self.starts.saturating_add(COMPLETION_INTERVAL);
            }
        }
    }
}

impl App {
    pub(super) fn turn_tip(
        &mut self,
        width: u16,
        now: Instant,
        frame_requester: &tui::FrameRequester,
    ) -> Option<(TipSurface, TurnTip)> {
        if !self.local_settings.tui.show_tooltips
            || !self.chat_widget.no_modal_or_popup_active()
            || !self.chat_widget.composer_is_empty()
            || self.chat_widget.is_external_writer_view()
            || self.chat_widget.has_queued_follow_up_messages()
            || !self.transcript_view.is_following()
            || self.transcript_view.has_active_interaction()
            || self.backtrack.primed
            || self.backtrack.overlay_preview_active
            || self.composer_hint(width).is_some()
        {
            return None;
        }
        let current = self.turn_tips.current.as_mut()?;
        if self.chat_widget.thread_id() != Some(current.thread_id) {
            return None;
        }
        let surface = match &current.phase {
            Phase::Working if self.chat_widget.is_agent_turn_running() => {
                let remaining =
                    WORKING_DELAY.saturating_sub(now.saturating_duration_since(current.started_at));
                if !remaining.is_zero() {
                    frame_requester.schedule_frame_in(remaining);
                    return None;
                }
                TipSurface::Working
            }
            Phase::Complete(tail) if !self.chat_widget.is_user_turn_pending_or_running() => {
                if !self
                    .transcript_cells
                    .last()
                    .is_some_and(|last| tail.ptr_eq(&Arc::downgrade(last)))
                {
                    current.phase = Phase::Finished;
                    return None;
                }
                TipSurface::Completion
            }
            _ => return None,
        };
        let content_width = usize::from(width.checked_sub(/*rhs*/ 4)?);
        let render = |template| {
            let text = crate::tooltips::render_tooltip(template, Some(&self.keymap))?;
            let lines = crate::tooltips::render_tooltip_lines(
                &text,
                content_width,
                self.config.cwd.as_path(),
            );
            match lines.as_slice() {
                [line] if line.width() <= content_width => Some(line.clone()),
                _ => None,
            }
        };
        if current.template.is_none() {
            let mut templates = crate::tooltips::tooltip_templates().collect::<Vec<_>>();
            templates.shuffle(&mut rand::rng());
            current.template = templates
                .iter()
                .copied()
                .filter(|template| Some(*template) != self.turn_tips.previous)
                .find(|template| render(template).is_some())
                .or_else(|| {
                    templates
                        .into_iter()
                        .find(|template| render(template).is_some())
                });
        }
        let line = render(current.template?)?;
        let line = prefix_hyperlink_lines(vec![line], "  └ ".dim(), "    ".dim())
            .pop()?
            .style(crate::style::secondary_text_style());
        Some((
            surface,
            TurnTip {
                line,
                rendered: Default::default(),
            },
        ))
    }
}

#[cfg(test)]
#[path = "turn_tips_tests.rs"]
mod tests;
