use crate::cwd_prompt::CwdPromptAction;
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
pub(crate) enum GitBranchSelection {
    Current,
    Session,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GitBranchPromptOutcome {
    Selection(GitBranchSelection),
    Exit,
}

impl GitBranchSelection {
    fn next(self) -> Self {
        match self {
            GitBranchSelection::Current => GitBranchSelection::Session,
            GitBranchSelection::Session => GitBranchSelection::Current,
        }
    }

    fn prev(self) -> Self {
        match self {
            GitBranchSelection::Current => GitBranchSelection::Session,
            GitBranchSelection::Session => GitBranchSelection::Current,
        }
    }
}

pub(crate) async fn run_git_branch_selection_prompt(
    tui: &mut Tui,
    action: CwdPromptAction,
    current_branch: &str,
    session_branch: &str,
) -> Result<GitBranchPromptOutcome> {
    let mut screen = GitBranchPromptScreen::new(
        tui.frame_requester(),
        action,
        current_branch.to_string(),
        session_branch.to_string(),
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
        Ok(GitBranchPromptOutcome::Exit)
    } else {
        Ok(GitBranchPromptOutcome::Selection(
            screen.selection().unwrap_or(GitBranchSelection::Session),
        ))
    }
}

struct GitBranchPromptScreen {
    request_frame: FrameRequester,
    action: CwdPromptAction,
    current_branch: String,
    session_branch: String,
    highlighted: GitBranchSelection,
    selection: Option<GitBranchSelection>,
    should_exit: bool,
}

impl GitBranchPromptScreen {
    fn new(
        request_frame: FrameRequester,
        action: CwdPromptAction,
        current_branch: String,
        session_branch: String,
    ) -> Self {
        Self {
            request_frame,
            action,
            current_branch,
            session_branch,
            highlighted: GitBranchSelection::Session,
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
            KeyCode::Char('1') => self.select(GitBranchSelection::Session),
            KeyCode::Char('2') => self.select(GitBranchSelection::Current),
            KeyCode::Enter => self.select(self.highlighted),
            KeyCode::Esc => self.select(GitBranchSelection::Current),
            _ => {}
        }
    }

    fn set_highlight(&mut self, highlight: GitBranchSelection) {
        if self.highlighted != highlight {
            self.highlighted = highlight;
            self.request_frame.schedule_frame();
        }
    }

    fn select(&mut self, selection: GitBranchSelection) {
        self.highlighted = selection;
        self.selection = Some(selection);
        self.request_frame.schedule_frame();
    }

    fn is_done(&self) -> bool {
        self.should_exit || self.selection.is_some()
    }

    fn selection(&self) -> Option<GitBranchSelection> {
        self.selection
    }
}

impl WidgetRef for &GitBranchPromptScreen {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        Clear.render(area, buf);
        let mut column = ColumnRenderable::new();

        let action_verb = self.action.verb();
        let action_past = self.action.past_participle();
        let current_branch = self.current_branch.as_str();
        let session_branch = self.session_branch.as_str();

        column.push("");
        column.push(Line::from(vec![
            "Choose git branch to ".into(),
            action_verb.bold(),
            " this session".into(),
        ]));
        column.push("");
        column.push(
            Line::from(format!(
                "Session = git branch recorded in the {action_past} session"
            ))
            .dim()
            .inset(Insets::tlbr(0, 2, 0, 0)),
        );
        column.push(
            Line::from("Current = branch checked out in the selected working directory".dim())
                .inset(Insets::tlbr(0, 2, 0, 0)),
        );
        column.push("");
        column.push(selection_option_row(
            0,
            format!("Switch to session branch ({session_branch})"),
            self.highlighted == GitBranchSelection::Session,
        ));
        column.push(selection_option_row(
            1,
            format!("Continue on current branch ({current_branch})"),
            self.highlighted == GitBranchSelection::Current,
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

    fn new_prompt() -> GitBranchPromptScreen {
        GitBranchPromptScreen::new(
            FrameRequester::test_dummy(),
            CwdPromptAction::Resume,
            "main".to_string(),
            "feature/resume".to_string(),
        )
    }

    #[test]
    fn git_branch_prompt_snapshot() {
        let screen = new_prompt();
        let mut terminal = Terminal::new(VT100Backend::new(80, 14)).expect("terminal");
        terminal
            .draw(|frame| frame.render_widget_ref(&screen, frame.area()))
            .expect("render git branch prompt");
        insta::assert_snapshot!("git_branch_prompt_modal", terminal.backend());
    }

    #[test]
    fn git_branch_prompt_selects_session_by_default() {
        let mut screen = new_prompt();
        screen.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert_eq!(screen.selection(), Some(GitBranchSelection::Session));
    }

    #[test]
    fn git_branch_prompt_can_select_current() {
        let mut screen = new_prompt();
        screen.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        screen.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert_eq!(screen.selection(), Some(GitBranchSelection::Current));
    }

    #[test]
    fn git_branch_prompt_ctrl_c_exits_instead_of_selecting() {
        let mut screen = new_prompt();
        screen.handle_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
        assert_eq!(screen.selection(), None);
        assert!(screen.is_done());
    }
}
