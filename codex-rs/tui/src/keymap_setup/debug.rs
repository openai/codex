use codex_config::types::TuiKeymap;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;
use std::time::Duration;
use std::time::Instant;

use crate::bottom_pane::BottomPaneView;
use crate::bottom_pane::CancellationEvent;
use crate::key_hint::KeyBinding;
use crate::keymap::RuntimeKeymap;
use crate::render::renderable::Renderable;

use super::actions;
use super::actions::matching_actions_for_key_event;
use super::key_event_to_config_key_spec;

const MISSING_KEY_HINT_DELAY: Duration = Duration::from_secs(3);
const SHORT_MISSING_KEY_HINT: &str = "Tip: Codex can only inspect keys your terminal sends.";
const DELAYED_MISSING_KEY_HINT: &str = "Still waiting? If nothing changes when you press a key, your terminal is not sending that key to Codex. Only received keys can be assigned as shortcuts.";

struct KeymapDebugReport {
    detected: KeyBinding,
    config_key: Result<String, String>,
    raw_event: String,
    matches: Vec<actions::KeymapDebugActionMatch>,
}

/// Bottom-pane view for inspecting how terminal key events map to keymap actions.
pub(crate) struct KeymapDebugView {
    runtime_keymap: RuntimeKeymap,
    keymap_config: TuiKeymap,
    opened_at: Instant,
    last_report: Option<KeymapDebugReport>,
    complete: bool,
}

pub(crate) fn build_keymap_debug_view(
    runtime_keymap: &RuntimeKeymap,
    keymap_config: &TuiKeymap,
) -> KeymapDebugView {
    KeymapDebugView {
        runtime_keymap: runtime_keymap.clone(),
        keymap_config: keymap_config.clone(),
        opened_at: Instant::now(),
        last_report: None,
        complete: false,
    }
}

impl KeymapDebugView {
    fn lines(&self, width: u16) -> Vec<Line<'static>> {
        self.lines_at(width, Instant::now())
    }

    fn lines_at(&self, width: u16, now: Instant) -> Vec<Line<'static>> {
        let wrap_width = usize::from(width.max(1));
        let mut lines = vec![
            Line::from("Keypress Inspector".bold()),
            Line::from(
                "Press any key to see what Codex receives. Esc is inspected; Ctrl+C closes.".dim(),
            ),
        ];
        let hint = if self.should_show_delayed_hint(now) {
            DELAYED_MISSING_KEY_HINT
        } else {
            SHORT_MISSING_KEY_HINT
        };
        push_wrapped_dim(&mut lines, hint.to_string(), wrap_width, "", "");

        let Some(report) = &self.last_report else {
            lines.push(Line::from(""));
            lines.push(Line::from("Waiting for a keypress...".cyan()));
            return lines;
        };

        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            "Detected: ".dim(),
            report.detected.display_label().cyan(),
        ]));
        match &report.config_key {
            Ok(config_key) => {
                lines.push(Line::from(vec![
                    "Config key: ".dim(),
                    config_key.clone().cyan(),
                ]));
            }
            Err(error) => {
                push_wrapped_dim(
                    &mut lines,
                    format!("unsupported - {error}"),
                    wrap_width,
                    "Config key: ",
                    "            ",
                );
            }
        }
        push_wrapped_dim(
            &mut lines,
            report.raw_event.clone(),
            wrap_width,
            "Raw event: ",
            "           ",
        );
        lines.push(Line::from(""));
        lines.push(Line::from("Assigned actions:".dim()));
        if report.matches.is_empty() {
            lines.push(Line::from("  none".dim()));
        } else {
            for matched_action in &report.matches {
                let action = format!(
                    "{}.{} ({}) - {} [{}]",
                    matched_action.context,
                    matched_action.action,
                    matched_action.label,
                    matched_action.description,
                    matched_action.source.label()
                );
                push_wrapped_dim(&mut lines, action, wrap_width, "  - ", "    ");
            }
        }
        lines
    }

    fn should_show_delayed_hint(&self, now: Instant) -> bool {
        self.last_report.is_none() && now.duration_since(self.opened_at) >= MISSING_KEY_HINT_DELAY
    }

    #[cfg(test)]
    pub(crate) fn show_delayed_hint_for_test(&mut self) {
        self.opened_at = Instant::now() - MISSING_KEY_HINT_DELAY;
    }
}

impl Renderable for KeymapDebugView {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        Paragraph::new(self.lines(area.width)).render(area, buf);
    }

    fn desired_height(&self, width: u16) -> u16 {
        self.lines(width).len() as u16
    }
}

impl BottomPaneView for KeymapDebugView {
    fn handle_key_event(&mut self, key_event: KeyEvent) {
        if key_event.kind == KeyEventKind::Release {
            return;
        }

        self.last_report = Some(KeymapDebugReport {
            detected: KeyBinding::from_event(key_event),
            config_key: key_event_to_config_key_spec(key_event),
            raw_event: key_event_debug_summary(key_event),
            matches: matching_actions_for_key_event(
                &self.runtime_keymap,
                &self.keymap_config,
                key_event,
            ),
        });
    }

    fn is_complete(&self) -> bool {
        self.complete
    }

    fn on_ctrl_c(&mut self) -> CancellationEvent {
        self.complete = true;
        CancellationEvent::Handled
    }

    fn prefer_esc_to_handle_key_event(&self) -> bool {
        true
    }

    fn next_frame_delay(&self) -> Option<Duration> {
        if self.last_report.is_some() {
            return None;
        }

        self.opened_at
            .checked_add(MISSING_KEY_HINT_DELAY)
            .and_then(|show_at| show_at.checked_duration_since(Instant::now()))
            .filter(|delay| !delay.is_zero())
    }
}

fn push_wrapped_dim(
    lines: &mut Vec<Line<'static>>,
    text: String,
    wrap_width: usize,
    initial_indent: &'static str,
    subsequent_indent: &'static str,
) {
    let options = textwrap::Options::new(wrap_width)
        .initial_indent(initial_indent)
        .subsequent_indent(subsequent_indent);
    lines.extend(
        textwrap::wrap(&text, options)
            .into_iter()
            .map(|line| Line::from(line.into_owned().dim())),
    );
}

fn key_event_debug_summary(key_event: KeyEvent) -> String {
    format!(
        "code={:?}, modifiers={}, kind={:?}",
        key_event.code,
        key_modifiers_debug_label(key_event.modifiers),
        key_event.kind
    )
}

fn key_modifiers_debug_label(modifiers: KeyModifiers) -> String {
    if modifiers.is_empty() {
        return "none".to_string();
    }

    let mut parts = Vec::new();
    if modifiers.contains(KeyModifiers::CONTROL) {
        parts.push("ctrl".to_string());
    }
    if modifiers.contains(KeyModifiers::ALT) {
        parts.push("alt".to_string());
    }
    if modifiers.contains(KeyModifiers::SHIFT) {
        parts.push("shift".to_string());
    }

    let known_modifiers = KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SHIFT;
    let other_modifiers = modifiers.difference(known_modifiers);
    if !other_modifiers.is_empty() {
        parts.push(format!("{other_modifiers:?}"));
    }
    parts.join("|")
}
