use std::path::Path;

use crate::key_hint;
use crate::render::Insets;
use crate::render::renderable::ColumnRenderable;
use crate::render::renderable::Renderable;
use crate::render::renderable::RenderableExt as _;
use crate::selection_list::selection_option_row;
use crate::tui::FrameRequester;
use crate::tui::Tui;
use crate::tui::TuiEvent;
use color_eyre::Result;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::prelude::Widget;
use ratatui::style::Stylize as _;
use ratatui::text::Line;
use ratatui::widgets::Clear;
use ratatui::widgets::WidgetRef;
use tokio_stream::StreamExt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CwdPromptAction {
    Resume,
    Fork,
}

impl CwdPromptAction {
    fn verb(self) -> &'static str {
        match self {
            CwdPromptAction::Resume => "resume",
            CwdPromptAction::Fork => "fork",
        }
    }

    fn past_participle(self) -> &'static str {
        match self {
            CwdPromptAction::Resume => "resumed",
            CwdPromptAction::Fork => "forked",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CwdSelection {
    Current,
    Session,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CwdPromptOutcome {
    Selection(CwdSelection),
    Exit,
}

impl CwdSelection {
    fn next(self) -> Self {
        match self {
            CwdSelection::Current => CwdSelection::Session,
            CwdSelection::Session => CwdSelection::Current,
        }
    }

    fn prev(self) -> Self {
        match self {
            CwdSelection::Current => CwdSelection::Session,
            CwdSelection::Session => CwdSelection::Current,
        }
    }
}

pub(crate) async fn run_cwd_selection_prompt(
    tui: &mut Tui,
    action: CwdPromptAction,
    current_cwd: &Path,
    session_cwd: &Path,
) -> Result<CwdPromptOutcome> {
    let mut screen = CwdPromptScreen::new(
        tui.frame_requester(),
        action,
        current_cwd.display().to_string(),
        session_cwd.display().to_string(),
    );
    tui.draw(u16::MAX, |frame| {
        frame.render_widget_ref(&screen, frame.area());
    })?;

    let events = tui.event_stream();
    tokio::pin!(events);

    while !screen.is_done() {
        if let Some(event) = events.next().await {
            match event {
                TuiEvent::Key(key_event) => screen.handle_key(key_event),
                TuiEvent::Paste(_) => {}
                TuiEvent::Draw => {
                    tui.draw(u16::MAX, |frame| {
                        frame.render_widget_ref(&screen, frame.area());
                    })?;
                }
            }
        } else {
            break;
        }
    }

    if screen.should_exit {
        Ok(CwdPromptOutcome::Exit)
    } else {
        Ok(CwdPromptOutcome::Selection(
            screen.selection().unwrap_or(CwdSelection::Session),
        ))
    }
}

struct CwdPromptScreen {
    request_frame: FrameRequester,
    action: CwdPromptAction,
    current_cwd: String,
    session_cwd: String,
    highlighted: CwdSelection,
    selection: Option<CwdSelection>,
    should_exit: bool,
}

impl CwdPromptScreen {
    fn new(
        request_frame: FrameRequester,
        action: CwdPromptAction,
        current_cwd: String,
        session_cwd: String,
    ) -> Self {
        Self {
            request_frame,
            action,
            current_cwd,
            session_cwd,
            highlighted: CwdSelection::Session,
            selection: None,
            should_exit: false,
        }
    }

    fn handle_key(&mut self, key_event: KeyEvent) {
        if key_event.kind == KeyEventKind::Release {
            return;
        }
        if key_event.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key_event.code, KeyCode::Char('c') | KeyCode::Char('d'))
        {
            self.selection = None;
            self.should_exit = true;
            self.request_frame.schedule_frame();
            return;
        }
        match key_event.code {
            KeyCode::Up | KeyCode::Char('k') => self.set_highlight(self.highlighted.prev()),
            KeyCode::Down | KeyCode::Char('j') => self.set_highlight(self.highlighted.next()),
            KeyCode::Char('1') => self.select(CwdSelection::Session),
            KeyCode::Char('2') => self.select(CwdSelection::Current),
            KeyCode::Enter => self.select(self.highlighted),
            KeyCode::Esc => self.select(CwdSelection::Session),
            _ => {}
        }
    }

    fn set_highlight(&mut self, highlight: CwdSelection) {
        if self.highlighted != highlight {
            self.highlighted = highlight;
            self.request_frame.schedule_frame();
        }
    }

    fn select(&mut self, selection: CwdSelection) {
        self.highlighted = selection;
        self.selection = Some(selection);
        self.request_frame.schedule_frame();
    }

    fn is_done(&self) -> bool {
        self.should_exit || self.selection.is_some()
    }

    fn selection(&self) -> Option<CwdSelection> {
        self.selection
    }
}

impl WidgetRef for &CwdPromptScreen {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        Clear.render(area, buf);
        let mut column = ColumnRenderable::new();

        let action_verb = self.action.verb();
        let action_past = self.action.past_participle();
        let current_cwd = self.current_cwd.as_str();
        let session_cwd = self.session_cwd.as_str();

        column.push("");
        column.push(Line::from(vec![
            "Choose working directory to ".into(),
            action_verb.bold(),
            " this session".into(),
        ]));
        column.push("");
        column.push(
            Line::from(format!(
                "Session = latest cwd recorded in the {action_past} session"
            ))
            .dim()
            .inset(Insets::tlbr(0, 2, 0, 0)),
        );
        column.push(
            Line::from("Current = your current working directory".dim())
                .inset(Insets::tlbr(0, 2, 0, 0)),
        );
        column.push("");
        column.push(selection_option_row(
            0,
            format!("Use session directory ({session_cwd})"),
            self.highlighted == CwdSelection::Session,
        ));
        column.push(selection_option_row(
            1,
            format!("Use current directory ({current_cwd})"),
            self.highlighted == CwdSelection::Current,
        ));
        column.push("");
        column.push(
            Line::from(vec![
                "Press ".dim(),
                key_hint::plain(KeyCode::Enter).into(),
                " to continue".dim(),
            ])
            .inset(Insets::tlbr(0, 2, 0, 0)),
        );
        column.render(area, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_backend::VT100Backend;
    use crossterm::event::KeyEvent;
    use crossterm::event::KeyModifiers;
    use pretty_assertions::assert_eq;
    use ratatui::Terminal;

    fn new_prompt() -> CwdPromptScreen {
        CwdPromptScreen::new(
            FrameRequester::test_dummy(),
            CwdPromptAction::Resume,
            "/Users/example/current".to_string(),
            "/Users/example/session".to_string(),
        )
    }

    #[test]
    fn cwd_prompt_snapshot() {
        let screen = new_prompt();
        let mut terminal = Terminal::new(VT100Backend::new(80, 14)).expect("terminal");
        terminal
            .draw(|frame| frame.render_widget_ref(&screen, frame.area()))
            .expect("render cwd prompt");
        insta::assert_snapshot!("cwd_prompt_modal", terminal.backend());
    }

    #[test]
    fn cwd_prompt_fork_snapshot() {
        let screen = CwdPromptScreen::new(
            FrameRequester::test_dummy(),
            CwdPromptAction::Fork,
            "/Users/example/current".to_string(),
            "/Users/example/session".to_string(),
        );
        let mut terminal = Terminal::new(VT100Backend::new(80, 14)).expect("terminal");
        terminal
            .draw(|frame| frame.render_widget_ref(&screen, frame.area()))
            .expect("render cwd prompt");
        insta::assert_snapshot!("cwd_prompt_fork_modal", terminal.backend());
    }

    #[test]
    fn cwd_prompt_selects_session_by_default() {
        let mut screen = new_prompt();
        screen.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert_eq!(screen.selection(), Some(CwdSelection::Session));
    }

    #[test]
    fn cwd_prompt_can_select_current() {
        let mut screen = new_prompt();
        screen.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        screen.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert_eq!(screen.selection(), Some(CwdSelection::Current));
    }

    #[test]
    fn cwd_prompt_ctrl_c_exits_instead_of_selecting() {
        let mut screen = new_prompt();
        screen.handle_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
        assert_eq!(screen.selection(), None);
        assert!(screen.is_done());
    }
}
