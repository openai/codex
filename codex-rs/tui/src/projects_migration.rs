use crate::key_hint;
use crate::render::Insets;
use crate::render::renderable::ColumnRenderable;
use crate::render::renderable::Renderable;
use crate::render::renderable::RenderableExt as _;
use crate::selection_list::selection_option_row;
use crate::tui::FrameRequester;
use crate::tui::Tui;
use crate::tui::TuiEvent;
use codex_core::config::CONFIG_TOML_FILE;
use codex_core::config::Config;
use codex_core::config::PROJECTS_TOML_FILE;
use codex_core::config::migrate_projects_to_projects_toml;
use codex_core::config::projects_in_config_toml;
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

pub(crate) enum ProjectsMigrationOutcome {
    Migrated,
    Skipped,
}

pub(crate) async fn run_projects_migration_prompt_if_needed(
    tui: &mut Tui,
    config: &Config,
) -> Result<Option<ProjectsMigrationOutcome>> {
    let has_projects = match projects_in_config_toml(&config.codex_home) {
        Ok(has_projects) => has_projects,
        Err(err) => {
            tracing::error!("Failed to check config.toml for projects: {err}");
            return Ok(None);
        }
    };
    if !has_projects {
        return Ok(None);
    }

    let mut screen = ProjectsMigrationScreen::new(tui.frame_requester(), config);
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

    match screen.selection() {
        Some(MigrationSelection::MoveProjects) => {
            match migrate_projects_to_projects_toml(&config.codex_home) {
                Ok(true) => Ok(Some(ProjectsMigrationOutcome::Migrated)),
                Ok(false) => Ok(Some(ProjectsMigrationOutcome::Skipped)),
                Err(err) => {
                    tracing::error!("Failed to migrate projects: {err}");
                    Ok(Some(ProjectsMigrationOutcome::Skipped))
                }
            }
        }
        Some(MigrationSelection::NotNow) | None => Ok(Some(ProjectsMigrationOutcome::Skipped)),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MigrationSelection {
    MoveProjects,
    NotNow,
}

struct ProjectsMigrationScreen {
    request_frame: FrameRequester,
    config_path: String,
    projects_path: String,
    highlighted: MigrationSelection,
    selection: Option<MigrationSelection>,
}

impl ProjectsMigrationScreen {
    fn new(request_frame: FrameRequester, config: &Config) -> Self {
        let config_path = config.codex_home.join(CONFIG_TOML_FILE);
        let projects_path = config.codex_home.join(PROJECTS_TOML_FILE);
        Self {
            request_frame,
            config_path: config_path.display().to_string(),
            projects_path: projects_path.display().to_string(),
            highlighted: MigrationSelection::MoveProjects,
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
            self.select(MigrationSelection::NotNow);
            return;
        }
        match key_event.code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.set_highlight(MigrationSelection::MoveProjects)
            }
            KeyCode::Down | KeyCode::Char('j') => self.set_highlight(MigrationSelection::NotNow),
            KeyCode::Char('1') => self.select(MigrationSelection::MoveProjects),
            KeyCode::Char('2') => self.select(MigrationSelection::NotNow),
            KeyCode::Enter => self.select(self.highlighted),
            KeyCode::Esc => self.select(MigrationSelection::NotNow),
            _ => {}
        }
    }

    fn set_highlight(&mut self, highlight: MigrationSelection) {
        if self.highlighted != highlight {
            self.highlighted = highlight;
            self.request_frame.schedule_frame();
        }
    }

    fn select(&mut self, selection: MigrationSelection) {
        self.highlighted = selection;
        self.selection = Some(selection);
        self.request_frame.schedule_frame();
    }

    fn is_done(&self) -> bool {
        self.selection.is_some()
    }

    fn selection(&self) -> Option<MigrationSelection> {
        self.selection
    }
}

impl WidgetRef for &ProjectsMigrationScreen {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        Clear.render(area, buf);
        let mut column = ColumnRenderable::new();

        column.push("");
        column.push(Line::from(vec![
            "> ".into(),
            "Move trusted projects to ".bold(),
            self.projects_path.clone().cyan().underlined(),
        ]));
        column.push("");
        column.push(
            Paragraph::new(format!(
                "Codex now stores trusted projects in {projects_path}. Move existing entries from {config_path}?",
                projects_path = self.projects_path,
                config_path = self.config_path
            ))
            .wrap(Wrap { trim: true })
            .inset(Insets::tlbr(0, 2, 0, 0)),
        );
        column.push("");
        column.push(selection_option_row(
            0,
            "Move projects now".to_string(),
            self.highlighted == MigrationSelection::MoveProjects,
        ));
        column.push(selection_option_row(
            1,
            "Not now".to_string(),
            self.highlighted == MigrationSelection::NotNow,
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
