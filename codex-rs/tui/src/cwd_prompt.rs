//! Resume/fork directory choices with shared picker presentation and caller-owned persistence.

use std::path::Path;

use crate::bottom_pane::picker_option_list;
use crate::bottom_pane::render_menu_surface;
use crate::key_hint;
use crate::legacy_core::config::Config;
use crate::legacy_core::config::edit::ConfigEditsBuilder;
use crate::local_settings::LocalSettings;
use crate::render::Insets;
use crate::render::renderable::FlexRenderable;
use crate::render::renderable::Renderable;
use crate::render::renderable::RenderableExt as _;
use crate::render::renderable::RenderableItem;
use crate::tui::FrameRequester;
use crate::tui::Tui;
use crate::tui::TuiEvent;
use codex_config::types::ResumeCwdMode;
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
use ratatui::widgets::Paragraph;
use ratatui::widgets::WidgetRef;
use ratatui::widgets::Wrap;
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
    CurrentAndRemember,
    SessionAndRemember,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CwdPromptOutcome {
    Selection(CwdSelection),
    Exit,
}

impl CwdSelection {
    fn next(self) -> Self {
        match self {
            CwdSelection::Session => CwdSelection::Current,
            CwdSelection::Current => CwdSelection::SessionAndRemember,
            CwdSelection::SessionAndRemember => CwdSelection::CurrentAndRemember,
            CwdSelection::CurrentAndRemember => CwdSelection::Session,
        }
    }

    fn prev(self) -> Self {
        match self {
            CwdSelection::Session => CwdSelection::CurrentAndRemember,
            CwdSelection::Current => CwdSelection::Session,
            CwdSelection::SessionAndRemember => CwdSelection::Current,
            CwdSelection::CurrentAndRemember => CwdSelection::SessionAndRemember,
        }
    }

    fn remembered_mode(self) -> Option<ResumeCwdMode> {
        match self {
            CwdSelection::Current | CwdSelection::Session => None,
            CwdSelection::CurrentAndRemember => Some(ResumeCwdMode::Current),
            CwdSelection::SessionAndRemember => Some(ResumeCwdMode::Session),
        }
    }

    pub(crate) fn selected_cwd<'path>(
        self,
        current_cwd: &'path Path,
        session_cwd: &'path Path,
        remembered_current_cwd: &'path Path,
    ) -> &'path Path {
        match self {
            CwdSelection::Current => current_cwd,
            CwdSelection::CurrentAndRemember => remembered_current_cwd,
            CwdSelection::Session | CwdSelection::SessionAndRemember => session_cwd,
        }
    }
}

