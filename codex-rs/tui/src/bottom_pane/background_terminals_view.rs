use std::collections::HashMap;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;

use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use ratatui::buffer::Buffer;
use ratatui::layout::Constraint;
use ratatui::layout::Layout;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::bottom_pane::CancellationEvent;
use crate::bottom_pane::bottom_pane_view::BottomPaneView;
use crate::bottom_pane::scroll_state::ScrollState;
use crate::bottom_pane::selection_popup_common::GenericDisplayRow;
use crate::bottom_pane::selection_popup_common::render_rows;
use crate::exec_command::strip_bash_lc_and_escape;
use crate::key_hint;
use crate::render::Insets;
use crate::render::RectExt as _;
use crate::render::renderable::Renderable;
use codex_core::protocol::ExecCommandBeginEvent;
use codex_core::protocol::ExecCommandEndEvent;
use codex_core::protocol::ExecCommandOutputDeltaEvent;
use codex_core::protocol::ExecCommandSource;
use codex_core::protocol::Op;
use codex_core::protocol::TerminalInteractionEvent;

const MAX_LOG_LINES: usize = 500;
const MAX_COMPLETED_PROCESSES: usize = 20;
const MAX_LIST_ROWS: usize = 12;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BackgroundTerminalStatus {
    Running,
    Exited { exit_code: i32 },
}

impl BackgroundTerminalStatus {
    fn description(self) -> String {
        match self {
            BackgroundTerminalStatus::Running => "running".to_string(),
            BackgroundTerminalStatus::Exited { exit_code } => {
                format!("exited ({exit_code})")
            }
        }
    }

    fn is_running(self) -> bool {
        matches!(self, BackgroundTerminalStatus::Running)
    }

    fn is_exited(self) -> bool {
        matches!(self, BackgroundTerminalStatus::Exited { .. })
    }
}

#[derive(Clone, Debug)]
pub(crate) struct BackgroundTerminalListItem {
    pub(crate) process_id: String,
    pub(crate) command_display: String,
    pub(crate) status: BackgroundTerminalStatus,
}

#[derive(Clone, Debug)]
pub(crate) struct BackgroundTerminalSnapshot {
    pub(crate) process_id: String,
    pub(crate) command_display: String,
    pub(crate) cwd: PathBuf,
    pub(crate) status: BackgroundTerminalStatus,
    pub(crate) output_lines: Vec<String>,
}

struct LogBuffer {
    lines: VecDeque<String>,
    partial: String,
}

impl LogBuffer {
    fn new() -> Self {
        Self {
            lines: VecDeque::new(),
            partial: String::new(),
        }
    }

    fn push_chunk(&mut self, chunk: &str) {
        let cleaned = chunk.replace("\r\n", "\n").replace('\r', "\n");
        if cleaned.is_empty() {
            return;
        }
        let mut combined = std::mem::take(&mut self.partial);
        combined.push_str(&cleaned);
        let mut parts = combined.split('\n').peekable();
        while let Some(part) = parts.next() {
            if parts.peek().is_some() {
                self.push_line(part.to_string());
            } else {
                self.partial = part.to_string();
            }
        }
    }

    fn push_line(&mut self, line: String) {
        if self.lines.len() >= MAX_LOG_LINES {
            self.lines.pop_front();
        }
        self.lines.push_back(line);
    }

    fn snapshot_lines(&self) -> Vec<String> {
        let mut out: Vec<String> = self.lines.iter().cloned().collect();
        if !self.partial.is_empty() {
            out.push(self.partial.clone());
        }
        out
    }
}

struct BackgroundTerminalEntry {
    process_id: String,
    command_display: String,
    cwd: PathBuf,
    status: BackgroundTerminalStatus,
    log: LogBuffer,
}

pub(crate) struct BackgroundTerminalsState {
    processes: Vec<BackgroundTerminalEntry>,
    call_id_to_process: HashMap<String, String>,
}

