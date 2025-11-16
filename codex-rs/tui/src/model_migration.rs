use crate::render::Insets;
use crate::render::renderable::ColumnRenderable;
use crate::render::renderable::Renderable;
use crate::render::renderable::RenderableExt as _;
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
use ratatui::widgets::Clear;
use ratatui::widgets::Paragraph;
use ratatui::widgets::WidgetRef;
use ratatui::widgets::Wrap;
use tokio_stream::StreamExt;

/// Outcome of the migration prompt.
pub(crate) enum ModelMigrationOutcome {
    Accepted,
    Exit,
}

pub(crate) async fn run_model_migration_prompt(tui: &mut Tui) -> ModelMigrationOutcome {
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

    let alt = AltScreenGuard::enter(tui);

    let mut screen = ModelMigrationScreen::new(alt.tui.frame_requester());

    let _ = alt.tui.draw(u16::MAX, |frame| {
        frame.render_widget_ref(&screen, frame.area());
    });

    let events = alt.tui.event_stream();
    tokio::pin!(events);

    while !screen.is_done() {
        if let Some(event) = events.next().await {
            match event {
                TuiEvent::Key(key_event) => screen.handle_key(key_event),
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
    done: bool,
    should_exit: bool,
}

impl ModelMigrationScreen {
    fn new(request_frame: FrameRequester) -> Self {
        Self {
            request_frame,
            done: false,
            should_exit: false,
        }
    }

    fn accept(&mut self) {
        self.done = true;
        self.request_frame.schedule_frame();
    }

    fn handle_key(&mut self, key_event: KeyEvent) {
        if key_event.kind == KeyEventKind::Release {
            return;
        }

        if key_event.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key_event.code, KeyCode::Char('c') | KeyCode::Char('d'))
        {
            self.should_exit = true;
            self.done = true;
            self.request_frame.schedule_frame();
            return;
        }

        if matches!(key_event.code, KeyCode::Esc | KeyCode::Enter) {
            self.accept();
        }
    }

    fn is_done(&self) -> bool {
        self.done
    }

    fn outcome(&self) -> ModelMigrationOutcome {
        if self.should_exit {
            ModelMigrationOutcome::Exit
        } else {
            ModelMigrationOutcome::Accepted
        }
    }
}

impl WidgetRef for &ModelMigrationScreen {
    fn render_ref(&self, area: ratatui::layout::Rect, buf: &mut ratatui::buffer::Buffer) {
        Clear.render(area, buf);

        let mut column = ColumnRenderable::new();

        column.push("");
        column.push(Line::from(vec![
            "> ".into(),
            "Introducing our gpt-5.1 models".bold(),
        ]));
        column.push(Line::from(""));

        column.push(
            Paragraph::new(Line::from(
                "We've upgraded our family of models supported in Codex to gpt-5.1, gpt-5.1-codex and gpt-5.1-codex-mini.",
            ))
            .wrap(Wrap { trim: false })
            .inset(Insets::tlbr(0, 2, 0, 0)),
        );
        column.push(Line::from(""));
        column.push(
            Paragraph::new(Line::from(
                "You can continue using legacy models by specifying them directly with the -m option or in your config.toml.",
            ))
            .wrap(Wrap { trim: false })
            .inset(Insets::tlbr(0, 2, 0, 0)),
        );
        column.push(Line::from(""));
        column.push(
            Line::from(vec![
                "Learn more at ".into(),
                "www.openai.com/index/gpt-5-1".cyan().underlined(),
                ".".into(),
            ])
            .inset(Insets::tlbr(0, 2, 0, 0)),
        );
        column.push(Line::from(""));
        column.push(
            Line::from(vec!["Press enter to continue".dim()]).inset(Insets::tlbr(0, 2, 0, 0)),
        );

        column.render(area, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::ModelMigrationScreen;
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
        let height: u16 = 12;
        let backend = VT100Backend::new(width, height);
        let mut terminal = Terminal::with_options(backend).expect("terminal");
        terminal.set_viewport_area(Rect::new(0, 0, width, height));

        let screen = ModelMigrationScreen::new(FrameRequester::test_dummy());

        {
            let mut frame = terminal.get_frame();
            frame.render_widget_ref(&screen, frame.area());
        }
        terminal.flush().expect("flush");

        assert_snapshot!("model_migration_prompt", terminal.backend());
    }

    #[test]
    fn prompt_snapshot_gpt5_family() {
        let backend = VT100Backend::new(65, 12);
        let mut terminal = Terminal::with_options(backend).expect("terminal");
        terminal.set_viewport_area(Rect::new(0, 0, 65, 12));

        let screen = ModelMigrationScreen::new(FrameRequester::test_dummy());
        {
            let mut frame = terminal.get_frame();
            frame.render_widget_ref(&screen, frame.area());
        }
        terminal.flush().expect("flush");
        assert_snapshot!("model_migration_prompt_gpt5_family", terminal.backend());
    }

    #[test]
    fn prompt_snapshot_gpt5_codex() {
        let backend = VT100Backend::new(60, 12);
        let mut terminal = Terminal::with_options(backend).expect("terminal");
        terminal.set_viewport_area(Rect::new(0, 0, 60, 12));

        let screen = ModelMigrationScreen::new(FrameRequester::test_dummy());
        {
            let mut frame = terminal.get_frame();
            frame.render_widget_ref(&screen, frame.area());
        }
        terminal.flush().expect("flush");
        assert_snapshot!("model_migration_prompt_gpt5_codex", terminal.backend());
    }

    #[test]
    fn prompt_snapshot_gpt5_codex_mini() {
        let backend = VT100Backend::new(60, 12);
        let mut terminal = Terminal::with_options(backend).expect("terminal");
        terminal.set_viewport_area(Rect::new(0, 0, 60, 12));

        let screen = ModelMigrationScreen::new(FrameRequester::test_dummy());
        {
            let mut frame = terminal.get_frame();
            frame.render_widget_ref(&screen, frame.area());
        }
        terminal.flush().expect("flush");
        assert_snapshot!("model_migration_prompt_gpt5_codex_mini", terminal.backend());
    }

    #[test]
    fn escape_key_accepts_prompt() {
        let mut screen = ModelMigrationScreen::new(FrameRequester::test_dummy());

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
}