pub(crate) async fn run_cwd_selection_prompt(
    tui: &mut Tui,
    config: &Config,
    action: CwdPromptAction,
    current_cwd: &Path,
    session_cwd: &Path,
    remembered_current_cwd: &Path,
    allow_remember_current: bool,
) -> Result<CwdPromptOutcome> {
    let mut screen = CwdPromptScreen::new(
        tui.frame_requester(),
        action,
        current_cwd.display().to_string(),
        session_cwd.display().to_string(),
        remembered_current_cwd.display().to_string(),
        allow_remember_current,
    );
    tui.draw(u16::MAX, |frame| {
        frame.render_widget_ref(&screen, frame.area());
    })?;

    tui.discard_pending_input_before_interactive_screen()?;
    let events = tui.event_stream();
    tokio::pin!(events);

    while !screen.is_done() {
        if let Some(event) = events.next().await {
            tui.screen_size_for_event(&event)?;
            match event {
                TuiEvent::Key(key_event) => screen.handle_key(key_event),
                TuiEvent::Paste(_) | TuiEvent::FocusLost => {}
                TuiEvent::Draw | TuiEvent::Resume | TuiEvent::Resize(_) | TuiEvent::FocusGained => {
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
        let selection = screen.selection().unwrap_or(CwdSelection::Session);
        if let Some(error_line) =
            persist_remembered_cwd_selection(&LocalSettings::from(config), selection).await
        {
            tui.insert_history_lines(vec![error_line]);
        }
        Ok(CwdPromptOutcome::Selection(selection))
    }
}

async fn persist_remembered_cwd_selection(
    config: &LocalSettings,
    selection: CwdSelection,
) -> Option<Line<'static>> {
    let mode = selection.remembered_mode()?;
    match ConfigEditsBuilder::for_config_path(config.user_config_path.as_path())
        .set_resume_cwd(mode)
        .apply()
        .await
    {
        Ok(()) => None,
        Err(err) => {
            tracing::error!(error = %err, "failed to persist working directory preference");
            Some(Line::from("Failed to save working directory preference.").red())
        }
    }
}

struct CwdPromptScreen {
    request_frame: FrameRequester,
    action: CwdPromptAction,
    current_cwd: String,
    session_cwd: String,
    remembered_current_cwd: String,
    highlighted: CwdSelection,
    selection: Option<CwdSelection>,
    should_exit: bool,
    allow_remember_current: bool,
}

impl CwdPromptScreen {
    fn new(
        request_frame: FrameRequester,
        action: CwdPromptAction,
        current_cwd: String,
        session_cwd: String,
        remembered_current_cwd: String,
        allow_remember_current: bool,
    ) -> Self {
        Self {
            request_frame,
            action,
            current_cwd,
            session_cwd,
            remembered_current_cwd,
            highlighted: CwdSelection::Session,
            selection: None,
            should_exit: false,
            allow_remember_current,
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
            KeyCode::Up | KeyCode::Char('k') => {
                let mut highlighted = self.highlighted.prev();
                if !self.allow_remember_current && highlighted == CwdSelection::CurrentAndRemember {
                    highlighted = highlighted.prev();
                }
                self.set_highlight(highlighted);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let mut highlighted = self.highlighted.next();
                if !self.allow_remember_current && highlighted == CwdSelection::CurrentAndRemember {
                    highlighted = highlighted.next();
                }
                self.set_highlight(highlighted);
            }
            KeyCode::Char('1') => self.select(CwdSelection::Session),
            KeyCode::Char('2') => self.select(CwdSelection::Current),
            KeyCode::Char('3') => self.select(CwdSelection::SessionAndRemember),
            KeyCode::Char('4') if self.allow_remember_current => {
                self.select(CwdSelection::CurrentAndRemember);
            }
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
        let mut column = FlexRenderable::new();

        let action_verb = self.action.verb();
        let action_past = self.action.past_participle();
        let current_cwd = self.current_cwd.as_str();
        let session_cwd = self.session_cwd.as_str();

        column.push(/*flex*/ 1, RenderableItem::Borrowed(&""));
        column.push(
            /*flex*/ 0,
            Paragraph::new(format!("Working directory · {action_verb}").bold())
                .wrap(Wrap { trim: false })
                .inset(Insets::vh(/*v*/ 0, /*h*/ 2)),
        );
        column.push(
            /*flex*/ 0,
            Paragraph::new(format!(
                "Session = latest cwd recorded in the {action_past} session\n\
                 Current = your current working directory"
            ))
            .dim()
            .wrap(Wrap { trim: false })
            .inset(Insets::vh(/*v*/ 0, /*h*/ 2)),
        );
        let mut labels = vec![
            format!("Use session directory ({session_cwd})"),
            format!("Use current directory ({current_cwd})"),
            "Always use session directory".to_string(),
        ];
        if self.allow_remember_current {
            let label = if self.remembered_current_cwd == self.current_cwd {
                "Always use current directory".to_string()
            } else {
                format!(
                    "Always use current directory ({})",
                    self.remembered_current_cwd
                )
            };
            labels.push(label);
        }
        let selected_index = match self.highlighted {
            CwdSelection::Session => 0,
            CwdSelection::Current => 1,
            CwdSelection::SessionAndRemember => 2,
            CwdSelection::CurrentAndRemember => 3,
        };
        column.push(/*flex*/ 1, picker_option_list(labels, selected_index));
        column.push(
            /*flex*/ 0,
            Paragraph::new(Line::from(vec![
                key_hint::plain(KeyCode::Enter).into(),
                " continue · ".dim(),
                key_hint::plain(KeyCode::Esc).into(),
                " use session · ".dim(),
                key_hint::ctrl(KeyCode::Char('c')).into(),
                " quit".dim(),
            ]))
            .wrap(Wrap { trim: false })
            .inset(Insets::vh(/*v*/ 0, /*h*/ 2)),
        );
        column.push(/*flex*/ 1, RenderableItem::Borrowed(&""));
        let panel = Rect {
            height: column.desired_height(area.width).min(area.height),
            ..area
        };
        render_menu_surface(panel, buf);
        column.render(panel, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::legacy_core::config::ConfigBuilder;
    use crate::test_backend::VT100Backend;
    use crossterm::event::KeyEvent;
    use crossterm::event::KeyModifiers;
    use pretty_assertions::assert_eq;
    use ratatui::Terminal;
    use ratatui::widgets::FrameExt;
    use tempfile::TempDir;

    fn new_prompt() -> CwdPromptScreen {
        CwdPromptScreen::new(
            FrameRequester::test_dummy(),
            CwdPromptAction::Resume,
            "/Users/example/current".to_string(),
            "/Users/example/session".to_string(),
            "/Users/example/current".to_string(),
            /*allow_remember_current*/ true,
        )
    }

    #[test]
    fn cwd_prompt_snapshot() {
        let screen = new_prompt();
        let mut terminal =
            Terminal::new(VT100Backend::new(/*width*/ 80, /*height*/ 14)).expect("terminal");
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
            "/Users/example/current".to_string(),
            /*allow_remember_current*/ true,
        );
        let mut terminal =
            Terminal::new(VT100Backend::new(/*width*/ 80, /*height*/ 14)).expect("terminal");
        terminal
            .draw(|frame| frame.render_widget_ref(&screen, frame.area()))
            .expect("render cwd prompt");
        insta::assert_snapshot!("cwd_prompt_fork_modal", terminal.backend());
    }

    #[test]
    fn cwd_prompt_remote_exec_snapshot() {
        let screen = CwdPromptScreen::new(
            FrameRequester::test_dummy(),
            CwdPromptAction::Resume,
            "/Users/example/current".to_string(),
            "/Users/example/session".to_string(),
            "/Users/example/current".to_string(),
            /*allow_remember_current*/ false,
        );
        let mut terminal =
            Terminal::new(VT100Backend::new(/*width*/ 80, /*height*/ 13)).expect("terminal");
        terminal
            .draw(|frame| frame.render_widget_ref(&screen, frame.area()))
            .expect("render remote exec cwd prompt");
        let rendered = terminal.backend().to_string();
        let rendered = rendered
            .lines()
            .map(str::trim_end)
            .collect::<Vec<_>>()
            .join("\n");
        insta::assert_snapshot!("cwd_prompt_remote_exec_modal", rendered);
    }

    #[test]
    fn cwd_prompt_remembered_current_snapshot() {
        let screen = CwdPromptScreen::new(
            FrameRequester::test_dummy(),
            CwdPromptAction::Resume,
            "/Users/example/current".to_string(),
            "/Users/example/session".to_string(),
            "/Users/example/launched".to_string(),
            /*allow_remember_current*/ true,
        );
        let mut terminal =
            Terminal::new(VT100Backend::new(/*width*/ 80, /*height*/ 14)).expect("terminal");
        terminal
            .draw(|frame| frame.render_widget_ref(&screen, frame.area()))
            .expect("render remembered current cwd prompt");
        let rendered = terminal.backend().to_string();
        let rendered = rendered
            .lines()
            .map(str::trim_end)
            .collect::<Vec<_>>()
            .join("\n");
        insta::assert_snapshot!("cwd_prompt_remembered_current_modal", rendered);
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
    fn narrow_picker_keeps_selected_choice_and_escape_uses_session() {
        let mut screen = new_prompt();
        screen.session_cwd = "/workspace/projects/design-system/日本語/session".to_string();
        screen.current_cwd = "/workspace/projects/design-system/Português/current".to_string();
        screen.handle_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
        let (width, height) = (40, 16);
        let mut terminal = Terminal::new(VT100Backend::new(width, height)).expect("terminal");
        terminal
            .draw(|frame| frame.render_widget_ref(&screen, frame.area()))
            .expect("render resized directory picker");
        let rendered = terminal.backend().to_string();
        assert!(rendered.contains("› 4. Always use current directory"));
        assert!(rendered.contains("esc use session"));
        assert_eq!(screen.selection(), None);
        insta::assert_snapshot!(format!("cwd_picker_selected_{width}x{height}"), rendered);
        screen.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert_eq!(screen.selection(), Some(CwdSelection::Session));
    }

    #[test]
    fn cwd_prompt_omits_unusable_remembered_current_choice() {
        let mut screen = CwdPromptScreen::new(
            FrameRequester::test_dummy(),
            CwdPromptAction::Resume,
            "/Users/example/current".to_string(),
            "/Users/example/session".to_string(),
            "/Users/example/current".to_string(),
            /*allow_remember_current*/ false,
        );

        screen.handle_key(KeyEvent::new(KeyCode::Char('4'), KeyModifiers::NONE));
        assert_eq!(screen.selection(), None);
        screen.handle_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
        assert_eq!(screen.highlighted, CwdSelection::SessionAndRemember);
    }

    #[test]
    fn cwd_prompt_ctrl_c_exits_instead_of_selecting() {
        let mut screen = new_prompt();
        screen.handle_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
        assert_eq!(screen.selection(), None);
        assert!(screen.is_done());
    }

    #[tokio::test]
    async fn cwd_prompt_remembered_choices_select_and_persist_matching_directory() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let config = ConfigBuilder::default()
            .codex_home(temp_dir.path().to_path_buf())
            .build()
            .await?;
        let current_cwd = Path::new("/Users/example/current");
        let session_cwd = Path::new("/Users/example/session");
        let remembered_current_cwd = Path::new("/Users/example/launched");

        for (key, expected_selection, expected_mode, expected_cwd) in [
            (
                '3',
                CwdSelection::SessionAndRemember,
                "session",
                session_cwd,
            ),
            (
                '4',
                CwdSelection::CurrentAndRemember,
                "current",
                remembered_current_cwd,
            ),
        ] {
            let mut screen = new_prompt();
            screen.handle_key(KeyEvent::new(KeyCode::Char(key), KeyModifiers::NONE));
            let selection = screen.selection().expect("remembered choice is selected");

            assert_eq!(selection, expected_selection);
            assert_eq!(
                selection.selected_cwd(current_cwd, session_cwd, remembered_current_cwd),
                expected_cwd
            );
            assert_eq!(
                persist_remembered_cwd_selection(&LocalSettings::from(&config), selection).await,
                None
            );
            let persisted: toml::Value = toml::from_str(&std::fs::read_to_string(
                temp_dir.path().join("config.toml"),
            )?)?;
            assert_eq!(persisted["tui"]["resume_cwd"].as_str(), Some(expected_mode));
        }

        Ok(())
    }

    #[tokio::test]
    async fn cwd_prompt_persistence_failure_snapshot() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let config = ConfigBuilder::default()
            .codex_home(temp_dir.path().to_path_buf())
            .build()
            .await?;
        let config_path = temp_dir.path().join("config.toml");
        std::fs::create_dir(&config_path)?;

        let error_line = persist_remembered_cwd_selection(
            &LocalSettings::from(&config),
            CwdSelection::CurrentAndRemember,
        )
        .await
        .expect("saving to a directory should fail");
        let mut terminal =
            Terminal::new(VT100Backend::new(/*width*/ 100, /*height*/ 1)).expect("terminal");
        terminal
            .draw(|frame| frame.render_widget(error_line, frame.area()))
            .expect("render persistence error");
        let rendered = terminal.backend().to_string();

        insta::assert_snapshot!("cwd_prompt_persistence_failure", rendered);
        Ok(())
    }
}