impl BackgroundTerminalsState {
    pub(crate) fn new() -> Self {
        Self {
            processes: Vec::new(),
            call_id_to_process: HashMap::new(),
        }
    }

    pub(crate) fn on_exec_begin(&mut self, ev: &ExecCommandBeginEvent) -> bool {
        if !is_unified_exec_source(ev.source) {
            return false;
        }
        let process_id = ev.process_id.clone().unwrap_or_else(|| ev.call_id.clone());
        self.call_id_to_process
            .insert(ev.call_id.clone(), process_id.clone());

        if ev.source == ExecCommandSource::UnifiedExecInteraction {
            if self.find_process_mut(&process_id).is_none() {
                let command_display = strip_bash_lc_and_escape(&ev.command);
                self.processes.push(BackgroundTerminalEntry {
                    process_id: process_id.clone(),
                    command_display,
                    cwd: ev.cwd.clone(),
                    status: BackgroundTerminalStatus::Running,
                    log: LogBuffer::new(),
                });
            }
            if let Some(input) = ev.interaction_input.as_ref() {
                self.append_input(&process_id, input);
            }
            return true;
        }

        let command_display = strip_bash_lc_and_escape(&ev.command);
        if let Some(entry) = self.find_process_mut(&process_id) {
            entry.command_display = command_display;
            entry.cwd = ev.cwd.clone();
            entry.status = BackgroundTerminalStatus::Running;
        } else {
            self.processes.push(BackgroundTerminalEntry {
                process_id,
                command_display,
                cwd: ev.cwd.clone(),
                status: BackgroundTerminalStatus::Running,
                log: LogBuffer::new(),
            });
        }
        true
    }

    pub(crate) fn on_exec_output_delta(&mut self, ev: &ExecCommandOutputDeltaEvent) -> bool {
        let Some(process_id) = self.call_id_to_process.get(&ev.call_id).cloned() else {
            return false;
        };
        let Some(entry) = self.find_process_mut(&process_id) else {
            return false;
        };
        let text = String::from_utf8_lossy(&ev.chunk);
        entry.log.push_chunk(text.as_ref());
        true
    }

    pub(crate) fn on_exec_end(&mut self, ev: &ExecCommandEndEvent) -> bool {
        if !is_unified_exec_source(ev.source) {
            return false;
        }
        let process_id = ev.process_id.clone().unwrap_or_else(|| ev.call_id.clone());
        let exit_code = ev.exit_code;
        self.call_id_to_process
            .retain(|_, value| value != &process_id);

        let Some(idx) = self
            .processes
            .iter()
            .position(|entry| entry.process_id == process_id)
        else {
            return false;
        };

        let mut entry = self.processes.remove(idx);
        entry.status = BackgroundTerminalStatus::Exited { exit_code };
        entry
            .log
            .push_line(format!("process exited with code {exit_code}"));
        self.processes.push(entry);
        self.prune_completed_processes();
        true
    }

    pub(crate) fn on_terminal_interaction(&mut self, ev: &TerminalInteractionEvent) -> bool {
        if ev.stdin.is_empty() {
            return false;
        }
        self.append_input(&ev.process_id, &ev.stdin);
        true
    }

    pub(crate) fn running_command_displays(&self) -> Vec<String> {
        self.processes
            .iter()
            .filter(|entry| entry.status.is_running())
            .map(|entry| entry.command_display.clone())
            .collect()
    }

    pub(crate) fn list_items(&self) -> Vec<BackgroundTerminalListItem> {
        self.processes
            .iter()
            .map(|entry| BackgroundTerminalListItem {
                process_id: entry.process_id.clone(),
                command_display: entry.command_display.clone(),
                status: entry.status,
            })
            .collect()
    }

    pub(crate) fn snapshot_process(&self, process_id: &str) -> Option<BackgroundTerminalSnapshot> {
        let entry = self.processes.iter().find(|p| p.process_id == process_id)?;
        Some(BackgroundTerminalSnapshot {
            process_id: entry.process_id.clone(),
            command_display: entry.command_display.clone(),
            cwd: entry.cwd.clone(),
            status: entry.status,
            output_lines: entry.log.snapshot_lines(),
        })
    }

