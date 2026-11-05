//! The unfocused pane projects assistant replies and turn status while the thread event
//! store continues to own replay.

use super::ThreadEventStore;
use crate::app_event::CapturedSideDisplay;
use crate::app_event::CapturedSideWork;
use crate::git_action_directives::parse_assistant_markdown;
use crate::history_cell::AgentMarkdownCell;
use crate::history_cell::FinalMessageSeparator;
use crate::history_cell::HistoryCell;
use crate::transcript_view::TranscriptView;
use chrono::Local;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ThreadItemPayload;
use codex_app_server_protocol::TurnStatus;
use codex_protocol::items::AsyncUserInputQuestion;
use codex_utils_absolute_path::AbsolutePathBuf;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

pub(super) struct SideCompanion {
    pub(super) display: CapturedSideDisplay,
    pub(super) cells: Vec<Arc<dyn HistoryCell>>,
    pub(super) view: TranscriptView,
    pub(super) answer: Option<SideAnswer>,
    pub(super) parked_tail: usize,
}

pub(super) struct SideAnswer {
    turn_id: String,
    item_id: String,
    index: usize,
    pending: String,
}

impl SideAnswer {
    pub(super) fn new((turn_id, item_id): (String, String), index: usize) -> Self {
        Self {
            turn_id,
            item_id,
            index,
            pending: String::new(),
        }
    }

    fn matches(&self, turn_id: &str, item_id: &str) -> bool {
        self.turn_id == turn_id && self.item_id == item_id
    }
}

impl SideCompanion {
    pub(super) fn empty(cwd: AbsolutePathBuf) -> Self {
        Self {
            display: CapturedSideDisplay {
                rendered_completion_turn_ids: Default::default(),
                active: None,
                auxiliary: Vec::new(),
                draft: Box::default(),
                status: None,
                work: None,
                last_completed_agent_message: None,
                cwd,
                visualizations: None,
            },
            cells: Vec::new(),
            view: TranscriptView::default(),
            answer: None,
            parked_tail: 0,
        }
    }

    pub(super) fn apply_notification(
        &mut self,
        notification: &ServerNotification,
        store: &ThreadEventStore,
        new_status: impl FnOnce() -> crate::status_indicator_widget::StatusIndicatorWidget,
    ) -> bool {
        match notification {
            ServerNotification::TurnStarted(event)
                if self
                    .display
                    .work
                    .as_ref()
                    .is_none_or(|work| work.turn_id != event.turn.id) =>
            {
                self.display.work = Some(CapturedSideWork::new(
                    event.turn.id.clone(),
                    new_status(),
                    Duration::ZERO,
                ));
            }
            ServerNotification::AgentMessageDelta(delta)
                if !delta.delta.is_empty() && !store.is_realtime_delegated_turn(&delta.turn_id) =>
            {
                if self
                    .answer
                    .as_ref()
                    .is_some_and(|answer| !answer.matches(&delta.turn_id, &delta.item_id))
                {
                    self.flush_answer();
                    self.answer = None;
                }
                let answer = self.answer.get_or_insert_with(|| {
                    SideAnswer::new(
                        (delta.turn_id.clone(), delta.item_id.clone()),
                        self.cells.len(),
                    )
                });
                answer.pending.push_str(&delta.delta);
            }
            ServerNotification::ItemCompleted(event) => {
                let item = &event.item.payload;
                if store.is_realtime_delegated_turn(&event.turn_id)
                    && crate::chatwidget::is_private_realtime_agent_item(item)
                {
                    return false;
                }
                return self.finish_item(&event.turn_id, item);
            }
            ServerNotification::TurnCompleted(event)
                if event.turn.status != TurnStatus::InProgress =>
            {
                let turn = &event.turn;
                if self.display.active.is_some() {
                    for item in turn.item_payloads() {
                        if matches!(item, ThreadItemPayload::CommandExecution { .. }) {
                            self.finish_item(&turn.id, item);
                        }
                    }
                }
                if turn.status == TurnStatus::Completed
                    && let Some((item, _)) = crate::chatwidget::final_agent_message(
                        turn,
                        store.is_realtime_delegated_turn(&turn.id),
                    )
                    && crate::chatwidget::needs_agent_message(
                        self.display.last_completed_agent_message.as_ref(),
                        &turn.id,
                        item,
                    )
                {
                    self.finish_item(&turn.id, item);
                }
                if self
                    .answer
                    .as_ref()
                    .is_some_and(|answer| answer.turn_id == turn.id)
                {
                    self.flush_answer();
                    self.answer = None;
                }
                let work = self.display.work.take_if(|work| work.turn_id == turn.id);
                if turn.status == TurnStatus::Completed
                    && (work.is_some() || store.active_turn_id() == Some(turn.id.as_str()))
                    && let Some(completion) = FinalMessageSeparator::from_turn_once(
                        turn,
                        &mut self.display.rendered_completion_turn_ids,
                        work.map(|work| work.timer.elapsed_at(Instant::now())),
                        Some(Local::now()),
                    )
                {
                    self.cells.push(Arc::new(completion));
                }
            }
            ServerNotification::ThreadClosed(_) => return self.display.work.take().is_some(),
            _ => return false,
        }
        true
    }

