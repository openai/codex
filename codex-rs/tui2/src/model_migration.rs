use crate::key_hint;
use crate::render::Insets;
use crate::render::renderable::ColumnRenderable;
use crate::render::renderable::Renderable;
use crate::render::renderable::RenderableExt as _;
use crate::selection_list::selection_option_row;
use crate::tui::FrameRequester;
use crate::tui::Tui;
use crate::tui::TuiEvent;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;
use ratatui::prelude::Stylize as _;
use ratatui::prelude::Widget;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Clear;
use ratatui::widgets::Paragraph;
use ratatui::widgets::WidgetRef;
use ratatui::widgets::Wrap;
use tokio_stream::StreamExt;

/// Outcome of the migration prompt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ModelMigrationOutcome {
    Accepted,
    Rejected,
    Exit,
}

#[derive(Clone)]
pub(crate) struct ModelMigrationCopy {
    pub heading: Vec<Span<'static>>,
    pub content: Vec<Line<'static>>,
    pub can_opt_out: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MigrationMenuOption {
    TryNewModel,
    UseExistingModel,
}

impl MigrationMenuOption {
    fn all() -> [Self; 2] {
        [Self::TryNewModel, Self::UseExistingModel]
    }

    fn label(self) -> &'static str {
        match self {
            Self::TryNewModel => "Try new model",
            Self::UseExistingModel => "Use existing model",
        }
    }
}

pub(crate) fn migration_copy_for_models(
    current_model: &str,
    target_model: &str,
    target_display_name: String,
    target_description: Option<String>,
    can_opt_out: bool,
) -> ModelMigrationCopy {
    let heading_text = Span::from(format!("Try {target_display_name}")).bold();
    let description_line = target_description
        .filter(|desc| !desc.is_empty())
        .map(Line::from)
        .unwrap_or_else(|| {
            Line::from(format!(
                "{target_display_name} is recommended for better performance and reliability."
            ))
        });

    let mut content = vec![
        Line::from(format!(
            "We recommend switching from {current_model} to {target_model}."
        )),
        Line::from(""),
        description_line,
        Line::from(""),
    ];

    if can_opt_out {
        content.push(Line::from(format!(
            "You can continue using {current_model} if you prefer."
        )));
    } else {
        content.push(Line::from("Press enter to continue".dim()));
    }

    ModelMigrationCopy {
        heading: vec![heading_text],
        content,
        can_opt_out,
    }
}

pub(crate) async fn run_model_migration_prompt(
    tui: &mut Tui,
    copy: ModelMigrationCopy,
) -> ModelMigrationOutcome {
    let alt = AltScreenGuard::enter(tui);
    let mut screen = ModelMigrationScreen::new(alt.tui.frame_requester(), copy);

    let _ = alt.tui.draw(u16::MAX, |frame| {
        frame.render_widget_ref(&screen, frame.area());
    });

    let events = alt.tui.event_stream();
    tokio::pin!(events);

    while !screen.is_done() {
        if let Some(event) = events.next().await {
            match event {
                TuiEvent::Key(key_event) => screen.handle_key(key_event),
                TuiEvent::Mouse(_) => {}
                TuiEvent::Paste(_) => {}
                TuiEvent::Draw => {
                    let _ = alt.tui.draw(u16::MAX, |frame| {
                        frame.render_widget_ref(&screen, frame.area());
                    });
                }
            }
        } else {
            screen.accept();
            break;
        }
    }

    screen.outcome()
}

struct ModelMigrationScreen {
    request_frame: FrameRequester,
    copy: ModelMigrationCopy,
    done: bool,
    outcome: ModelMigrationOutcome,
    highlighted_option: MigrationMenuOption,
}

impl ModelMigrationScreen {
    fn new(request_frame: FrameRequester, copy: ModelMigrationCopy) -> Self {
        Self {
            request_frame,
            copy,
            done: false,
            outcome: ModelMigrationOutcome::Accepted,
            highlighted_option: MigrationMenuOption::TryNewModel,
        }
    }

    fn finish_with(&mut self, outcome: ModelMigrationOutcome) {
        self.outcome = outcome;
        self.done = true;
        self.request_frame.schedule_frame();
    }

    fn accept(&mut self) {
        self.finish_with(ModelMigrationOutcome::Accepted);
    }

    fn reject(&mut self) {
        self.finish_with(ModelMigrationOutcome::Rejected);
    }

    fn exit(&mut self) {
        self.finish_with(ModelMigrationOutcome::Exit);
    }

    fn confirm_selection(&mut self) {
        if self.copy.can_opt_out {
            match self.highlighted_option {
                MigrationMenuOption::TryNewModel => self.accept(),
                MigrationMenuOption::UseExistingModel => self.reject(),
            }
        } else {
            self.accept();
        }
    }