    pub(crate) fn command_display_for_process(&self, process_id: &str) -> Option<String> {
        self.processes
            .iter()
            .find(|entry| entry.process_id == process_id)
            .map(|entry| entry.command_display.clone())
    }

    fn append_input(&mut self, process_id: &str, input: &str) {
        let Some(entry) = self.find_process_mut(process_id) else {
            return;
        };
        let input = input.trim_end_matches('\n');
        if input.is_empty() {
            return;
        }
        for line in input.split('\n') {
            entry.log.push_line(format!("> {line}"));
        }
    }

    fn prune_completed_processes(&mut self) {
        let completed = self
            .processes
            .iter()
            .filter(|entry| entry.status.is_exited())
            .count();
        let mut to_drop = completed.saturating_sub(MAX_COMPLETED_PROCESSES);
        if to_drop == 0 {
            return;
        }
        self.processes.retain(|entry| {
            if to_drop == 0 {
                return true;
            }
            if entry.status.is_exited() {
                to_drop = to_drop.saturating_sub(1);
                return false;
            }
            true
        });
        let remaining_ids: Vec<String> = self
            .processes
            .iter()
            .map(|entry| entry.process_id.clone())
            .collect();
        self.call_id_to_process
            .retain(|_, value| remaining_ids.iter().any(|id| id == value));
    }

    fn find_process_mut(&mut self, process_id: &str) -> Option<&mut BackgroundTerminalEntry> {
        self.processes
            .iter_mut()
            .find(|entry| entry.process_id == process_id)
    }
}

fn is_unified_exec_source(source: ExecCommandSource) -> bool {
    matches!(
        source,
        ExecCommandSource::UnifiedExecStartup | ExecCommandSource::UnifiedExecInteraction
    )
}

pub(crate) struct BackgroundTerminalsView {
    state: ScrollState,
    selected_key: Option<String>,
    complete: bool,
    app_event_tx: AppEventSender,
    shared_state: Arc<Mutex<BackgroundTerminalsState>>,
}

impl BackgroundTerminalsView {
    pub(crate) fn new(
        shared_state: Arc<Mutex<BackgroundTerminalsState>>,
        app_event_tx: AppEventSender,
    ) -> Self {
        let mut view = Self {
            state: ScrollState::new(),
            selected_key: None,
            complete: false,
            app_event_tx,
            shared_state,
        };
        view.initialize_selection();
        view
    }

    fn initialize_selection(&mut self) {
        let items = self.snapshot_items();
        if let Some(first) = items.first() {
            self.state.selected_idx = Some(0);
            self.selected_key = Some(first.process_id.clone());
        }
    }

    fn move_selection(&mut self, delta: i32, items: &[BackgroundTerminalListItem]) {
        let len = items.len();
        if len == 0 {
            self.state.reset();
            self.selected_key = None;
            return;
        }
        if delta < 0 {
            self.state.move_up_wrap(len);
        } else {
            self.state.move_down_wrap(len);
        }
        self.state.ensure_visible(len, MAX_LIST_ROWS.min(len));
        if let Some(idx) = self.state.selected_idx {
            self.selected_key = Some(items[idx].process_id.clone());
        }
    }

    fn terminate_selected(&mut self, items: &[BackgroundTerminalListItem]) {
        let Some(idx) = self
            .state
            .selected_idx
            .filter(|idx| *idx < items.len())
            .or_else(|| (!items.is_empty()).then_some(0))
        else {
            return;
        };
        let Some(item) = items.get(idx) else {
            return;
        };
        if !item.status.is_running() {
            return;
        }
        self.app_event_tx
            .send(AppEvent::CodexOp(Op::TerminateUnifiedExec {
                process_id: item.process_id.clone(),
            }));
    }

    fn snapshot_items(&self) -> Vec<BackgroundTerminalListItem> {
        let Ok(state) = self.shared_state.lock() else {
            return Vec::new();
        };
        state.list_items()
    }