    /// History pages may arrive while a reply or command still owns a slot in the parked transcript.
    pub(super) fn splice_older_cells(
        &mut self,
        range: std::ops::Range<usize>,
        cells: Vec<Arc<dyn HistoryCell>>,
    ) {
        let shift = |index: usize| {
            if index >= range.end {
                index - range.len() + cells.len()
            } else {
                index
            }
        };
        self.parked_tail = shift(self.parked_tail);
        if let Some(answer) = &mut self.answer {
            answer.index = shift(answer.index);
        }
        self.cells.splice(range, cells);
    }

    fn finish_item(&mut self, turn: &str, item: &ThreadItemPayload) -> bool {
        match item {
            ThreadItemPayload::AgentMessage {
                id,
                text,
                questions,
                ..
            } => {
                let replaces = self
                    .answer
                    .as_ref()
                    .is_some_and(|answer| answer.matches(turn, id));
                if !replaces || text.is_empty() {
                    self.flush_answer();
                }
                let finished = self.answer.take_if(|_| replaces);
                if !text.is_empty() {
                    let index = finished.map_or(self.cells.len(), |answer| answer.index);
                    self.write_answer(
                        index,
                        text.clone(),
                        questions.as_deref().unwrap_or_default(),
                    );
                }
                self.display.last_completed_agent_message = Some((turn.to_owned(), id.clone()));
            }
            ThreadItemPayload::CommandExecution {
                id,
                status,
                exit_code,
                source,
                aggregated_output,
                duration_ms,
                ..
            } => {
                let Some(cell) = self.display.active.as_mut().and_then(|active| {
                    active
                        .as_any_mut()
                        .downcast_mut::<crate::exec_cell::ExecCell>()
                }) else {
                    return false;
                };
                if *status == codex_app_server_protocol::CommandExecutionStatus::InProgress
                    || !cell.complete_call(
                        id,
                        crate::exec_cell::CommandOutput::from_execution(
                            status.clone(),
                            *exit_code,
                            *source,
                            aggregated_output.clone(),
                        ),
                        Duration::from_millis(duration_ms.unwrap_or_default().max(0) as u64),
                    )
                {
                    return false;
                }
                if !cell.is_active()
                    && let Some(active) = self.display.active.take()
                {
                    // Later answer deltas may already have appended a cell after the parked command.
                    self.cells.insert(self.parked_tail, Arc::from(active));
                    if let Some(answer) = &mut self.answer
                        && answer.index >= self.parked_tail
                    {
                        answer.index += 1;
                    }
                }
            }
            _ => return false,
        }
        true
    }

    /// Commit pending deltas at paint time so parsing and wrapping run once per frame.
    pub(super) fn flush_answer(&mut self) {
        let Some(answer) = self
            .answer
            .as_mut()
            .filter(|answer| !answer.pending.is_empty())
        else {
            return;
        };
        let index = answer.index;
        let mut source = self
            .cells
            .get(index)
            .and_then(|cell| cell.copy_source())
            .unwrap_or_default()
            .to_owned();
        source.push_str(&answer.pending);
        answer.pending.clear();
        self.write_answer(index, source, &[]);
    }

    fn write_answer(&mut self, index: usize, source: String, questions: &[AsyncUserInputQuestion]) {
        let cwd = &self.display.cwd;
        let markdown = parse_assistant_markdown(&source, cwd.as_path());
        let cell: Arc<dyn HistoryCell> = Arc::new(AgentMarkdownCell::from_parsed_message(
            markdown.visible_markdown,
            source,
            cwd.as_path(),
            self.display.visualizations.clone(),
            questions,
        ));
        if index == self.cells.len() {
            self.cells.push(cell);
        } else {
            self.view
                .replace_range(&self.cells, index..index + 1, &cell);
            self.cells[index] = cell;
        }
    }
}