    fn highlight_option(&mut self, option: MigrationMenuOption) {
        if self.highlighted_option != option {
            self.highlighted_option = option;
            self.request_frame.schedule_frame();
        }
    }

    fn handle_key(&mut self, key_event: KeyEvent) {
        if key_event.kind == KeyEventKind::Release {
            return;
        }

        if is_ctrl_exit_combo(key_event) {
            self.exit();
            return;
        }

        if self.copy.can_opt_out {
            self.handle_menu_key(key_event.code);
        } else if matches!(key_event.code, KeyCode::Esc | KeyCode::Enter) {
            self.accept();
        }
    }

    fn is_done(&self) -> bool {
        self.done
    }

    fn outcome(&self) -> ModelMigrationOutcome {
        self.outcome
    }
}

impl WidgetRef for &ModelMigrationScreen {
    fn render_ref(&self, area: ratatui::layout::Rect, buf: &mut ratatui::buffer::Buffer) {
        Clear.render(area, buf);

        let mut column = ColumnRenderable::new();
        column.push("");
        column.push(self.heading_line());
        column.push(Line::from(""));
        self.render_content(&mut column);
        if self.copy.can_opt_out {
            self.render_menu(&mut column);
        }

        column.render(area, buf);
    }
}

impl ModelMigrationScreen {
    fn handle_menu_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.highlight_option(MigrationMenuOption::TryNewModel);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.highlight_option(MigrationMenuOption::UseExistingModel);
            }
            KeyCode::Char('1') => {
                self.highlight_option(MigrationMenuOption::TryNewModel);
                self.accept();
            }
            KeyCode::Char('2') => {
                self.highlight_option(MigrationMenuOption::UseExistingModel);
                self.reject();
            }
            KeyCode::Enter | KeyCode::Esc => self.confirm_selection(),
            _ => {}
        }
    }

    fn heading_line(&self) -> Line<'static> {
        let mut heading = vec![Span::raw("> ")];
        heading.extend(self.copy.heading.iter().cloned());
        Line::from(heading)
    }

    fn render_content(&self, column: &mut ColumnRenderable) {
        self.render_lines(&self.copy.content, column);
    }

    fn render_lines(&self, lines: &[Line<'static>], column: &mut ColumnRenderable) {
        for line in lines {
            column.push(
                Paragraph::new(line.clone())
                    .wrap(Wrap { trim: false })
                    .inset(Insets::tlbr(0, 2, 0, 0)),
            );
        }
    }

    fn render_menu(&self, column: &mut ColumnRenderable) {
        column.push(Line::from(""));
        column.push(
            Paragraph::new("Choose how you'd like Codex to proceed.")
                .wrap(Wrap { trim: false })
                .inset(Insets::tlbr(0, 2, 0, 0)),
        );
        column.push(Line::from(""));

        for (idx, option) in MigrationMenuOption::all().into_iter().enumerate() {
            column.push(selection_option_row(
                idx,
                option.label().to_string(),
                self.highlighted_option == option,
            ));
        }

        column.push(Line::from(""));
        column.push(
            Line::from(vec![
                "Use ".dim(),
                key_hint::plain(KeyCode::Up).into(),
                "/".dim(),
                key_hint::plain(KeyCode::Down).into(),
                " to move, press ".dim(),
                key_hint::plain(KeyCode::Enter).into(),
                " to confirm".dim(),
            ])
            .inset(Insets::tlbr(0, 2, 0, 0)),
        );
    }
}

// Render the prompt on the terminal's alternate screen so exiting or cancelling
// does not leave a large blank region in the normal scrollback. This does not
// change the prompt's appearance – only where it is drawn.
struct AltScreenGuard<'a> {
    tui: &'a mut Tui,
}

impl<'a> AltScreenGuard<'a> {
    fn enter(tui: &'a mut Tui) -> Self {
        let _ = tui.enter_alt_screen();
        Self { tui }
    }
}

impl Drop for AltScreenGuard<'_> {
    fn drop(&mut self) {
        let _ = self.tui.leave_alt_screen();
    }
}

fn is_ctrl_exit_combo(key_event: KeyEvent) -> bool {
    key_event.modifiers.contains(KeyModifiers::CONTROL)
        && matches!(key_event.code, KeyCode::Char('c') | KeyCode::Char('d'))
}

#[cfg(test)]
mod tests {
    use super::ModelMigrationScreen;
    use super::migration_copy_for_models;
    use crate::custom_terminal::Terminal;
    use crate::test_backend::VT100Backend;
    use crate::tui::FrameRequester;
    use crossterm::event::KeyCode;
    use crossterm::event::KeyEvent;
    use insta::assert_snapshot;
    use ratatui::layout::Rect;