    fn snapshot_selected(&self, process_id: &str) -> Option<BackgroundTerminalSnapshot> {
        let Ok(state) = self.shared_state.lock() else {
            return None;
        };
        state.snapshot_process(process_id)
    }

    fn sync_selection(&mut self, items: &[BackgroundTerminalListItem]) {
        if items.is_empty() {
            self.state.reset();
            self.selected_key = None;
            return;
        }
        if let Some(selected) = self.selected_key.as_ref()
            && let Some(idx) = items.iter().position(|item| &item.process_id == selected)
        {
            self.state.selected_idx = Some(idx);
        } else {
            self.state.clamp_selection(items.len());
            if let Some(idx) = self.state.selected_idx {
                self.selected_key = Some(items[idx].process_id.clone());
            }
        }
        self.state
            .ensure_visible(items.len(), MAX_LIST_ROWS.min(items.len()));
    }

    fn header_lines(&self, width: u16, running_count: usize) -> Vec<Line<'static>> {
        let mut lines = Vec::new();
        lines.push(Line::from("Background terminals".bold()));
        let summary = if running_count == 0 {
            "No background terminals running.".to_string()
        } else if running_count == 1 {
            "1 background terminal running.".to_string()
        } else {
            format!("{running_count} background terminals running.")
        };
        let width = width.max(1) as usize;
        for wrapped in textwrap::wrap(&summary, width) {
            lines.push(Line::from(wrapped.into_owned()).dim());
        }
        lines
    }

    fn footer_line() -> Line<'static> {
        Line::from(vec![
            key_hint::plain(KeyCode::Up).into(),
            "/".into(),
            key_hint::plain(KeyCode::Down).into(),
            " select".dim(),
            "  ".into(),
            key_hint::plain(KeyCode::Char('x')).into(),
            " terminate".dim(),
            "  ".into(),
            key_hint::plain(KeyCode::Esc).into(),
            " close".dim(),
        ])
    }

    fn render_list(
        &self,
        area: Rect,
        buf: &mut Buffer,
        items: &[BackgroundTerminalListItem],
        selected_idx: Option<usize>,
    ) {
        let rows: Vec<GenericDisplayRow> = items
            .iter()
            .enumerate()
            .map(|(idx, item)| {
                let prefix = if selected_idx == Some(idx) { '>' } else { ' ' };
                let label = if item.command_display.is_empty() {
                    format!("{prefix} [{}]", item.process_id)
                } else {
                    format!("{prefix} [{}] {}", item.process_id, item.command_display)
                };
                let status = item.status.description();
                GenericDisplayRow {
                    name: label,
                    description: Some(status.to_string()),
                    selected_description: Some(format!("{status} · x to terminate")),
                    ..Default::default()
                }
            })
            .collect();
        let mut state = self.state;
        state.selected_idx = selected_idx;
        render_rows(
            area,
            buf,
            &rows,
            &state,
            MAX_LIST_ROWS,
            "No background terminals",
        );
    }

    fn render_output(
        &self,
        area: Rect,
        buf: &mut Buffer,
        item: Option<BackgroundTerminalSnapshot>,
    ) {
        if area.is_empty() {
            return;
        }

        let Some(item) = item else {
            Paragraph::new(vec![Line::from("No output yet".dim())]).render(area, buf);
            return;
        };

        let mut lines = Vec::new();
        let status = item.status.description();
        let header = if item.command_display.is_empty() {
            format!("Process {} ({status})", item.process_id)
        } else {
            format!("{} ({status})", item.command_display)
        };
        lines.push(Line::from(header.bold()));
        lines.push(Line::from(format!("cwd: {}", item.cwd.display()).dim()));
        lines.push(Line::from(""));

        let width = area.width.max(1) as usize;
        let mut output_lines: Vec<Line<'static>> = Vec::new();
        for raw in item.output_lines {
            if raw.is_empty() {
                output_lines.push(Line::from(""));
                continue;
            }
            for wrapped in textwrap::wrap(&raw, width) {
                output_lines.push(Line::from(wrapped.into_owned()));
            }
        }
        if output_lines.is_empty() {
            output_lines.push(Line::from("No output yet".dim()));
        }

        let mut combined: Vec<Line<'static>> = Vec::new();
        combined.extend(lines);
        combined.extend(output_lines);

        let visible = area.height as usize;
        let rendered = if combined.len() > visible {
            combined.split_off(combined.len().saturating_sub(visible))
        } else {
            combined
        };
        Paragraph::new(rendered).render(area, buf);
    }
}

