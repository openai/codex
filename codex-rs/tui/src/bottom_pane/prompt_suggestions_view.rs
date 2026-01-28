use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use ratatui::buffer::Buffer;
use ratatui::layout::Constraint;
use ratatui::layout::Layout;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Block;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;
use textwrap::wrap;

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::key_hint;
use crate::render::Insets;
use crate::render::RectExt as _;
use crate::render::renderable::Renderable;
use crate::style::user_message_style;
use codex_core::features::Feature;
use codex_core::protocol::PromptSuggestionContext;
use codex_core::protocol::PromptSuggestionEvent;
use codex_core::protocol::PromptSuggestionOrigin;

use super::CancellationEvent;
use super::bottom_pane_view::BottomPaneView;

pub(crate) struct PromptSuggestionsView {
    suggestion: Option<String>,
    origin: PromptSuggestionOrigin,
    context: PromptSuggestionContext,
    enabled: bool,
    auto_run_enabled: bool,
    app_event_tx: AppEventSender,
    complete: bool,
}

impl PromptSuggestionsView {
    pub(crate) fn new(
        suggestion: Option<PromptSuggestionEvent>,
        enabled: bool,
        auto_run_enabled: bool,
        app_event_tx: AppEventSender,
    ) -> Self {
        let (suggestion, origin, context) = match suggestion {
            Some(event) => (Some(event.suggestion), event.origin, event.context),
            None => (
                None,
                PromptSuggestionOrigin::Unknown,
                PromptSuggestionContext::Unknown,
            ),
        };
        Self {
            suggestion,
            origin,
            context,
            enabled,
            auto_run_enabled,
            app_event_tx,
            complete: false,
        }
    }

    fn suggestion_text(&self) -> Option<&str> {
        self.suggestion
            .as_deref()
            .map(str::trim)
            .filter(|text| !text.is_empty())
    }

    fn toggle_enabled(&mut self) {
        self.enabled = !self.enabled;
        let mut updates = vec![(Feature::PromptSuggestions, self.enabled)];
        if !self.enabled && self.auto_run_enabled {
            self.auto_run_enabled = false;
            updates.push((Feature::PromptSuggestionsAutorun, false));
        }
        self.app_event_tx
            .send(AppEvent::UpdateFeatureFlags { updates });
    }

    fn toggle_auto_run(&mut self) {
        let mut updates = Vec::new();
        if !self.enabled {
            self.enabled = true;
            updates.push((Feature::PromptSuggestions, true));
        }
        self.auto_run_enabled = !self.auto_run_enabled;
        updates.push((Feature::PromptSuggestionsAutorun, self.auto_run_enabled));
        self.app_event_tx
            .send(AppEvent::UpdateFeatureFlags { updates });
    }

    fn wrapped_lines(text: &str, width: u16, indent: &'static str) -> Vec<Line<'static>> {
        let wrap_width = width.max(1) as usize;
        wrap(text, wrap_width)
            .into_iter()
            .map(|line| {
                let text = line.into_owned();
                Line::from(vec![indent.into(), text.into()])
            })
            .collect()
    }

    fn header_lines(&self) -> Vec<Line<'static>> {
        let status = if self.enabled {
            "On".green()
        } else {
            "Off".red()
        };
        let auto_run = if self.auto_run_enabled {
            "On".green()
        } else {
            "Off".red()
        };
        vec![
            Line::from("Prompt suggestions".bold()),
            Line::from(vec!["Status: ".dim(), status]),
            Line::from(vec!["Auto-run: ".dim(), auto_run]),
        ]
    }

