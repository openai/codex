//! Interactive full-screen presentation for completed review story snapshots.

use std::collections::HashSet;
use std::io::Result;

use codex_app_server_protocol::ReviewStorySnapshot;
use codex_app_server_protocol::ReviewStorySnapshotStatus;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;
use ratatui::buffer::Buffer;
use ratatui::layout::Constraint;
use ratatui::layout::Direction;
use ratatui::layout::Layout;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Block;
use ratatui::widgets::Borders;
use ratatui::widgets::Clear;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;

use crate::key_hint::KeyBindingListExt;
use crate::keymap::PagerKeymap;
use crate::tui;
use crate::tui::TuiEvent;

mod render_helpers;

use self::render_helpers::adjust_scroll;
use self::render_helpers::centered_rect;
use self::render_helpers::clamped_scroll;
use self::render_helpers::diff_lines;
use self::render_helpers::help_lines;
use self::render_helpers::pane_block;
use self::render_helpers::primary_label;
use self::render_helpers::source_label;
use self::render_helpers::step_state_label;

const NARROW_WIDTH: u16 = 72;
const NARROW_HEIGHT: u16 = 18;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Pane {
    Steps,
    Details,
    Diff,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Subview {
    Overview,
    Help,
    Contents,
}

pub(crate) struct ReviewStoryOverlay {
    snapshot: ReviewStorySnapshot,
    selected_step: usize,
    visited_steps: HashSet<String>,
    focused_pane: Pane,
    details_scroll: u16,
    diff_scroll: u16,
    active_anchor: usize,
    subview: Option<Subview>,
    keymap: PagerKeymap,
    layout_is_narrow: bool,
    is_done: bool,
}

impl ReviewStoryOverlay {
    pub(crate) fn new(snapshot: ReviewStorySnapshot, keymap: PagerKeymap) -> Self {
        let visited_steps = snapshot
            .steps
            .first()
            .map(|step| HashSet::from([step.step_id.clone()]))
            .unwrap_or_default();
        Self {
            snapshot,
            selected_step: 0,
            visited_steps,
            focused_pane: Pane::Steps,
            details_scroll: 0,
            diff_scroll: 0,
            active_anchor: 0,
            subview: None,
            keymap,
            layout_is_narrow: false,
            is_done: false,
        }
    }

    pub(crate) fn handle_event(&mut self, tui: &mut tui::Tui, event: TuiEvent) -> Result<()> {
        match event {
            TuiEvent::Key(key) => {
                self.handle_key_event(key);
                tui.frame_requester().schedule_frame();
                Ok(())
            }
            TuiEvent::Draw | TuiEvent::Resize => tui.draw(u16::MAX, |frame| {
                self.render(frame.area(), frame.buffer);
            }),
            _ => Ok(()),
        }
    }

    pub(crate) fn is_done(&self) -> bool {
        self.is_done
    }

    fn handle_key_event(&mut self, key: KeyEvent) {
        if !matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) {
            return;
        }
        if self.handle_subview_key(key) {
            return;
        }
        if self.keymap.close.is_pressed(key) || key.code == KeyCode::Esc {
            self.is_done = true;
            return;
        }
        match key.code {
            KeyCode::Char('n') if key.modifiers == KeyModifiers::NONE => {
                self.move_step(/*amount*/ 1);
            }
            KeyCode::Char('p') if key.modifiers == KeyModifiers::NONE => {
                self.move_step(/*amount*/ -1);
            }
            KeyCode::Tab if key.modifiers == KeyModifiers::NONE => self.next_pane(),
            KeyCode::BackTab => self.previous_pane(),
            KeyCode::Enter if self.focused_pane == Pane::Steps => self.focused_pane = Pane::Diff,
            KeyCode::Char('[') if self.focused_pane == Pane::Diff => {
                self.move_anchor(/*amount*/ -1);
            }
            KeyCode::Char(']') if self.focused_pane == Pane::Diff => {
                self.move_anchor(/*amount*/ 1);
            }
            KeyCode::Char('o') if key.modifiers == KeyModifiers::NONE => {
                self.subview = Some(Subview::Overview);
            }
            KeyCode::Char('?') if key.modifiers == KeyModifiers::NONE => {
                self.subview = Some(Subview::Help);
            }
            KeyCode::Char('t') if key.modifiers == KeyModifiers::NONE => {
                self.subview = Some(Subview::Contents);
            }
            _ => self.handle_pane_navigation(key),
        }
    }

    fn handle_subview_key(&mut self, key: KeyEvent) -> bool {
        let Some(subview) = self.subview else {
            return false;
        };
        if key.code == KeyCode::Esc
            || matches!(key.code, KeyCode::Char('o')) && subview == Subview::Overview
            || matches!(key.code, KeyCode::Char('?')) && subview == Subview::Help
            || matches!(key.code, KeyCode::Char('t')) && subview == Subview::Contents
        {
            self.subview = None;
            return true;
        }
        if subview == Subview::Contents {
            if self.keymap.scroll_up.is_pressed(key) {
                self.move_step(/*amount*/ -1);
            } else if self.keymap.scroll_down.is_pressed(key) {
                self.move_step(/*amount*/ 1);
            } else if key.code == KeyCode::Enter {
                self.subview = None;
                self.focused_pane = Pane::Details;
            }
        }
        true
    }

    fn handle_pane_navigation(&mut self, key: KeyEvent) {
        if self.keymap.scroll_up.is_pressed(key) {
            self.scroll_or_select(/*amount*/ -1);
        } else if self.keymap.scroll_down.is_pressed(key) {
            self.scroll_or_select(/*amount*/ 1);
        } else if self.keymap.page_up.is_pressed(key) || self.keymap.half_page_up.is_pressed(key) {
            self.scroll_or_select(/*amount*/ -8);
        } else if self.keymap.page_down.is_pressed(key)
            || self.keymap.half_page_down.is_pressed(key)
        {
            self.scroll_or_select(/*amount*/ 8);
        } else if self.keymap.jump_top.is_pressed(key) {
            self.jump(/*bottom*/ false);
        } else if self.keymap.jump_bottom.is_pressed(key) {
            self.jump(/*bottom*/ true);
        }
    }

    fn scroll_or_select(&mut self, amount: i32) {
        match self.focused_pane {
            Pane::Steps => self.move_step(amount),
            Pane::Details => adjust_scroll(&mut self.details_scroll, amount),
            Pane::Diff => adjust_scroll(&mut self.diff_scroll, amount),
        }
    }

    fn jump(&mut self, bottom: bool) {
        match self.focused_pane {
            Pane::Steps => {
                let selected = if bottom {
                    self.snapshot.steps.len().saturating_sub(1)
                } else {
                    0
                };
                self.select_step(selected);
            }
            Pane::Details => self.details_scroll = if bottom { u16::MAX } else { 0 },
            Pane::Diff => self.diff_scroll = if bottom { u16::MAX } else { 0 },
        }
    }

    fn move_step(&mut self, amount: i32) {
        if self.snapshot.steps.is_empty() {
            return;
        }
        let selected = (self.selected_step as i32 + amount)
            .clamp(0, self.snapshot.steps.len().saturating_sub(1) as i32)
            as usize;
        self.select_step(selected);
    }

    fn select_step(&mut self, selected: usize) {
        if selected == self.selected_step || selected >= self.snapshot.steps.len() {
            return;
        }
        self.selected_step = selected;
        self.details_scroll = 0;
        self.diff_scroll = 0;
        self.active_anchor = 0;
        self.visited_steps
            .insert(self.snapshot.steps[selected].step_id.clone());
    }

    fn move_anchor(&mut self, amount: i32) {
        let anchors = self.selected_anchors();
        if anchors.is_empty() {
            return;
        }
        self.active_anchor = (self.active_anchor as i32 + amount)
            .clamp(0, anchors.len().saturating_sub(1) as i32) as usize;
        self.diff_scroll = 0;
    }

    fn next_pane(&mut self) {
        if self.layout_is_narrow {
            self.focused_pane = match self.focused_pane {
                Pane::Details => Pane::Diff,
                Pane::Diff | Pane::Steps => Pane::Details,
            };
            return;
        }
        self.focused_pane = match self.focused_pane {
            Pane::Steps => Pane::Details,
            Pane::Details => Pane::Diff,
            Pane::Diff => Pane::Steps,
        };
    }

    fn previous_pane(&mut self) {
        if self.layout_is_narrow {
            self.focused_pane = match self.focused_pane {
                Pane::Details => Pane::Diff,
                Pane::Diff | Pane::Steps => Pane::Details,
            };
            return;
        }
        self.focused_pane = match self.focused_pane {
            Pane::Steps => Pane::Diff,
            Pane::Details => Pane::Steps,
            Pane::Diff => Pane::Details,
        };
    }

    fn is_narrow(area: Rect) -> bool {
        area.width < NARROW_WIDTH || area.height < NARROW_HEIGHT
    }

    pub(crate) fn render(&mut self, area: Rect, buf: &mut Buffer) {
        self.layout_is_narrow = Self::is_narrow(area);
        if self.layout_is_narrow && self.focused_pane == Pane::Steps {
            self.focused_pane = Pane::Details;
        }
        Clear.render(area, buf);
        let shell = Block::default()
            .borders(Borders::ALL)
            .title(" REVIEW STORY ");
        let inner = shell.inner(area);
        shell.render(area, buf);
        let vertical = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2),
                Constraint::Min(4),
                Constraint::Length(1),
            ])
            .split(inner);
        self.render_header(vertical[0], buf);
        if self.layout_is_narrow {
            self.render_narrow_body(vertical[1], buf);
        } else {
            self.render_full_body(vertical[1], buf);
        }
        self.render_footer(vertical[2], buf, self.layout_is_narrow);
        if let Some(subview) = self.subview {
            self.render_subview(area, buf, subview);
        }
    }

    fn render_header(&self, area: Rect, buf: &mut Buffer) {
        let status = match self.snapshot.status {
            ReviewStorySnapshotStatus::Ready => "ready".green(),
            ReviewStorySnapshotStatus::Partial => "partial".magenta(),
            ReviewStorySnapshotStatus::Building => "building".cyan(),
            ReviewStorySnapshotStatus::Failed => "failed".red(),
        };
        let position = format!(
            "{}/{}",
            self.snapshot
                .steps
                .get(self.selected_step)
                .map_or(0, |_| self.selected_step + 1),
            self.snapshot.steps.len()
        );
        let mut metadata = vec![
            source_label(&self.snapshot.target).dim(),
            "  ".into(),
            format!("{} files", self.snapshot.anchors.len()).dim(),
            "  ".into(),
            status,
            "  ".into(),
            position.bold(),
        ];
        if self.snapshot.stale {
            metadata.extend(["  ".into(), "stale".magenta()]);
        }
        Paragraph::new(vec![
            self.snapshot.title.clone().bold().into(),
            metadata.into(),
        ])
        .render(area, buf);
    }

    fn render_full_body(&mut self, area: Rect, buf: &mut Buffer) {
        let columns = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length((area.width / 4).clamp(18, 32)),
                Constraint::Min(30),
            ])
            .split(area);
        self.render_steps(columns[0], buf);
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(43), Constraint::Percentage(57)])
            .split(columns[1]);
        self.render_details(rows[0], buf);
        self.render_diff(rows[1], buf);
    }

    fn render_narrow_body(&mut self, area: Rect, buf: &mut Buffer) {
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(42), Constraint::Percentage(58)])
            .split(area);
        self.render_details(rows[0], buf);
        self.render_diff(rows[1], buf);
    }

    fn render_steps(&self, area: Rect, buf: &mut Buffer) {
        let block = pane_block(" STEPS ", self.focused_pane == Pane::Steps);
        let inner = block.inner(area);
        block.render(area, buf);
        let lines = self
            .snapshot
            .steps
            .iter()
            .enumerate()
            .map(|(index, step)| {
                let marker = if index == self.selected_step {
                    ">".cyan().bold()
                } else if self.visited_steps.contains(&step.step_id) {
                    "x".green()
                } else {
                    ".".dim()
                };
                let state = step_state_label(step);
                vec![
                    marker,
                    format!(" {} ", step.index).dim(),
                    step.title.clone().into(),
                    state,
                ]
                .into()
            })
            .collect::<Vec<Line<'static>>>();
        Paragraph::new(lines).render(inner, buf);
    }

    fn render_details(&self, area: Rect, buf: &mut Buffer) {
        let title = self
            .selected_step()
            .map(|step| format!(" DETAILS - {} ", step.title))
            .unwrap_or_else(|| " DETAILS ".to_string());
        let block = pane_block(&title, self.focused_pane == Pane::Details);
        let inner = block.inner(area);
        block.render(area, buf);
        let lines = self.details_lines(inner.width.max(1));
        let offset = clamped_scroll(self.details_scroll, lines.len(), inner.height);
        Paragraph::new(lines).scroll((offset, 0)).render(inner, buf);
    }

    fn render_diff(&mut self, area: Rect, buf: &mut Buffer) {
        let anchors = self.selected_anchors();
        let count = anchors.len();
        let position = if count == 0 {
            0
        } else {
            self.active_anchor.min(count - 1) + 1
        };
        let title = format!(" DIFF - file {position}/{count} ");
        let block = pane_block(&title, self.focused_pane == Pane::Diff);
        let inner = block.inner(area);
        block.render(area, buf);
        let (lines, offsets) = diff_lines(&anchors, inner.width.max(1));
        if self.active_anchor < offsets.len() && self.diff_scroll == 0 && self.active_anchor > 0 {
            self.diff_scroll = offsets[self.active_anchor] as u16;
        }
        let offset = clamped_scroll(self.diff_scroll, lines.len(), inner.height);
        Paragraph::new(lines).scroll((offset, 0)).render(inner, buf);
    }

    fn render_footer(&self, area: Rect, buf: &mut Buffer, narrow: bool) {
        let focus = match self.focused_pane {
            Pane::Steps => "STEPS",
            Pane::Details => "DETAILS",
            Pane::Diff => "DIFF",
        };
        let close = primary_label(&self.keymap.close, "esc");
        let scroll = format!(
            "{}/{}",
            primary_label(&self.keymap.scroll_up, "up"),
            primary_label(&self.keymap.scroll_down, "down")
        );
        let text = if narrow {
            format!(" {focus}  n/p step  {scroll} scroll  tab pane  t contents  {close} close")
        } else {
            format!(
                " {focus}  n/p step  {scroll} scroll  tab pane  [/] file  o overview  ? help  {close} close"
            )
        };
        Paragraph::new(text.dim()).render(area, buf);
    }

    fn render_subview(&self, area: Rect, buf: &mut Buffer, subview: Subview) {
        let popup = centered_rect(area, /*width_percent*/ 72, /*height_percent*/ 70);
        Clear.render(popup, buf);
        let title = match subview {
            Subview::Overview => " OVERVIEW ",
            Subview::Help => " HELP ",
            Subview::Contents => " CONTENTS ",
        };
        let block = Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(Style::default().cyan());
        let inner = block.inner(popup);
        block.render(popup, buf);
        let lines = match subview {
            Subview::Overview => self.overview_lines(inner.width.max(1)),
            Subview::Help => help_lines(),
            Subview::Contents => self.contents_lines(),
        };
        Paragraph::new(lines).render(inner, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_app_server_protocol::ReviewStoryAnchor;
    use codex_app_server_protocol::ReviewStoryAnchorKind;
    use codex_app_server_protocol::ReviewStoryStep;
    use codex_app_server_protocol::ReviewStoryStepReadiness;
    use codex_app_server_protocol::ReviewTarget;
    use insta::assert_snapshot;
    use pretty_assertions::assert_eq;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn default_keymap() -> PagerKeymap {
        crate::keymap::RuntimeKeymap::defaults().pager
    }

    fn snapshot() -> ReviewStorySnapshot {
        ReviewStorySnapshot {
            story_snapshot_id: "story-1".to_string(),
            thread_id: "thread-1".to_string(),
            title: "Introduce review story navigation".to_string(),
            overview: "A stored narrative orders the API and terminal changes for review."
                .to_string(),
            target: ReviewTarget::BaseBranch {
                branch: "main".to_string(),
            },
            source_fingerprint: "source-1234".to_string(),
            status: ReviewStorySnapshotStatus::Ready,
            created_at: 1,
            updated_at: 2,
            previous_story_snapshot_id: None,
            stale: false,
            steps: vec![
                ReviewStoryStep {
                    step_id: "api".to_string(),
                    index: 1,
                    title: "Define the artifact".to_string(),
                    goal: "Expose a reusable snapshot contract.".to_string(),
                    summary: "Adds the v2 request and response types.".to_string(),
                    dependency_rationale: "The interface precedes any presentation layer."
                        .to_string(),
                    anchor_ids: vec!["protocol".to_string()],
                    review_focus: vec!["Confirm the wire format can support both clients."
                        .to_string()],
                    readiness: ReviewStoryStepReadiness::Ready,
                    error: None,
                },
                ReviewStoryStep {
                    step_id: "tui".to_string(),
                    index: 2,
                    title: "Navigate the story".to_string(),
                    goal: "Let a reviewer move step by step.".to_string(),
                    summary: "Adds a focused cockpit with diff evidence.".to_string(),
                    dependency_rationale: "The surface consumes the completed contract."
                        .to_string(),
                    anchor_ids: vec!["overlay".to_string()],
                    review_focus: vec!["Check compact terminal behavior.".to_string()],
                    readiness: ReviewStoryStepReadiness::Ready,
                    error: None,
                },
            ],
            anchors: vec![
                ReviewStoryAnchor {
                    anchor_id: "protocol".to_string(),
                    file_path: "app-server-protocol/src/review_story.rs".to_string(),
                    change_kind: ReviewStoryAnchorKind::Added,
                    summary: "Adds shared story payloads.".to_string(),
                    diff: "diff --git a/protocol.rs b/protocol.rs\n--- a/protocol.rs\n+++ b/protocol.rs\n@@ -1 +1,2 @@\n pub struct Thread;\n+pub struct Story;\n".to_string(),
                },
                ReviewStoryAnchor {
                    anchor_id: "overlay".to_string(),
                    file_path: "tui/src/review_story_overlay.rs".to_string(),
                    change_kind: ReviewStoryAnchorKind::Added,
                    summary: "Adds the story cockpit.".to_string(),
                    diff: "diff --git a/overlay.rs b/overlay.rs\n--- /dev/null\n+++ b/overlay.rs\n@@ -0,0 +1 @@\n+struct ReviewStoryOverlay;\n".to_string(),
                },
            ],
        }
    }

    fn render(overlay: &mut ReviewStoryOverlay, width: u16, height: u16) -> TestBackend {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("terminal");
        terminal
            .draw(|frame| overlay.render(frame.area(), frame.buffer_mut()))
            .expect("draw");
        terminal.backend().clone()
    }

    #[test]
    fn full_cockpit_snapshot() {
        let mut overlay = ReviewStoryOverlay::new(snapshot(), default_keymap());

        assert_snapshot!(render(&mut overlay, /*width*/ 116, /*height*/ 30));
    }

    #[test]
    fn compact_cockpit_snapshot_hides_steps_rail() {
        let mut overlay = ReviewStoryOverlay::new(snapshot(), default_keymap());

        assert_snapshot!(render(&mut overlay, /*width*/ 68, /*height*/ 22));
        assert_eq!(overlay.focused_pane, Pane::Details);
    }

    #[test]
    fn overview_subview_snapshot() {
        let mut overlay = ReviewStoryOverlay::new(snapshot(), default_keymap());
        overlay.subview = Some(Subview::Overview);

        assert_snapshot!(render(&mut overlay, /*width*/ 116, /*height*/ 30));
    }

    #[test]
    fn navigation_resets_step_scroll_and_tracks_visited_steps() {
        let mut overlay = ReviewStoryOverlay::new(snapshot(), default_keymap());
        overlay.details_scroll = 4;
        overlay.diff_scroll = 3;
        overlay.active_anchor = 1;

        overlay.handle_key_event(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE));

        assert_eq!(overlay.selected_step, 1);
        assert_eq!(overlay.details_scroll, 0);
        assert_eq!(overlay.diff_scroll, 0);
        assert_eq!(overlay.active_anchor, 0);
        assert!(overlay.visited_steps.contains("tui"));
    }

    #[test]
    fn escape_closes_a_subview_before_closing_the_story() {
        let mut overlay = ReviewStoryOverlay::new(snapshot(), default_keymap());
        overlay.subview = Some(Subview::Help);

        overlay.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert_eq!(overlay.subview, None);
        assert!(!overlay.is_done);

        overlay.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert!(overlay.is_done);
    }
}
