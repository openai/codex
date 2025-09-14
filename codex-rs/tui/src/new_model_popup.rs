use crate::tui::FrameRequester;
use crate::tui::Tui;
use crate::tui::TuiEvent;
use codex_core::config::SWIFTFOX_MEDIUM_MODEL;
use color_eyre::eyre::Result;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::prelude::Widget;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Clear;
use ratatui::widgets::Paragraph;
use ratatui::widgets::WidgetRef;
use ratatui::widgets::Wrap;
use tokio_stream::StreamExt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ModelUpgradeDecision {
    Switch,
    KeepCurrent,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ModelUpgradeOption {
    TryNewModel,
    KeepCurrent,
}

struct ModelUpgradePopup {
    highlighted: ModelUpgradeOption,
    decision: Option<ModelUpgradeDecision>,
    request_frame: FrameRequester,
}

impl ModelUpgradePopup {
    fn new(request_frame: FrameRequester) -> Self {
        Self {
            highlighted: ModelUpgradeOption::TryNewModel,
            decision: None,
            request_frame,
        }
    }

    fn handle_key_event(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Up | KeyCode::Char('k') => self.highlight(ModelUpgradeOption::TryNewModel),
            KeyCode::Down | KeyCode::Char('j') => self.highlight(ModelUpgradeOption::KeepCurrent),
            KeyCode::Char('1') => self.select(ModelUpgradeOption::TryNewModel),
            KeyCode::Char('2') => self.select(ModelUpgradeOption::KeepCurrent),
            KeyCode::Enter => self.select(self.highlighted),
            KeyCode::Esc => self.select(ModelUpgradeOption::KeepCurrent),
            _ => {}
        }
    }

    fn highlight(&mut self, option: ModelUpgradeOption) {
        if self.highlighted != option {
            self.highlighted = option;
            self.request_frame.schedule_frame();
        }
    }

    fn select(&mut self, option: ModelUpgradeOption) {
        self.decision = Some(option.into());
        self.request_frame.schedule_frame();
    }
}

impl From<ModelUpgradeOption> for ModelUpgradeDecision {
    fn from(option: ModelUpgradeOption) -> Self {
        match option {
            ModelUpgradeOption::TryNewModel => ModelUpgradeDecision::Switch,
            ModelUpgradeOption::KeepCurrent => ModelUpgradeDecision::KeepCurrent,
        }
    }
}

impl WidgetRef for &ModelUpgradePopup {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        Clear.render(area, buf);

        let mut lines: Vec<Line> = vec![
            String::new().into(),
            format!("   Codex is now powered by {SWIFTFOX_MEDIUM_MODEL}, a new model that is")
                .into(),
            Line::from(vec![
                "   ".into(),
                "faster, a better collaborator, ".bold(),
                "and ".into(),
                "more steerable.".bold(),
            ]),
            "".into(),
        ];

        let create_option =
            |index: usize, option: ModelUpgradeOption, text: &str| -> Line<'static> {
                if self.highlighted == option {
                    Line::from(vec![
                        format!("> {}. ", index + 1).cyan(),
                        text.to_owned().cyan(),
                    ])
                } else {
                    format!("  {}. {text}", index + 1).into()
                }
            };

        lines.push(create_option(
            0,
            ModelUpgradeOption::TryNewModel,
            &format!("Yes, switch me to {SWIFTFOX_MEDIUM_MODEL}"),
        ));
        lines.push(create_option(
            1,
            ModelUpgradeOption::KeepCurrent,
            "Not right now",
        ));
        lines.push("".into());
        lines.push(
            "  Press Enter to confirm or Esc to keep your current model"
                .dim()
                .into(),
        );

        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .render(area, buf);
    }
}

pub(crate) async fn run_model_upgrade_popup(tui: &mut Tui) -> Result<ModelUpgradeDecision> {
    let mut popup = ModelUpgradePopup::new(tui.frame_requester());

    tui.draw(u16::MAX, |frame| {
        frame.render_widget_ref(&popup, frame.area());
    })?;

    let events = tui.event_stream();
    tokio::pin!(events);
    while popup.decision.is_none() {
        if let Some(event) = events.next().await {
            match event {
                TuiEvent::Key(key_event) => popup.handle_key_event(key_event),
                TuiEvent::Draw => {
                    let _ = tui.draw(u16::MAX, |frame| {
                        frame.render_widget_ref(&popup, frame.area());
                    });
                }
                _ => {}
            }
        } else {
            break;
        }
    }

    Ok(popup.decision.unwrap_or(ModelUpgradeDecision::KeepCurrent))
}