    fn metadata_lines(&self) -> Vec<Line<'static>> {
        let origin = match self.origin {
            PromptSuggestionOrigin::Llm => "LLM",
            PromptSuggestionOrigin::Unknown => "Unknown",
        };
        let context = match &self.context {
            PromptSuggestionContext::LastAssistant => "Last assistant response".to_string(),
            PromptSuggestionContext::History { depth } => {
                format!("History (last {depth} user turns)")
            }
            PromptSuggestionContext::Unknown => "Unknown".to_string(),
        };
        vec![
            Line::from(vec!["Origin: ".dim(), origin.into()]),
            Line::from(vec!["Context: ".dim(), context.into()]),
        ]
    }

    fn content_lines(&self, width: u16) -> Vec<Line<'static>> {
        let mut lines = self.header_lines();
        lines.extend(self.metadata_lines());
        lines.push(Line::from(""));

        if !self.enabled {
            lines.push("Suggestions are off.".italic().into());
            lines.push(
                Line::from(vec![
                    "Press ".into(),
                    key_hint::plain(KeyCode::Char('t')).into(),
                    " to enable.".into(),
                ])
                .dim(),
            );
            return lines;
        }

        if let Some(text) = self.suggestion_text() {
            lines.push("Suggested reply".bold().into());
            lines.push(Line::from(""));
            lines.extend(Self::wrapped_lines(text, width.saturating_sub(2), "  "));
            if self.auto_run_enabled {
                lines.push(Line::from(""));
                lines.push(
                    "Auto-run will submit the suggestion when the composer is idle."
                        .dim()
                        .into(),
                );
            }
        } else {
            lines.push("Waiting for a suggestion...".italic().into());
            lines.push(
                "Send another message or wait for the next response."
                    .dim()
                    .into(),
            );
        }

        lines
    }
}

impl BottomPaneView for PromptSuggestionsView {
    fn handle_key_event(&mut self, key_event: KeyEvent) {
        match key_event {
            KeyEvent {
                code: KeyCode::Enter,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                if let Some(text) = self.suggestion_text() {
                    self.app_event_tx
                        .send(AppEvent::SubmitPromptSuggestion(text.to_string()));
                    self.complete = true;
                }
            }
            KeyEvent {
                code: KeyCode::Tab,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                if let Some(text) = self.suggestion_text() {
                    self.app_event_tx
                        .send(AppEvent::PrefillPromptSuggestion(text.to_string()));
                    self.complete = true;
                }
            }
            KeyEvent {
                code: KeyCode::Char('t'),
                modifiers: KeyModifiers::NONE,
                ..
            }
            | KeyEvent {
                code: KeyCode::Char('T'),
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                self.toggle_enabled();
            }
            KeyEvent {
                code: KeyCode::Char('a'),
                modifiers: KeyModifiers::NONE,
                ..
            }
            | KeyEvent {
                code: KeyCode::Char('A'),
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                self.toggle_auto_run();
            }
            KeyEvent {
                code: KeyCode::Esc, ..
            } => {
                self.on_ctrl_c();
            }
            _ => {}
        }
    }

    fn is_complete(&self) -> bool {
        self.complete
    }

    fn on_ctrl_c(&mut self) -> CancellationEvent {
        self.complete = true;
        CancellationEvent::Handled
    }
}

impl Renderable for PromptSuggestionsView {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 || area.width == 0 {
            return;
        }

        let [content_area, footer_area] =
            Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).areas(area);

        Block::default()
            .style(user_message_style())
            .render(content_area, buf);

        let inner = content_area.inset(Insets::vh(1, 2));
        let lines = self.content_lines(inner.width);

        Paragraph::new(lines).render(inner, buf);

        let hint_area = Rect {
            x: footer_area.x + 2,
            y: footer_area.y,
            width: footer_area.width.saturating_sub(2),
            height: footer_area.height,
        };
        let has_suggestion = self.suggestion_text().is_some();
        prompt_suggestions_hint_line(self.enabled, has_suggestion)
            .dim()
            .render(hint_area, buf);
    }

    fn desired_height(&self, width: u16) -> u16 {
        if width == 0 {
            return 0;
        }
        let content_lines = self.content_lines(width.saturating_sub(4)).len() as u16;
        let padding = 2;
        let footer = 1;
        content_lines + padding + footer
    }
}

fn prompt_suggestions_hint_line(enabled: bool, has_suggestion: bool) -> Line<'static> {
    let mut spans = Vec::new();
    if enabled && has_suggestion {
        spans.push(key_hint::plain(KeyCode::Enter).into());
        spans.push(" send / ".into());
        spans.push(key_hint::plain(KeyCode::Tab).into());
        spans.push(" edit / ".into());
    }
    spans.push(key_hint::plain(KeyCode::Char('t')).into());
    spans.push(if enabled { " toggle / " } else { " enable / " }.into());
    spans.push(key_hint::plain(KeyCode::Char('a')).into());
    spans.push(" auto-run / ".into());
    spans.push(key_hint::plain(KeyCode::Esc).into());
    spans.push(" dismiss".into());
    Line::from(spans)
}