impl BottomPaneView for BackgroundTerminalsView {
    fn handle_key_event(&mut self, key_event: KeyEvent) {
        let items = self.snapshot_items();
        self.sync_selection(&items);
        match key_event {
            KeyEvent {
                code: KeyCode::Up, ..
            }
            | KeyEvent {
                code: KeyCode::Char('k'),
                modifiers: KeyModifiers::CONTROL,
                ..
            }
            | KeyEvent {
                code: KeyCode::Char('p'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => self.move_selection(-1, &items),
            KeyEvent {
                code: KeyCode::Down,
                ..
            }
            | KeyEvent {
                code: KeyCode::Char('n'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => self.move_selection(1, &items),
            KeyEvent {
                code: KeyCode::Char('j'),
                modifiers: KeyModifiers::NONE,
                ..
            } => self.move_selection(1, &items),
            KeyEvent {
                code: KeyCode::Char('k'),
                modifiers: KeyModifiers::NONE,
                ..
            } => self.move_selection(-1, &items),
            KeyEvent {
                code: KeyCode::Char('x'),
                modifiers: KeyModifiers::NONE,
                ..
            } => self.terminate_selected(&items),
            KeyEvent {
                code: KeyCode::Esc, ..
            } => {
                self.on_ctrl_c();
            }
            _ => {}
        }
    }

    fn is_complete(&self) -> bool {
        self.complete
    }

    fn on_ctrl_c(&mut self) -> CancellationEvent {
        self.complete = true;
        CancellationEvent::Handled
    }
}

impl Renderable for BackgroundTerminalsView {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() {
            return;
        }

        let items = self.snapshot_items();
        let running_count = items.iter().filter(|item| item.status.is_running()).count();
        let selected_idx = self
            .state
            .selected_idx
            .filter(|idx| *idx < items.len())
            .or_else(|| (!items.is_empty()).then_some(0));

        let content_area = area.inset(Insets::vh(1, 2));
        if content_area.is_empty() {
            return;
        }

        let header_lines = self.header_lines(content_area.width, running_count);
        let header_height = header_lines.len() as u16;
        let footer_height = if content_area.height > header_height + 2 {
            1
        } else {
            0
        };
        let content_height = content_area
            .height
            .saturating_sub(header_height + footer_height);

        let [header_area, main_area, footer_area] = Layout::vertical([
            Constraint::Length(header_height),
            Constraint::Length(content_height),
            Constraint::Length(footer_height),
        ])
        .areas(content_area);

        if header_area.height > 0 {
            Paragraph::new(header_lines).render(header_area, buf);
        }

        if main_area.height > 0 {
            let columns =
                Layout::horizontal([Constraint::Percentage(35), Constraint::Percentage(65)])
                    .areas(main_area);

            let [list_area, output_area] = columns;
            self.render_list(list_area, buf, &items, selected_idx);
            let selected = selected_idx
                .and_then(|idx| items.get(idx))
                .and_then(|item| self.snapshot_selected(&item.process_id));
            self.render_output(output_area, buf, selected);
        }

        if footer_area.height > 0 {
            let hint_area = Rect {
                x: footer_area.x,
                y: footer_area.y,
                width: footer_area.width,
                height: footer_area.height,
            };
            Self::footer_line().render(hint_area, buf);
        }
    }

    fn desired_height(&self, _width: u16) -> u16 {
        8
    }
}
