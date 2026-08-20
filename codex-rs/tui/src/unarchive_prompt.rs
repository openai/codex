use crate::key_hint;
use crate::render::renderable::ColumnRenderable;
use crate::render::renderable::Renderable;
use crate::selection_list::selection_option_row;
use crate::session_start::SessionStartAction;
use crate::tui::Tui;
use crate::tui::TuiEvent;
use codex_protocol::ThreadId;
use color_eyre::Result;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize as _;
use ratatui::text::Line;
use ratatui::widgets::Clear;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;
use ratatui::widgets::WidgetRef;
use ratatui::widgets::Wrap;
use tokio_stream::StreamExt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum UnarchiveChoice {
    Unarchive,
    Cancel,
}

pub(crate) async fn run_unarchive_prompt(
    tui: &mut Tui,
    thread_id: ThreadId,
    action: SessionStartAction,
) -> Result<UnarchiveChoice> {
    let mut guard = PromptScreenGuard::enter(tui)?;
    let mut screen = UnarchivePrompt::new(thread_id, action);
    guard.draw(&screen)?;
    guard
        .tui
        .discard_pending_input_before_interactive_screen()?;
    let events = guard.tui.event_stream();
    tokio::pin!(events);
    loop {
        guard.draw(&screen)?;
        let Some(event) = events.next().await else {
            return Ok(UnarchiveChoice::Cancel);
        };
        guard.tui.screen_size_for_event(&event)?;
        match event {
            TuiEvent::Key(key) => {
                if let Some(choice) = screen.handle_key(key) {
                    return Ok(choice);
                }
            }
            TuiEvent::Paste(_) | TuiEvent::Draw | TuiEvent::Resize(_) | TuiEvent::Resume => {}
        }
    }
}

/// Keep startup prompts out of normal scrollback when an alternate screen is available.
/// Inline mode owns only its content-height viewport, including when the terminal is resized.
struct PromptScreenGuard<'a> {
    tui: &'a mut Tui,
    entered_alt_screen: bool,
}

impl<'a> PromptScreenGuard<'a> {
    fn enter(tui: &'a mut Tui) -> Result<Self> {
        let was_alternate = tui.is_alt_screen_active();
        if !was_alternate {
            tui.enter_alt_screen()?;
        }
        let entered_alt_screen = !was_alternate && tui.is_alt_screen_active();
        Ok(Self {
            tui,
            entered_alt_screen,
        })
    }

    fn draw(&mut self, screen: &UnarchivePrompt) -> Result<()> {
        let height = if self.tui.is_alt_screen_active() {
            u16::MAX
        } else {
            screen
                .content()
                .desired_height(self.tui.terminal.size()?.width)
        };
        self.tui.draw(height, |frame| {
            frame.render_widget_ref(screen, frame.area());
        })?;
        Ok(())
    }
}

impl Drop for PromptScreenGuard<'_> {
    fn drop(&mut self) {
        let result = if self.entered_alt_screen {
            self.tui.leave_alt_screen()
        } else if !self.tui.is_alt_screen_active() {
            let result = self.tui.terminal.clear();
            let mut area = self.tui.terminal.viewport_area;
            area.height = 0;
            self.tui.terminal.set_viewport_area(area);
            result
        } else {
            Ok(())
        };
        if let Err(err) = result {
            tracing::warn!("failed to restore terminal after unarchive prompt: {err}");
        }
    }
}

struct UnarchivePrompt {
    thread_id: ThreadId,
    action: SessionStartAction,
    highlighted: UnarchiveChoice,
}

impl UnarchivePrompt {
    fn new(thread_id: ThreadId, action: SessionStartAction) -> Self {
        Self {
            thread_id,
            action,
            highlighted: UnarchiveChoice::Unarchive,
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> Option<UnarchiveChoice> {
        if key.kind == KeyEventKind::Release {
            return None;
        }
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('c') | KeyCode::Char('d'))
        {
            return Some(UnarchiveChoice::Cancel);
        }
        match key.code {
            KeyCode::Up | KeyCode::Down | KeyCode::Char('k' | 'j') => {
                self.highlighted = match self.highlighted {
                    UnarchiveChoice::Unarchive => UnarchiveChoice::Cancel,
                    UnarchiveChoice::Cancel => UnarchiveChoice::Unarchive,
                };
                None
            }
            KeyCode::Char('1' | 'y' | 'Y') => Some(UnarchiveChoice::Unarchive),
            KeyCode::Esc | KeyCode::Char('2' | 'n' | 'N') => Some(UnarchiveChoice::Cancel),
            KeyCode::Enter => Some(self.highlighted),
            _ => None,
        }
    }

    fn content(&self) -> ColumnRenderable<'static> {
        let mut column = ColumnRenderable::new();
        column.push("");
        column.push(
            Paragraph::new("This conversation is archived".bold()).wrap(Wrap { trim: false }),
        );
        column.push(
            Paragraph::new(Line::from(self.thread_id.to_string()).dim()).wrap(Wrap { trim: false }),
        );
        column.push("");
        column.push(selection_option_row(
            /*index*/ 0,
            format!("Unarchive and {}", self.action.verb()),
            self.highlighted == UnarchiveChoice::Unarchive,
        ));
        column.push(selection_option_row(
            /*index*/ 1,
            "Cancel".to_string(),
            self.highlighted == UnarchiveChoice::Cancel,
        ));
        column.push("");
        column.push(
            Paragraph::new(Line::from(vec![
                "Press ".dim(),
                key_hint::plain(KeyCode::Enter).into(),
                " to continue or ".dim(),
                key_hint::plain(KeyCode::Esc).into(),
                " to cancel".dim(),
            ]))
            .wrap(Wrap { trim: false }),
        );
        column
    }
}

impl WidgetRef for &UnarchivePrompt {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        Clear.render(area, buf);
        self.content().render(area, buf);
    }
}

#[cfg(test)]
#[path = "unarchive_prompt_tests.rs"]
mod tests;
