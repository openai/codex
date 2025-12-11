use crate::tui::FrameRequester;
use crate::tui::Tui;
use crate::tui::TuiEvent;
use codex_core::skills::SkillError;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::prelude::Stylize as _;
use ratatui::text::Line;
use ratatui::widgets::Block;
use ratatui::widgets::Borders;
use ratatui::widgets::Clear;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;
use ratatui::widgets::WidgetRef;
use ratatui::widgets::Wrap;
use tokio_stream::StreamExt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SkillErrorPromptOutcome {
    Continue,
    Exit,
}

pub(crate) async fn run_skill_error_prompt(
    tui: &mut Tui,
    errors: &[SkillError],
) -> SkillErrorPromptOutcome {
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
    let mut screen = SkillErrorScreen::new(alt.tui.frame_requester(), errors);

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
            screen.confirm_continue();
            break;
        }
    }

    screen.outcome()
}

struct SkillErrorScreen {
    request_frame: FrameRequester,
    lines: Vec<Line<'static>>,
    done: bool,
    exit: bool,
}

impl SkillErrorScreen {
    fn new(request_frame: FrameRequester, errors: &[SkillError]) -> Self {
        let mut lines: Vec<Line<'static>> = Vec::new();
        lines.push(Line::from("Skill validation errors detected".bold()));
        lines.push(Line::from(
            "Fix these SKILL.md files and restart. Invalid skills are ignored until resolved. Press enter or esc to continue, Ctrl+C or Ctrl+D to exit.",
        ));
        lines.push(Line::from(""));

        for error in errors {
            let message = format!("- {}: {}", error.path.display(), error.message);
            lines.push(Line::from(message));
        }

        Self {
            request_frame,
            lines,
            done: false,
            exit: false,
        }
    }

    fn is_done(&self) -> bool {
        self.done
    }

    fn confirm_continue(&mut self) {
        self.done = true;
        self.exit = false;
        self.request_frame.schedule_frame();
    }

    fn confirm_exit(&mut self) {
        self.done = true;
        self.exit = true;
        self.request_frame.schedule_frame();
    }

    fn outcome(&self) -> SkillErrorPromptOutcome {
        if self.exit {
            SkillErrorPromptOutcome::Exit
        } else {
            SkillErrorPromptOutcome::Continue
        }
    }

    fn handle_key(&mut self, key_event: KeyEvent) {
        if key_event.kind == KeyEventKind::Release {
            return;
        }

        if key_event
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::META)
            && matches!(key_event.code, KeyCode::Char('c') | KeyCode::Char('d'))
        {
            self.confirm_exit();
            return;
        }

        match key_event.code {
            KeyCode::Enter | KeyCode::Esc | KeyCode::Char(' ') | KeyCode::Char('q') => {
                self.confirm_continue();
            }
            _ => {}
        }
    }
}

impl WidgetRef for &SkillErrorScreen {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        Clear.render(area, buf);
        let block = Block::default()
            .title("Skill errors".bold())
            .borders(Borders::ALL);
        Paragraph::new(self.lines.clone())
            .block(block)
            .wrap(Wrap { trim: true })
            .render(area, buf);
    }
}
