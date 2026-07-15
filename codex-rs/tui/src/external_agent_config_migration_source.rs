use crate::key_hint;
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
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::prelude::Stylize as _;
use ratatui::prelude::Widget;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Clear;
use ratatui::widgets::WidgetRef;
use tokio_stream::StreamExt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ExternalAgentConfigMigrationSource {
    Cla,
    Cur,
}

impl ExternalAgentConfigMigrationSource {
    pub(crate) const ALL: [Self; 2] = [Self::Cla, Self::Cur];

    pub(crate) fn migration_source(self) -> &'static str {
        match self {
            Self::Cla => "claude-code",
            Self::Cur => "cursor",
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Cla => "Claude Code",
            Self::Cur => "Cursor",
        }
    }
}

pub(crate) async fn run_external_agent_config_source_prompt(
    tui: &mut Tui,
    sources: &[ExternalAgentConfigMigrationSource],
) -> Option<ExternalAgentConfigMigrationSource> {
    let mut screen = ExternalAgentConfigSourceScreen::new(tui.frame_requester(), sources);
    let _ = tui.draw(u16::MAX, |frame| {
        frame.render_widget_ref(&screen, frame.area());
    });

    let events = tui.event_stream();
    tokio::pin!(events);

    while !screen.is_done() {
        if let Some(event) = events.next().await {
            match event {
                TuiEvent::Key(key_event) => screen.handle_key(key_event),
                TuiEvent::Paste(_) => {}
                TuiEvent::Draw | TuiEvent::Resize => {
                    let _ = tui.draw(u16::MAX, |frame| {
                        frame.render_widget_ref(&screen, frame.area());
                    });
                }
            }
        } else {
            screen.cancel();
        }
    }

    screen.selection()
}

struct ExternalAgentConfigSourceScreen {
    request_frame: FrameRequester,
    sources: Vec<ExternalAgentConfigMigrationSource>,
    highlighted_idx: usize,
    selection: Option<ExternalAgentConfigMigrationSource>,
    done: bool,
}

impl ExternalAgentConfigSourceScreen {
    fn new(request_frame: FrameRequester, sources: &[ExternalAgentConfigMigrationSource]) -> Self {
        Self {
            request_frame,
            sources: sources.to_vec(),
            highlighted_idx: 0,
            selection: None,
            done: sources.is_empty(),
        }
    }

    fn handle_key(&mut self, key_event: KeyEvent) {
        if key_event.kind == KeyEventKind::Release || self.sources.is_empty() {
            return;
        }
        if key_event.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key_event.code, KeyCode::Char('c') | KeyCode::Char('d'))
        {
            self.cancel();
            return;
        }
        match key_event.code {
            KeyCode::Up | KeyCode::Char('k') => self.move_highlight(/*delta*/ -1),
            KeyCode::Down | KeyCode::Char('j') => self.move_highlight(/*delta*/ 1),
            KeyCode::Char(character) if character.is_ascii_digit() => {
                let Some(index) = character
                    .to_digit(10)
                    .and_then(|index| index.checked_sub(1))
                else {
                    return;
                };
                self.select(index as usize);
            }
            KeyCode::Enter => self.select(self.highlighted_idx),
            KeyCode::Esc => self.cancel(),
            _ => {}
        }
    }

    fn move_highlight(&mut self, delta: isize) {
        self.highlighted_idx = self
            .highlighted_idx
            .saturating_add_signed(delta)
            .min(self.sources.len().saturating_sub(1));
        self.request_frame.schedule_frame();
    }

    fn select(&mut self, index: usize) {
        let Some(source) = self.sources.get(index).copied() else {
            return;
        };
        self.selection = Some(source);
        self.done = true;
        self.request_frame.schedule_frame();
    }

    fn cancel(&mut self) {
        self.selection = None;
        self.done = true;
        self.request_frame.schedule_frame();
    }

    fn is_done(&self) -> bool {
        self.done
    }

    fn selection(&self) -> Option<ExternalAgentConfigMigrationSource> {
        self.selection
    }
}

impl WidgetRef for &ExternalAgentConfigSourceScreen {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        Clear.render(area, buf);
        let mut column = ColumnRenderable::new();
        column.push("");
        column.push("Choose an import source".bold());
        column.push("");
        column.push(
            Line::from("Select the app whose setup you want to import.".dim()).inset(Insets::tlbr(
                /*top*/ 0, /*left*/ 2, /*bottom*/ 0, /*right*/ 0,
            )),
        );
        column.push("");
        for (index, source) in self.sources.iter().enumerate() {
            let prefix = if index == self.highlighted_idx {
                "›"
            } else {
                " "
            };
            let label = Span::from(format!("{prefix} {}. {}", index + 1, source.label()));
            column.push(Line::from(if index == self.highlighted_idx {
                label.cyan()
            } else {
                label
            }));
        }
        column.push("");
        column.push(
            Line::from(vec![
                "Press ".dim(),
                key_hint::plain(KeyCode::Enter).into(),
                " to continue".dim(),
            ])
            .inset(Insets::tlbr(
                /*top*/ 0, /*left*/ 2, /*bottom*/ 0, /*right*/ 0,
            )),
        );
        column.render(area, buf);
    }
}

#[cfg(test)]
#[path = "external_agent_config_migration_source_tests.rs"]
mod tests;