    #[test]
    fn prompt_snapshot() {
        let width: u16 = 60;
        let height: u16 = 28;
        let backend = VT100Backend::new(width, height);
        let mut terminal = Terminal::with_options(backend).expect("terminal");
        terminal.set_viewport_area(Rect::new(0, 0, width, height));

        let screen = ModelMigrationScreen::new(
            FrameRequester::test_dummy(),
            migration_copy_for_models(
                "gpt-5.1-codex-mini",
                "gpt-5.1-codex-max",
                "gpt-5.1-codex-max".to_string(),
                Some("Codex-optimized flagship for deep and fast reasoning.".to_string()),
                true,
            ),
        );

        {
            let mut frame = terminal.get_frame();
            frame.render_widget_ref(&screen, frame.area());
        }
        terminal.flush().expect("flush");

        assert_snapshot!("model_migration_prompt", terminal.backend());
    }

    #[test]
    fn prompt_snapshot_gpt5_family() {
        let backend = VT100Backend::new(65, 22);
        let mut terminal = Terminal::with_options(backend).expect("terminal");
        terminal.set_viewport_area(Rect::new(0, 0, 65, 22));

        let screen = ModelMigrationScreen::new(
            FrameRequester::test_dummy(),
            migration_copy_for_models(
                "gpt-5",
                "gpt-5.1",
                "gpt-5.1".to_string(),
                Some("Broad world knowledge with strong general reasoning.".to_string()),
                false,
            ),
        );
        {
            let mut frame = terminal.get_frame();
            frame.render_widget_ref(&screen, frame.area());
        }
        terminal.flush().expect("flush");
        assert_snapshot!("model_migration_prompt_gpt5_family", terminal.backend());
    }

    #[test]
    fn prompt_snapshot_gpt5_codex() {
        let backend = VT100Backend::new(60, 22);
        let mut terminal = Terminal::with_options(backend).expect("terminal");
        terminal.set_viewport_area(Rect::new(0, 0, 60, 22));

        let screen = ModelMigrationScreen::new(
            FrameRequester::test_dummy(),
            migration_copy_for_models(
                "gpt-5-codex",
                "gpt-5.1-codex-max",
                "gpt-5.1-codex-max".to_string(),
                Some("Codex-optimized flagship for deep and fast reasoning.".to_string()),
                false,
            ),
        );
        {
            let mut frame = terminal.get_frame();
            frame.render_widget_ref(&screen, frame.area());
        }
        terminal.flush().expect("flush");
        assert_snapshot!("model_migration_prompt_gpt5_codex", terminal.backend());
    }

    #[test]
    fn prompt_snapshot_gpt5_codex_mini() {
        let backend = VT100Backend::new(60, 22);
        let mut terminal = Terminal::with_options(backend).expect("terminal");
        terminal.set_viewport_area(Rect::new(0, 0, 60, 22));

        let screen = ModelMigrationScreen::new(
            FrameRequester::test_dummy(),
            migration_copy_for_models(
                "gpt-5-codex-mini",
                "gpt-5.1-codex-mini",
                "gpt-5.1-codex-mini".to_string(),
                Some("Optimized for codex. Cheaper, faster, but less capable.".to_string()),
                false,
            ),
        );
        {
            let mut frame = terminal.get_frame();
            frame.render_widget_ref(&screen, frame.area());
        }
        terminal.flush().expect("flush");
        assert_snapshot!("model_migration_prompt_gpt5_codex_mini", terminal.backend());
    }

    #[test]
    fn escape_key_accepts_prompt() {
        let mut screen = ModelMigrationScreen::new(
            FrameRequester::test_dummy(),
            migration_copy_for_models(
                "gpt-old",
                "gpt-new",
                "gpt-new".to_string(),
                Some("Latest recommended model for better performance.".to_string()),
                true,
            ),
        );

        // Simulate pressing Escape
        screen.handle_key(KeyEvent::new(
            KeyCode::Esc,
            crossterm::event::KeyModifiers::NONE,
        ));
        assert!(screen.is_done());
        // Esc should not be treated as Exit – it accepts like Enter.
        assert!(matches!(
            screen.outcome(),
            super::ModelMigrationOutcome::Accepted
        ));
    }

    #[test]
    fn selecting_use_existing_model_rejects_upgrade() {
        let mut screen = ModelMigrationScreen::new(
            FrameRequester::test_dummy(),
            migration_copy_for_models(
                "gpt-old",
                "gpt-new",
                "gpt-new".to_string(),
                Some("Latest recommended model for better performance.".to_string()),
                true,
            ),
        );

        screen.handle_key(KeyEvent::new(
            KeyCode::Down,
            crossterm::event::KeyModifiers::NONE,
        ));
        screen.handle_key(KeyEvent::new(
            KeyCode::Enter,
            crossterm::event::KeyModifiers::NONE,
        ));

        assert!(screen.is_done());
        assert!(matches!(
            screen.outcome(),
            super::ModelMigrationOutcome::Rejected
        ));
    }
}
