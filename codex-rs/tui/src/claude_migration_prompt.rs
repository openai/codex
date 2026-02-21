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

#[derive(Clone, Debug)]
pub(crate) struct ClaudeHomeMigrationPromptData {
    pub prior_codex_thread_count: usize,
    pub imported_config_keys: Vec<String>,
    pub copied_skills: Vec<String>,
    pub copy_agents_md: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ClaudeHomeMigrationPromptOutcome {
    ImportNow,
    SkipOnce,
    Never,
}

pub(crate) async fn run_claude_home_migration_prompt(
    tui: &mut Tui,
    data: ClaudeHomeMigrationPromptData,
) -> Result<ClaudeHomeMigrationPromptOutcome> {
    let mut screen = ClaudeHomeMigrationPromptScreen::new(tui.frame_requester(), data);
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

    Ok(screen
        .selection()
        .unwrap_or(ClaudeHomeMigrationPromptOutcome::SkipOnce))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ClaudeMigrationSelection {
    ImportNow,
    SkipOnce,
    Never,
}

impl ClaudeMigrationSelection {
    fn next(self) -> Self {
        match self {
            Self::ImportNow => Self::SkipOnce,
            Self::SkipOnce => Self::Never,
            Self::Never => Self::ImportNow,
        }
    }

    fn prev(self) -> Self {
        match self {
            Self::ImportNow => Self::Never,
            Self::SkipOnce => Self::ImportNow,
            Self::Never => Self::SkipOnce,
        }
    }

    fn into_outcome(self) -> ClaudeHomeMigrationPromptOutcome {
        match self {
            Self::ImportNow => ClaudeHomeMigrationPromptOutcome::ImportNow,
            Self::SkipOnce => ClaudeHomeMigrationPromptOutcome::SkipOnce,
            Self::Never => ClaudeHomeMigrationPromptOutcome::Never,
        }
    }
}

struct ClaudeHomeMigrationPromptScreen {
    request_frame: FrameRequester,
    data: ClaudeHomeMigrationPromptData,
    highlighted: ClaudeMigrationSelection,
    selection: Option<ClaudeMigrationSelection>,
}

impl ClaudeHomeMigrationPromptScreen {
    fn new(request_frame: FrameRequester, data: ClaudeHomeMigrationPromptData) -> Self {
        Self {
            request_frame,
            data,
            highlighted: ClaudeMigrationSelection::ImportNow,
            selection: None,
        }
    }

    fn handle_key(&mut self, key_event: KeyEvent) {
        if key_event.kind == KeyEventKind::Release {
            return;
        }
        if key_event.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key_event.code, KeyCode::Char('c') | KeyCode::Char('d'))
        {
            self.select(ClaudeMigrationSelection::SkipOnce);
            return;
        }
        match key_event.code {
            KeyCode::Up | KeyCode::Char('k') => self.set_highlight(self.highlighted.prev()),
            KeyCode::Down | KeyCode::Char('j') => self.set_highlight(self.highlighted.next()),
            KeyCode::Char('1') => self.select(ClaudeMigrationSelection::ImportNow),
            KeyCode::Char('2') => self.select(ClaudeMigrationSelection::SkipOnce),
            KeyCode::Char('3') => self.select(ClaudeMigrationSelection::Never),
            KeyCode::Enter => self.select(self.highlighted),
            KeyCode::Esc => self.select(ClaudeMigrationSelection::SkipOnce),
            _ => {}
        }
    }

    fn set_highlight(&mut self, highlighted: ClaudeMigrationSelection) {
        if self.highlighted != highlighted {
            self.highlighted = highlighted;
            self.request_frame.schedule_frame();
        }
    }

    fn select(&mut self, selection: ClaudeMigrationSelection) {
        self.highlighted = selection;
        self.selection = Some(selection);
        self.request_frame.schedule_frame();
    }

    fn is_done(&self) -> bool {
        self.selection.is_some()
    }

    fn selection(&self) -> Option<ClaudeHomeMigrationPromptOutcome> {
        self.selection.map(ClaudeMigrationSelection::into_outcome)
    }
}

impl WidgetRef for &ClaudeHomeMigrationPromptScreen {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        Clear.render(area, buf);

        let mut column = ColumnRenderable::new();
        let imported_config_keys = self.data.imported_config_keys.len();
        let copied_skills = self.data.copied_skills.len();

        column.push("");
        column.push(Line::from(vec![
            "Claude setup found".bold().cyan(),
            " ".into(),
            "(one-time migration available)".dim(),
        ]));
        column.push("");
        column.push(
            Line::from(format!(
                "Codex detected Claude config in ~/.claude and can import it into ~/.codex."
            ))
            .inset(Insets::tlbr(0, 2, 0, 0)),
        );
        column.push(
            Line::from(format!(
                "This check ran because prior Codex thread count is {}.",
                self.data.prior_codex_thread_count
            ))
            .dim()
            .inset(Insets::tlbr(0, 2, 0, 0)),
        );
        column.push("");
        column.push(
            Line::from(format!(
                "Will import {} config key(s)",
                imported_config_keys
            ))
            .inset(Insets::tlbr(0, 2, 0, 0)),
        );
        column.push(
            Line::from(format!("Will copy {} skill(s)", copied_skills))
                .inset(Insets::tlbr(0, 2, 0, 0)),
        );
        if self.data.copy_agents_md {
            column.push(
                Line::from("Will copy ~/.claude/CLAUDE.md to ~/.codex/AGENTS.md")
                    .inset(Insets::tlbr(0, 2, 0, 0)),
            );
        }
        column.push("");
        column.push(selection_option_row(
            0,
            "Import now".to_string(),
            self.highlighted == ClaudeMigrationSelection::ImportNow,
        ));
        column.push(selection_option_row(
            1,
            "Skip once".to_string(),
            self.highlighted == ClaudeMigrationSelection::SkipOnce,
        ));
        column.push(selection_option_row(
            2,
            "Never ask again".to_string(),
            self.highlighted == ClaudeMigrationSelection::Never,
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
