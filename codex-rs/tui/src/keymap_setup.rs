//! Guided keymap remapping UI for `/keymap`.
//!
//! Pick an action, choose whether to set or remove its root-level custom
//! binding, then validate and persist the resulting runtime keymap.

mod actions;
mod picker;

pub(crate) use picker::KEYMAP_PICKER_VIEW_ID;
pub(crate) use picker::build_keymap_picker_params;
pub(crate) use picker::build_keymap_picker_params_for_selected_action;

use codex_config::types::KeybindingSpec;
use codex_config::types::KeybindingsSpec;
use codex_config::types::TuiKeymap;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;

use crate::app_event::AppEvent;
use crate::app_event::KeymapEditIntent;
use crate::app_event_sender::AppEventSender;
use crate::bottom_pane::BottomPaneView;
use crate::bottom_pane::CancellationEvent;
use crate::bottom_pane::ColumnWidthMode;
use crate::bottom_pane::SelectionItem;
use crate::bottom_pane::SelectionViewParams;
use crate::bottom_pane::popup_consts::standard_popup_hint_line;
use crate::keymap::RuntimeKeymap;
use crate::render::renderable::ColumnRenderable;
use crate::render::renderable::Renderable;
use actions::KEYMAP_ACTIONS;
use actions::action_label;
use actions::binding_slot;
use actions::bindings_for_action;
use actions::format_binding_summary;

pub(crate) const KEYMAP_ACTION_MENU_VIEW_ID: &str = "keymap-action-menu";
pub(crate) const KEYMAP_REPLACE_BINDING_MENU_VIEW_ID: &str = "keymap-replace-binding-menu";

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum KeymapEditOutcome {
    Updated {
        keymap_config: Box<TuiKeymap>,
        bindings: Vec<String>,
        message: String,
    },
    Unchanged {
        message: String,
    },
}

fn key_binding_span(binding: &str) -> ratatui::text::Span<'static> {
    if binding == "unbound" {
        binding.to_string().dim()
    } else {
        binding.to_string().cyan()
    }
}

fn keymap_action_menu_hint_line() -> Line<'static> {
    Line::from(vec![
        "enter".cyan(),
        " select · ".dim(),
        "esc".cyan(),
        " back".dim(),
    ])
}

fn open_capture_action(
    context: String,
    action: String,
    intent: KeymapEditIntent,
) -> Box<dyn Fn(&AppEventSender) + Send + Sync> {
    Box::new(move |tx| {
        tx.send(AppEvent::OpenKeymapCapture {
            context: context.clone(),
            action: action.clone(),
            intent: intent.clone(),
        });
    })
}

fn action_menu_item(
    name: &str,
    description: &str,
    selected_description: String,
    context: &str,
    action: &str,
    intent: KeymapEditIntent,
) -> SelectionItem {
    SelectionItem {
        name: name.to_string(),
        description: Some(description.to_string()),
        selected_description: Some(selected_description),
        actions: vec![open_capture_action(
            context.to_string(),
            action.to_string(),
            intent,
        )],
        ..Default::default()
    }
}

pub(crate) fn build_keymap_action_menu_params(
    context: String,
    action: String,
    runtime_keymap: &RuntimeKeymap,
    keymap_config: &TuiKeymap,
) -> SelectionViewParams {
    let current_bindings =
        active_binding_specs(runtime_keymap, &context, &action).unwrap_or_else(|_| Vec::new());
    let current_binding = if current_bindings.is_empty() {
        "unbound".to_string()
    } else {
        current_bindings.join(", ")
    };
    let active_binding_count = current_bindings.len();
    let custom_binding = has_custom_binding(keymap_config, &context, &action).unwrap_or(false);
    let descriptor = KEYMAP_ACTIONS
        .iter()
        .find(|descriptor| descriptor.context == context && descriptor.action == action);
    let context_label = descriptor
        .map(|descriptor| descriptor.context_label)
        .unwrap_or(context.as_str())
        .to_string();
    let description = descriptor
        .map(|descriptor| descriptor.description)
        .unwrap_or("Configure this shortcut.");
    let remove_disabled_reason = (!custom_binding)
        .then(|| "There is no custom root binding for this action to remove.".to_string());
    let label = action_label(&action);
    let remove_context = context.clone();
    let remove_action = action.clone();
    let config_path = format!("tui.keymap.{context}.{action}");
    let source = if custom_binding {
        "Custom root override".cyan()
    } else {
        "Default keymap".dim()
    };
    let mut header = ColumnRenderable::new();
    header.push(Line::from("Edit Shortcut".bold()));
    header.push(Line::from(vec![
        label.bold(),
        " · ".dim(),
        context_label.dim(),
    ]));
    header.push(Line::from(vec![
        "Current ".dim(),
        key_binding_span(&current_binding),
        " · ".dim(),
        source,
    ]));
    header.push(Line::from(vec![
        "Config ".dim(),
        format!("`{config_path}`").cyan(),
    ]));
    header.push(Line::from(description.to_string().dim()));

    let mut items = Vec::new();
    match active_binding_count {
        0 => {
            items.push(action_menu_item(
                "Set key",
                "Capture a key for this unbound action.",
                "Capture one key and bind this action.".to_string(),
                &context,
                &action,
                KeymapEditIntent::ReplaceAll,
            ));
        }
        1 => {
            items.push(action_menu_item(
                "Replace binding",
                "Capture a replacement key.",
                format!("Capture one key and replace `{current_binding}`."),
                &context,
                &action,
                KeymapEditIntent::ReplaceAll,
            ));
            items.push(action_menu_item(
                "Add alternate binding",
                "Keep the current binding and add another key.",
                format!("Capture one key and keep `{current_binding}` as an alternate."),
                &context,
                &action,
                KeymapEditIntent::AddAlternate,
            ));
        }
        _ => {
            let replace_one_context = context.clone();
            let replace_one_action = action.clone();
            items.push(SelectionItem {
                name: "Replace one binding...".to_string(),
                description: Some("Choose which existing binding to replace.".to_string()),
                selected_description: Some(
                    "Pick one current binding, then capture its replacement.".to_string(),
                ),
                actions: vec![Box::new(move |tx| {
                    tx.send(AppEvent::OpenKeymapReplaceBindingMenu {
                        context: replace_one_context.clone(),
                        action: replace_one_action.clone(),
                    });
                })],
                ..Default::default()
            });
            items.push(action_menu_item(
                "Replace all bindings",
                "Replace every current binding with one key.",
                format!("Capture one key and replace `{current_binding}`."),
                &context,
                &action,
                KeymapEditIntent::ReplaceAll,
            ));
            items.push(action_menu_item(
                "Add alternate binding",
                "Keep current bindings and add another key.",
                format!("Capture one key and keep `{current_binding}`."),
                &context,
                &action,
                KeymapEditIntent::AddAlternate,
            ));
        }
    }
    items.push(SelectionItem {
        name: "Remove custom binding".to_string(),
        description: Some(if custom_binding {
            "Restore the default keymap binding.".to_string()
        } else {
            "No root override to remove.".to_string()
        }),
        selected_description: Some(
            "Delete the root override and use the default keymap again.".to_string(),
        ),
        disabled_reason: remove_disabled_reason,
        actions: vec![Box::new(move |tx| {
            tx.send(AppEvent::KeymapCleared {
                context: remove_context.clone(),
                action: remove_action.clone(),
            });
        })],
        ..Default::default()
    });
    items.push(SelectionItem {
        name: "Back to shortcuts".to_string(),
        description: Some("Return to the shortcut list.".to_string()),
        dismiss_on_select: true,
        ..Default::default()
    });

    SelectionViewParams {
        view_id: Some(KEYMAP_ACTION_MENU_VIEW_ID),
        header: Box::new(header),
        footer_note: Some(Line::from(vec![
            "Changes write the root ".dim(),
            "`tui.keymap.*`".cyan(),
            " override.".dim(),
        ])),
        footer_hint: Some(keymap_action_menu_hint_line()),
        items,
        col_width_mode: ColumnWidthMode::Fixed,
        ..Default::default()
    }
}

pub(crate) fn build_keymap_replace_binding_menu_params(
    context: String,
    action: String,
    runtime_keymap: &RuntimeKeymap,
) -> SelectionViewParams {
    let bindings = active_binding_specs(runtime_keymap, &context, &action).unwrap_or_default();
    let label = action_label(&action);
    let mut header = ColumnRenderable::new();
    header.push(Line::from("Replace Binding".bold()));
    header.push(Line::from(vec![
        label.bold(),
        " · ".dim(),
        format!("{context}.{action}").dim(),
    ]));
    header.push(Line::from("Choose the binding to replace.".dim()));

    let items = bindings
        .into_iter()
        .map(|binding| {
            let capture_context = context.clone();
            let capture_action = action.clone();
            let old_key = binding.clone();
            SelectionItem {
                name: binding.clone(),
                description: Some("Replace this binding.".to_string()),
                selected_description: Some(format!("Capture a new key to replace `{binding}`.")),
                actions: vec![Box::new(move |tx| {
                    tx.send(AppEvent::OpenKeymapCapture {
                        context: capture_context.clone(),
                        action: capture_action.clone(),
                        intent: KeymapEditIntent::ReplaceOne {
                            old_key: old_key.clone(),
                        },
                    });
                })],
                dismiss_on_select: true,
                ..Default::default()
            }
        })
        .collect();

    SelectionViewParams {
        view_id: Some(KEYMAP_REPLACE_BINDING_MENU_VIEW_ID),
        header: Box::new(header),
        footer_hint: Some(keymap_action_menu_hint_line()),
        items,
        col_width_mode: ColumnWidthMode::Fixed,
        ..Default::default()
    }
}

pub(crate) fn build_keymap_conflict_params(
    context: String,
    action: String,
    key: String,
    intent: KeymapEditIntent,
    error: String,
) -> SelectionViewParams {
    let retry_context = context.clone();
    let retry_action = action.clone();
    let retry_intent = intent;
    SelectionViewParams {
        title: Some("Shortcut Conflict".to_string()),
        subtitle: Some(format!("{context}.{action} cannot use `{key}`.")),
        footer_note: Some(Line::from(error)),
        footer_hint: Some(standard_popup_hint_line()),
        items: vec![
            SelectionItem {
                name: "Pick another key".to_string(),
                description: Some("Return to key capture for this action.".to_string()),
                actions: vec![Box::new(move |tx| {
                    tx.send(AppEvent::OpenKeymapCapture {
                        context: retry_context.clone(),
                        action: retry_action.clone(),
                        intent: retry_intent.clone(),
                    });
                })],
                dismiss_on_select: true,
                ..Default::default()
            },
            SelectionItem {
                name: "Cancel".to_string(),
                description: Some("Leave keymap unchanged.".to_string()),
                dismiss_on_select: true,
                ..Default::default()
            },
        ],
        col_width_mode: ColumnWidthMode::Fixed,
        ..Default::default()
    }
}

pub(crate) fn build_keymap_capture_view(
    context: String,
    action: String,
    intent: KeymapEditIntent,
    runtime_keymap: &RuntimeKeymap,
    app_event_tx: AppEventSender,
) -> KeymapCaptureView {
    let current_binding = format_binding_summary(
        bindings_for_action(runtime_keymap, &context, &action).unwrap_or(&[]),
    );
    let label = action_label(&action);
    KeymapCaptureView::new(
        context,
        action,
        intent,
        label,
        current_binding,
        app_event_tx,
    )
}

#[cfg(test)]
fn keymap_with_replacement(
    keymap: &TuiKeymap,
    context: &str,
    action: &str,
    key: &str,
) -> Result<TuiKeymap, String> {
    keymap_with_bindings(keymap, context, action, &[key.to_string()])
}

pub(crate) fn keymap_with_edit(
    keymap: &TuiKeymap,
    runtime_keymap: &RuntimeKeymap,
    context: &str,
    action: &str,
    key: &str,
    intent: &KeymapEditIntent,
) -> Result<KeymapEditOutcome, String> {
    let current_bindings = active_binding_specs(runtime_keymap, context, action)?;
    let next_bindings = match intent {
        KeymapEditIntent::ReplaceAll => vec![key.to_string()],
        KeymapEditIntent::AddAlternate => {
            if current_bindings.iter().any(|binding| binding == key) {
                return Ok(KeymapEditOutcome::Unchanged {
                    message: format!("No change: `{context}.{action}` already uses `{key}`."),
                });
            }
            let mut bindings = current_bindings.clone();
            bindings.push(key.to_string());
            bindings
        }
        KeymapEditIntent::ReplaceOne { old_key } => {
            if !current_bindings.iter().any(|binding| binding == old_key) {
                return Err(format!(
                    "`{context}.{action}` no longer uses `{old_key}`. Reopen /keymap and choose a binding again."
                ));
            }
            let bindings = current_bindings
                .iter()
                .map(|binding| {
                    if binding == old_key {
                        key.to_string()
                    } else {
                        binding.clone()
                    }
                })
                .collect::<Vec<_>>();
            dedup_bindings(bindings)
        }
    };

    if next_bindings == current_bindings {
        return Ok(KeymapEditOutcome::Unchanged {
            message: format!("No change: `{context}.{action}` already uses `{key}`."),
        });
    }

    let message = match intent {
        KeymapEditIntent::ReplaceAll => format!("Remapped `{context}.{action}` to `{key}`."),
        KeymapEditIntent::AddAlternate => format!("Added `{key}` to `{context}.{action}`."),
        KeymapEditIntent::ReplaceOne { old_key } => {
            format!("Replaced `{old_key}` with `{key}` for `{context}.{action}`.")
        }
    };

    Ok(KeymapEditOutcome::Updated {
        keymap_config: Box::new(keymap_with_bindings(
            keymap,
            context,
            action,
            &next_bindings,
        )?),
        bindings: next_bindings,
        message,
    })
}

fn keymap_with_bindings(
    keymap: &TuiKeymap,
    context: &str,
    action: &str,
    keys: &[String],
) -> Result<TuiKeymap, String> {
    let mut keymap = keymap.clone();
    let slot = binding_slot(&mut keymap, context, action).ok_or_else(|| {
        format!("Unknown keymap action `{context}.{action}`. Reopen /keymap and choose an action.")
    })?;
    *slot = Some(match keys {
        [key] => KeybindingsSpec::One(KeybindingSpec(key.clone())),
        keys => KeybindingsSpec::Many(
            keys.iter()
                .map(|key| KeybindingSpec(key.clone()))
                .collect::<Vec<_>>(),
        ),
    });
    Ok(keymap)
}

pub(crate) fn active_binding_specs(
    runtime_keymap: &RuntimeKeymap,
    context: &str,
    action: &str,
) -> Result<Vec<String>, String> {
    let bindings = bindings_for_action(runtime_keymap, context, action).ok_or_else(|| {
        format!("Unknown keymap action `{context}.{action}`. Reopen /keymap and choose an action.")
    })?;
    bindings
        .iter()
        .map(|binding| binding_to_config_key_spec(*binding))
        .collect()
}

fn dedup_bindings(bindings: Vec<String>) -> Vec<String> {
    bindings.into_iter().fold(Vec::new(), |mut deduped, key| {
        if !deduped.contains(&key) {
            deduped.push(key);
        }
        deduped
    })
}

pub(crate) fn keymap_without_custom_binding(
    keymap: &TuiKeymap,
    context: &str,
    action: &str,
) -> Result<TuiKeymap, String> {
    let mut keymap = keymap.clone();
    let slot = binding_slot(&mut keymap, context, action).ok_or_else(|| {
        format!("Unknown keymap action `{context}.{action}`. Reopen /keymap and choose an action.")
    })?;
    *slot = None;
    Ok(keymap)
}

fn has_custom_binding(keymap: &TuiKeymap, context: &str, action: &str) -> Result<bool, String> {
    let mut keymap = keymap.clone();
    let slot = binding_slot(&mut keymap, context, action).ok_or_else(|| {
        format!("Unknown keymap action `{context}.{action}`. Reopen /keymap and choose an action.")
    })?;
    Ok(slot.is_some())
}

pub(crate) struct KeymapCaptureView {
    context: String,
    action: String,
    intent: KeymapEditIntent,
    label: String,
    current_binding: String,
    app_event_tx: AppEventSender,
    complete: bool,
    error_message: Option<String>,
}

impl KeymapCaptureView {
    fn new(
        context: String,
        action: String,
        intent: KeymapEditIntent,
        label: String,
        current_binding: String,
        app_event_tx: AppEventSender,
    ) -> Self {
        Self {
            context,
            action,
            intent,
            label,
            current_binding,
            app_event_tx,
            complete: false,
            error_message: None,
        }
    }

    fn lines(&self, width: u16) -> Vec<Line<'static>> {
        let wrap_width = usize::from(width.max(1));
        let mut lines = vec![
            Line::from("Remap Shortcut".bold()),
            Line::from(vec![
                "Action: ".dim(),
                self.label.clone().into(),
                "  ".into(),
                format!("{}.{}", self.context, self.action).dim(),
            ]),
            Line::from(vec!["Current: ".dim(), self.current_binding.clone().cyan()]),
            Line::from("Press the new key now. Esc cancels.".dim()),
        ];

        if let Some(error) = &self.error_message {
            lines.push(Line::from(""));
            let options = textwrap::Options::new(wrap_width)
                .initial_indent("Error: ")
                .subsequent_indent("       ");
            lines.extend(
                textwrap::wrap(error, options)
                    .into_iter()
                    .map(|line| Line::from(line.into_owned().red())),
            );
        }

        lines
    }
}

impl Renderable for KeymapCaptureView {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        Paragraph::new(self.lines(area.width)).render(area, buf);
    }

    fn desired_height(&self, width: u16) -> u16 {
        self.lines(width).len() as u16
    }
}

impl BottomPaneView for KeymapCaptureView {
    fn handle_key_event(&mut self, key_event: KeyEvent) {
        if key_event.kind == KeyEventKind::Release {
            return;
        }

        if key_event.code == KeyCode::Esc {
            self.complete = true;
            return;
        }

        match key_event_to_config_key_spec(key_event) {
            Ok(key) => {
                self.app_event_tx.send(AppEvent::KeymapCaptured {
                    context: self.context.clone(),
                    action: self.action.clone(),
                    key,
                    intent: self.intent.clone(),
                });
                self.complete = true;
            }
            Err(error) => {
                self.error_message = Some(error);
            }
        }
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
}

fn key_event_to_config_key_spec(key_event: KeyEvent) -> Result<String, String> {
    key_parts_to_config_key_spec(key_event.code, key_event.modifiers)
}

fn binding_to_config_key_spec(binding: crate::key_hint::KeyBinding) -> Result<String, String> {
    let (code, modifiers) = binding.parts();
    key_parts_to_config_key_spec(code, modifiers)
}

fn key_parts_to_config_key_spec(
    code: KeyCode,
    mut modifiers: KeyModifiers,
) -> Result<String, String> {
    let supported_modifiers = KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SHIFT;
    if !modifiers.difference(supported_modifiers).is_empty() {
        return Err(
            "Only ctrl, alt, and shift modifiers can be stored in `tui.keymap`.".to_string(),
        );
    }

    let key = match code {
        KeyCode::Enter => "enter".to_string(),
        KeyCode::Tab => "tab".to_string(),
        KeyCode::Backspace => "backspace".to_string(),
        KeyCode::Esc => "esc".to_string(),
        KeyCode::Delete => "delete".to_string(),
        KeyCode::Up => "up".to_string(),
        KeyCode::Down => "down".to_string(),
        KeyCode::Left => "left".to_string(),
        KeyCode::Right => "right".to_string(),
        KeyCode::Home => "home".to_string(),
        KeyCode::End => "end".to_string(),
        KeyCode::PageUp => "page-up".to_string(),
        KeyCode::PageDown => "page-down".to_string(),
        KeyCode::F(number) if (1..=12).contains(&number) => format!("f{number}"),
        KeyCode::F(_) => {
            return Err(
                "Only function keys F1 through F12 can be stored in `tui.keymap`.".to_string(),
            );
        }
        KeyCode::Char(' ') => "space".to_string(),
        KeyCode::Char(mut ch) => {
            if ch == '-' {
                return Err("The `-` key cannot be represented in `tui.keymap` yet.".to_string());
            }
            if !ch.is_ascii() || ch.is_ascii_control() {
                return Err("Only printable ASCII keys can be stored in `tui.keymap`.".to_string());
            }
            if ch.is_ascii_uppercase() {
                modifiers.insert(KeyModifiers::SHIFT);
                ch = ch.to_ascii_lowercase();
            }
            ch.to_string()
        }
        _ => {
            return Err("That key is not supported by `tui.keymap`.".to_string());
        }
    };

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
    parts.push(key);
    Ok(parts.join("-"))
}

#[cfg(test)]
mod tests {
    use super::picker::KEYMAP_ALL_TAB_ID;
    use super::picker::KEYMAP_COMMON_TAB_ID;
    use super::picker::KEYMAP_CUSTOM_TAB_ID;
    use super::picker::KEYMAP_UNBOUND_TAB_ID;
    use super::*;
    use crate::bottom_pane::BottomPane;
    use crate::bottom_pane::BottomPaneParams;
    use crate::bottom_pane::ListSelectionView;
    use crate::bottom_pane::SelectionTab;
    use crate::tui::FrameRequester;
    use insta::assert_snapshot;
    use pretty_assertions::assert_eq;
    use ratatui::buffer::Buffer;
    use tokio::sync::mpsc::UnboundedReceiver;
    use tokio::sync::mpsc::unbounded_channel;

    fn app_event_sender() -> AppEventSender {
        let (tx, _rx) = unbounded_channel();
        AppEventSender::new(tx)
    }

    fn render_capture(view: &KeymapCaptureView, width: u16, height: u16) -> Buffer {
        let area = Rect::new(0, 0, width, height);
        let mut buf = Buffer::empty(area);
        view.render(area, &mut buf);
        buf
    }

    fn render_picker(params: SelectionViewParams, width: u16) -> String {
        let view =
            ListSelectionView::new(params, app_event_sender(), RuntimeKeymap::defaults().list);
        render_picker_from_view(&view, width)
    }

    fn render_picker_from_view(view: &ListSelectionView, width: u16) -> String {
        let height = view.desired_height(width);
        let area = Rect::new(0, 0, width, height);
        let mut buf = Buffer::empty(area);
        view.render(area, &mut buf);
        render_buffer(&buf)
    }

    fn render_buffer(buf: &Buffer) -> String {
        let area = buf.area();
        (0..area.height)
            .map(|row| {
                let mut line = String::new();
                for col in 0..area.width {
                    let symbol = buf[(col, row)].symbol();
                    if symbol.is_empty() {
                        line.push(' ');
                    } else {
                        line.push_str(symbol);
                    }
                }
                line.trim_end().to_string()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn test_pane() -> (BottomPane, AppEventSender, UnboundedReceiver<AppEvent>) {
        let (tx_raw, rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let pane = BottomPane::new(BottomPaneParams {
            app_event_tx: tx.clone(),
            frame_requester: FrameRequester::test_dummy(),
            has_input_focus: true,
            enhanced_keys_supported: false,
            placeholder_text: "Ask Codex to do anything".to_string(),
            disable_paste_burst: false,
            animations_enabled: false,
            skills: Some(Vec::new()),
        });
        (pane, tx, rx)
    }

    fn selection_tab<'a>(params: &'a SelectionViewParams, id: &str) -> &'a SelectionTab {
        params
            .tabs
            .iter()
            .find(|tab| tab.id == id)
            .expect("selection tab")
    }

    fn selection_item<'a>(params: &'a SelectionViewParams, name: &str) -> &'a SelectionItem {
        params
            .items
            .iter()
            .find(|item| item.name == name)
            .expect("selection item")
    }

    fn action_menu_rows(params: &SelectionViewParams) -> String {
        params
            .items
            .iter()
            .map(|item| {
                format!(
                    "{} | {} | {}",
                    item.name,
                    item.description.as_deref().unwrap_or_default(),
                    item.disabled_reason.as_deref().unwrap_or("enabled")
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn picker_covers_every_replaceable_action() {
        let runtime = RuntimeKeymap::defaults();
        let params = build_keymap_picker_params(&runtime, &TuiKeymap::default());
        let all_tab = selection_tab(&params, KEYMAP_ALL_TAB_ID);

        assert!(params.items.is_empty());
        assert_eq!(all_tab.items.len(), KEYMAP_ACTIONS.len());
        assert!(
            all_tab.items.iter().all(|item| !item.dismiss_on_select),
            "keymap picker should stay open behind the action menu"
        );
        assert!(KEYMAP_ACTIONS.iter().all(|descriptor| {
            binding_slot(
                &mut TuiKeymap::default(),
                descriptor.context,
                descriptor.action,
            )
            .is_some()
        }));
        assert!(KEYMAP_ACTIONS.iter().all(|descriptor| {
            bindings_for_action(&runtime, descriptor.context, descriptor.action).is_some()
        }));
    }

    #[test]
    fn picker_common_tab_lists_curated_actions() {
        let runtime = RuntimeKeymap::defaults();
        let params = build_keymap_picker_params(&runtime, &TuiKeymap::default());
        let common_tab = selection_tab(&params, KEYMAP_COMMON_TAB_ID);
        let actions = common_tab
            .items
            .iter()
            .map(|item| {
                item.search_value
                    .as_deref()
                    .unwrap_or_default()
                    .split_whitespace()
                    .take(2)
                    .collect::<Vec<_>>()
                    .join(".")
            })
            .collect::<Vec<_>>();

        assert_eq!(
            actions,
            vec![
                "Composer.submit",
                "Editor.insert_newline",
                "Composer.queue",
                "Global.open_external_editor",
                "Global.copy",
                "Editor.delete_backward_word",
                "Editor.delete_forward_word",
                "Editor.move_word_left",
                "Editor.move_word_right",
                "Global.open_transcript",
                "Pager.close",
                "Pager.page_up",
                "Pager.page_down",
                "Approval.open_fullscreen",
                "Approval.approve",
                "Approval.approve_for_session",
                "Approval.decline",
                "Approval.cancel",
            ]
        );
    }

    #[test]
    fn picker_approval_tab_lists_all_approval_actions() {
        let runtime = RuntimeKeymap::defaults();
        let params = build_keymap_picker_params(&runtime, &TuiKeymap::default());
        let approval_tab = selection_tab(&params, "approval-shortcuts");
        let actions = approval_tab
            .items
            .iter()
            .map(|item| {
                item.search_value
                    .as_deref()
                    .unwrap_or_default()
                    .split_whitespace()
                    .take(2)
                    .collect::<Vec<_>>()
                    .join(".")
            })
            .collect::<Vec<_>>();

        assert_eq!(
            actions,
            vec![
                "Approval.open_fullscreen",
                "Approval.open_thread",
                "Approval.approve",
                "Approval.approve_for_session",
                "Approval.approve_for_prefix",
                "Approval.deny",
                "Approval.decline",
                "Approval.cancel",
            ]
        );
    }

    #[test]
    fn picker_content_snapshot() {
        let runtime = RuntimeKeymap::defaults();
        let params = build_keymap_picker_params(&runtime, &TuiKeymap::default());
        let all_tab = selection_tab(&params, KEYMAP_ALL_TAB_ID);
        let snapshot = params
            .tabs
            .iter()
            .map(|tab| {
                let selectable = tab.items.iter().filter(|item| !item.is_disabled).count();
                format!("tab: {} ({selectable} selectable)", tab.label)
            })
            .chain(all_tab.items.iter().take(12).map(|item| {
                format!(
                    "{} | {} | {}",
                    item.name,
                    item.description.as_deref().unwrap_or_default(),
                    item.search_value.as_deref().unwrap_or_default()
                )
            }))
            .collect::<Vec<_>>()
            .join("\n");

        assert_snapshot!("keymap_picker_first_actions", snapshot);
    }

    #[test]
    fn picker_customized_tab_contains_root_overrides() {
        let keymap =
            keymap_with_replacement(&TuiKeymap::default(), "composer", "submit", "ctrl-enter")
                .expect("replace binding");
        let runtime = RuntimeKeymap::from_config(&keymap).expect("runtime keymap");
        let params = build_keymap_picker_params(&runtime, &keymap);
        let custom_tab = selection_tab(&params, KEYMAP_CUSTOM_TAB_ID);
        let composer_tab = selection_tab(&params, "composer-shortcuts");

        assert_eq!(
            custom_tab
                .items
                .iter()
                .map(|item| item.name.as_str())
                .collect::<Vec<_>>(),
            vec!["Submit"]
        );
        assert!(
            composer_tab
                .items
                .iter()
                .any(|item| item.description.as_deref() == Some("ctrl-enter"))
        );
    }

    #[test]
    fn picker_unbound_tab_lists_default_unbound_actions() {
        let runtime = RuntimeKeymap::defaults();
        let params = build_keymap_picker_params(&runtime, &TuiKeymap::default());
        let unbound_tab = selection_tab(&params, KEYMAP_UNBOUND_TAB_ID);

        assert_eq!(unbound_tab.items.len(), 1);
        assert_eq!(unbound_tab.items[0].name, "No unbound shortcuts");
        assert!(unbound_tab.items[0].is_disabled);
    }

    #[test]
    fn picker_selected_action_starts_on_matching_all_tab_row() {
        let runtime = RuntimeKeymap::defaults();
        let params = build_keymap_picker_params_for_selected_action(
            &runtime,
            &TuiKeymap::default(),
            "composer",
            "submit",
        );
        let all_tab = selection_tab(&params, KEYMAP_ALL_TAB_ID);

        assert_eq!(params.initial_tab_id.as_deref(), Some(KEYMAP_ALL_TAB_ID));
        assert_eq!(
            params.initial_selected_idx,
            all_tab.items.iter().position(|item| item.name == "Submit")
        );
    }

    #[test]
    fn picker_all_tab_items_remain_searchable() {
        let runtime = RuntimeKeymap::defaults();
        let params = build_keymap_picker_params(&runtime, &TuiKeymap::default());
        let all_tab = selection_tab(&params, KEYMAP_ALL_TAB_ID);
        let snapshot = all_tab
            .items
            .iter()
            .take(12)
            .map(|item| {
                format!(
                    "{} | {} | {}",
                    item.name,
                    item.description.as_deref().unwrap_or_default(),
                    item.search_value.as_deref().unwrap_or_default()
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        assert_snapshot!("keymap_picker_all_tab_search", snapshot);
    }

    #[test]
    fn picker_wide_render_snapshot() {
        let runtime = RuntimeKeymap::defaults();
        let params = build_keymap_picker_params(&runtime, &TuiKeymap::default());

        assert_snapshot!("keymap_picker_wide", render_picker(params, /*width*/ 120));
    }

    #[test]
    fn picker_narrow_render_snapshot() {
        let runtime = RuntimeKeymap::defaults();
        let params = build_keymap_picker_params(&runtime, &TuiKeymap::default());

        assert_snapshot!("keymap_picker_narrow", render_picker(params, /*width*/ 78));
    }

    #[test]
    fn picker_custom_render_snapshot() {
        let keymap =
            keymap_with_replacement(&TuiKeymap::default(), "composer", "submit", "ctrl-enter")
                .expect("replace binding");
        let runtime = RuntimeKeymap::from_config(&keymap).expect("runtime keymap");
        let params = build_keymap_picker_params(&runtime, &keymap);

        assert_snapshot!("keymap_picker_custom", render_picker(params, /*width*/ 120));
    }

    #[test]
    fn picker_narrow_uses_compact_tabs() {
        let runtime = RuntimeKeymap::defaults();
        let params = build_keymap_picker_params(&runtime, &TuiKeymap::default());
        let rendered = render_picker(params, /*width*/ 78);

        assert!(rendered.contains("Keymap"));
        assert!(rendered.contains("Open Transcript"));
        assert!(rendered.contains("ctrl-t"));
        assert!(!rendered.contains("Selected Action"));
        assert!(!rendered.contains("Source: default keymap"));
    }

    #[test]
    fn action_menu_content_snapshot() {
        let unbound_keymap = keymap_with_bindings(&TuiKeymap::default(), "global", "copy", &[])
            .expect("unbound copy");
        let unbound_runtime = RuntimeKeymap::from_config(&unbound_keymap).expect("runtime keymap");
        let unbound_params = build_keymap_action_menu_params(
            "global".to_string(),
            "copy".to_string(),
            &unbound_runtime,
            &unbound_keymap,
        );

        let single_keymap =
            keymap_with_replacement(&TuiKeymap::default(), "composer", "submit", "ctrl-enter")
                .expect("replace binding");
        let single_runtime = RuntimeKeymap::from_config(&single_keymap).expect("runtime keymap");
        let single_params = build_keymap_action_menu_params(
            "composer".to_string(),
            "submit".to_string(),
            &single_runtime,
            &single_keymap,
        );

        let multi_keymap = keymap_with_bindings(
            &TuiKeymap::default(),
            "composer",
            "submit",
            &["ctrl-enter".to_string(), "alt-enter".to_string()],
        )
        .expect("multi binding");
        let multi_runtime = RuntimeKeymap::from_config(&multi_keymap).expect("runtime keymap");
        let multi_params = build_keymap_action_menu_params(
            "composer".to_string(),
            "submit".to_string(),
            &multi_runtime,
            &multi_keymap,
        );
        let replace_params = build_keymap_replace_binding_menu_params(
            "composer".to_string(),
            "submit".to_string(),
            &multi_runtime,
        );
        let snapshot = [
            "unbound:",
            &action_menu_rows(&unbound_params),
            "",
            "single:",
            &action_menu_rows(&single_params),
            "",
            "multi:",
            &action_menu_rows(&multi_params),
            "",
            "replace picker:",
            &action_menu_rows(&replace_params),
        ]
        .join("\n");

        assert_snapshot!("keymap_action_menu", snapshot);
    }

    #[test]
    fn action_menu_disables_clear_when_action_has_no_custom_binding() {
        let runtime = RuntimeKeymap::defaults();
        let params = build_keymap_action_menu_params(
            "composer".to_string(),
            "submit".to_string(),
            &runtime,
            &TuiKeymap::default(),
        );

        assert_eq!(params.view_id, Some(KEYMAP_ACTION_MENU_VIEW_ID));
        let replace = selection_item(&params, "Replace binding");
        let add_alternate = selection_item(&params, "Add alternate binding");
        let remove = selection_item(&params, "Remove custom binding");
        let back = selection_item(&params, "Back to shortcuts");
        assert_eq!(
            remove.disabled_reason.as_deref(),
            Some("There is no custom root binding for this action to remove.")
        );
        assert!(
            !replace.dismiss_on_select,
            "replace should keep the action menu under key capture"
        );
        assert!(
            !add_alternate.dismiss_on_select,
            "add alternate should keep the action menu under key capture"
        );
        assert!(!remove.dismiss_on_select, "clear-key waits for save result");
        assert!(
            back.dismiss_on_select,
            "back should dismiss the action menu"
        );
    }

    #[test]
    fn capture_view_snapshot() {
        let view = KeymapCaptureView::new(
            "composer".to_string(),
            "submit".to_string(),
            KeymapEditIntent::ReplaceAll,
            "Submit".to_string(),
            "enter".to_string(),
            app_event_sender(),
        );

        assert_snapshot!(
            "keymap_capture_view",
            format!("{:?}", render_capture(&view, /*width*/ 80, /*height*/ 8))
        );
    }

    #[test]
    fn capture_completion_returns_to_selected_keymap_picker_row() {
        let (mut pane, tx, mut rx) = test_pane();
        let runtime = RuntimeKeymap::defaults();
        pane.show_selection_view(build_keymap_picker_params(&runtime, &TuiKeymap::default()));
        pane.show_selection_view(build_keymap_action_menu_params(
            "composer".to_string(),
            "submit".to_string(),
            &runtime,
            &TuiKeymap::default(),
        ));

        pane.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        let AppEvent::OpenKeymapCapture {
            context,
            action,
            intent,
        } = rx.try_recv().expect("open capture event")
        else {
            panic!("expected OpenKeymapCapture event");
        };
        assert_eq!(intent, KeymapEditIntent::ReplaceAll);
        assert_eq!(pane.active_view_id(), Some(KEYMAP_ACTION_MENU_VIEW_ID));

        pane.show_view(Box::new(build_keymap_capture_view(
            context, action, intent, &runtime, tx,
        )));
        pane.handle_key_event(KeyEvent::new(KeyCode::Char('K'), KeyModifiers::CONTROL));

        let AppEvent::KeymapCaptured {
            context,
            action,
            key,
            intent,
        } = rx.try_recv().expect("captured key event")
        else {
            panic!("expected KeymapCaptured event");
        };
        assert_eq!(context, "composer");
        assert_eq!(action, "submit");
        assert_eq!(key, "ctrl-shift-k");
        assert_eq!(intent, KeymapEditIntent::ReplaceAll);
        assert_eq!(pane.active_view_id(), Some(KEYMAP_ACTION_MENU_VIEW_ID));

        let keymap =
            keymap_with_replacement(&TuiKeymap::default(), &context, &action, &key).unwrap();
        let runtime = RuntimeKeymap::from_config(&keymap).unwrap();
        let params =
            build_keymap_picker_params_for_selected_action(&runtime, &keymap, &context, &action);
        let selected_idx = params.initial_selected_idx;
        assert!(
            pane.replace_active_views_with_selection_view(
                &[
                    KEYMAP_PICKER_VIEW_ID,
                    KEYMAP_ACTION_MENU_VIEW_ID,
                    KEYMAP_REPLACE_BINDING_MENU_VIEW_ID,
                ],
                params
            ),
            "successful assignment should return to the main picker"
        );
        assert_eq!(pane.active_view_id(), Some(KEYMAP_PICKER_VIEW_ID));
        assert_eq!(
            pane.selected_index_for_active_view(KEYMAP_PICKER_VIEW_ID),
            selected_idx
        );

        pane.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert!(
            pane.no_modal_or_popup_active(),
            "the original picker should not remain behind the refreshed picker"
        );
    }

    #[test]
    fn clear_completion_returns_to_selected_keymap_picker_row() {
        let (mut pane, _tx, mut rx) = test_pane();
        let keymap =
            keymap_with_replacement(&TuiKeymap::default(), "composer", "submit", "ctrl-enter")
                .unwrap();
        let runtime = RuntimeKeymap::from_config(&keymap).unwrap();
        pane.show_selection_view(build_keymap_picker_params(&runtime, &keymap));
        pane.show_selection_view(build_keymap_action_menu_params(
            "composer".to_string(),
            "submit".to_string(),
            &runtime,
            &keymap,
        ));

        pane.handle_key_event(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        pane.handle_key_event(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        pane.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        let AppEvent::KeymapCleared { context, action } =
            rx.try_recv().expect("clear keymap event")
        else {
            panic!("expected KeymapCleared event");
        };
        assert_eq!(context, "composer");
        assert_eq!(action, "submit");
        assert_eq!(pane.active_view_id(), Some(KEYMAP_ACTION_MENU_VIEW_ID));

        let runtime = RuntimeKeymap::defaults();
        let params = build_keymap_picker_params_for_selected_action(
            &runtime,
            &TuiKeymap::default(),
            &context,
            &action,
        );
        let selected_idx = params.initial_selected_idx;
        assert!(
            pane.replace_active_views_with_selection_view(
                &[
                    KEYMAP_PICKER_VIEW_ID,
                    KEYMAP_ACTION_MENU_VIEW_ID,
                    KEYMAP_REPLACE_BINDING_MENU_VIEW_ID,
                ],
                params
            ),
            "successful clear should return to the main picker"
        );
        assert_eq!(pane.active_view_id(), Some(KEYMAP_PICKER_VIEW_ID));
        assert_eq!(
            pane.selected_index_for_active_view(KEYMAP_PICKER_VIEW_ID),
            selected_idx
        );

        pane.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert!(
            pane.no_modal_or_popup_active(),
            "the original picker should not remain behind the refreshed picker"
        );
    }

    #[test]
    fn replace_one_completion_drops_focused_keymap_submenus() {
        let (mut pane, _tx, _rx) = test_pane();
        let runtime = RuntimeKeymap::defaults();
        pane.show_selection_view(build_keymap_picker_params(&runtime, &TuiKeymap::default()));
        pane.show_selection_view(build_keymap_action_menu_params(
            "composer".to_string(),
            "toggle_shortcuts".to_string(),
            &runtime,
            &TuiKeymap::default(),
        ));
        pane.show_selection_view(build_keymap_replace_binding_menu_params(
            "composer".to_string(),
            "toggle_shortcuts".to_string(),
            &runtime,
        ));
        assert_eq!(
            pane.active_view_id(),
            Some(KEYMAP_REPLACE_BINDING_MENU_VIEW_ID)
        );

        let params = build_keymap_picker_params_for_selected_action(
            &runtime,
            &TuiKeymap::default(),
            "composer",
            "toggle_shortcuts",
        );
        assert!(
            pane.replace_active_views_with_selection_view(
                &[
                    KEYMAP_PICKER_VIEW_ID,
                    KEYMAP_ACTION_MENU_VIEW_ID,
                    KEYMAP_REPLACE_BINDING_MENU_VIEW_ID,
                ],
                params
            ),
            "successful replace-one should return to the main picker"
        );
        assert_eq!(pane.active_view_id(), Some(KEYMAP_PICKER_VIEW_ID));

        pane.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert!(
            pane.no_modal_or_popup_active(),
            "the parent action menu should not remain behind the picker"
        );
    }

    #[test]
    fn key_capture_serializes_modifier_order_for_config() {
        let event = KeyEvent::new(
            KeyCode::Char('K'),
            KeyModifiers::CONTROL | KeyModifiers::ALT,
        );

        assert_eq!(
            key_event_to_config_key_spec(event),
            Ok("ctrl-alt-shift-k".to_string())
        );
    }

    #[test]
    fn key_capture_serializes_special_keys() {
        assert_eq!(
            key_event_to_config_key_spec(KeyEvent::new(KeyCode::PageDown, KeyModifiers::SHIFT)),
            Ok("shift-page-down".to_string())
        );
    }

    #[test]
    fn key_capture_rejects_unrepresentable_keys() {
        assert!(
            key_event_to_config_key_spec(KeyEvent::new(KeyCode::Char('-'), KeyModifiers::NONE))
                .is_err()
        );
    }

    #[test]
    fn replacement_sets_single_binding() {
        let keymap =
            keymap_with_replacement(&TuiKeymap::default(), "composer", "submit", "ctrl-enter")
                .expect("replace binding");

        assert_eq!(
            keymap.composer.submit,
            Some(KeybindingsSpec::One(KeybindingSpec(
                "ctrl-enter".to_string()
            )))
        );
    }

    #[test]
    fn replace_all_collapses_multi_binding_to_single() {
        let keymap = keymap_with_bindings(
            &TuiKeymap::default(),
            "composer",
            "submit",
            &["ctrl-enter".to_string(), "alt-enter".to_string()],
        )
        .expect("multi binding");
        let runtime = RuntimeKeymap::from_config(&keymap).expect("runtime keymap");
        let outcome = keymap_with_edit(
            &keymap,
            &runtime,
            "composer",
            "submit",
            "ctrl-shift-enter",
            &KeymapEditIntent::ReplaceAll,
        )
        .expect("replace all");

        let KeymapEditOutcome::Updated {
            keymap_config,
            bindings,
            ..
        } = outcome
        else {
            panic!("expected updated keymap");
        };
        assert_eq!(bindings, vec!["ctrl-shift-enter"]);
        assert_eq!(
            keymap_config.composer.submit,
            Some(KeybindingsSpec::One(KeybindingSpec(
                "ctrl-shift-enter".to_string()
            )))
        );
    }

    #[test]
    fn add_alternate_grows_single_binding() {
        let runtime = RuntimeKeymap::defaults();
        let outcome = keymap_with_edit(
            &TuiKeymap::default(),
            &runtime,
            "composer",
            "submit",
            "ctrl-enter",
            &KeymapEditIntent::AddAlternate,
        )
        .expect("add alternate");

        let KeymapEditOutcome::Updated {
            keymap_config,
            bindings,
            ..
        } = outcome
        else {
            panic!("expected updated keymap");
        };
        assert_eq!(bindings, vec!["enter", "ctrl-enter"]);
        assert_eq!(
            keymap_config.composer.submit,
            Some(KeybindingsSpec::Many(vec![
                KeybindingSpec("enter".to_string()),
                KeybindingSpec("ctrl-enter".to_string())
            ]))
        );
    }

    #[test]
    fn add_alternate_grows_default_multi_binding() {
        let runtime = RuntimeKeymap::defaults();
        let outcome = keymap_with_edit(
            &TuiKeymap::default(),
            &runtime,
            "editor",
            "move_left",
            "ctrl-shift-b",
            &KeymapEditIntent::AddAlternate,
        )
        .expect("add alternate");

        let KeymapEditOutcome::Updated {
            keymap_config,
            bindings,
            ..
        } = outcome
        else {
            panic!("expected updated keymap");
        };
        assert_eq!(bindings, vec!["left", "ctrl-b", "ctrl-shift-b"]);
        assert_eq!(
            keymap_config.editor.move_left,
            Some(KeybindingsSpec::Many(vec![
                KeybindingSpec("left".to_string()),
                KeybindingSpec("ctrl-b".to_string()),
                KeybindingSpec("ctrl-shift-b".to_string())
            ]))
        );
    }

    #[test]
    fn add_alternate_duplicate_is_noop() {
        let runtime = RuntimeKeymap::defaults();
        let outcome = keymap_with_edit(
            &TuiKeymap::default(),
            &runtime,
            "composer",
            "submit",
            "enter",
            &KeymapEditIntent::AddAlternate,
        )
        .expect("duplicate alternate");

        assert_eq!(
            outcome,
            KeymapEditOutcome::Unchanged {
                message: "No change: `composer.submit` already uses `enter`.".to_string()
            }
        );
    }

    #[test]
    fn replace_one_preserves_other_bindings() {
        let keymap = keymap_with_bindings(
            &TuiKeymap::default(),
            "composer",
            "submit",
            &["ctrl-enter".to_string(), "alt-enter".to_string()],
        )
        .expect("multi binding");
        let runtime = RuntimeKeymap::from_config(&keymap).expect("runtime keymap");
        let outcome = keymap_with_edit(
            &keymap,
            &runtime,
            "composer",
            "submit",
            "ctrl-shift-enter",
            &KeymapEditIntent::ReplaceOne {
                old_key: "ctrl-enter".to_string(),
            },
        )
        .expect("replace one");

        let KeymapEditOutcome::Updated {
            keymap_config,
            bindings,
            ..
        } = outcome
        else {
            panic!("expected updated keymap");
        };
        assert_eq!(bindings, vec!["ctrl-shift-enter", "alt-enter"]);
        assert_eq!(
            keymap_config.composer.submit,
            Some(KeybindingsSpec::Many(vec![
                KeybindingSpec("ctrl-shift-enter".to_string()),
                KeybindingSpec("alt-enter".to_string())
            ]))
        );
    }

    #[test]
    fn replace_one_deduplicates_replacement() {
        let keymap = keymap_with_bindings(
            &TuiKeymap::default(),
            "composer",
            "submit",
            &["ctrl-enter".to_string(), "alt-enter".to_string()],
        )
        .expect("multi binding");
        let runtime = RuntimeKeymap::from_config(&keymap).expect("runtime keymap");
        let outcome = keymap_with_edit(
            &keymap,
            &runtime,
            "composer",
            "submit",
            "alt-enter",
            &KeymapEditIntent::ReplaceOne {
                old_key: "ctrl-enter".to_string(),
            },
        )
        .expect("replace one");

        let KeymapEditOutcome::Updated {
            keymap_config,
            bindings,
            ..
        } = outcome
        else {
            panic!("expected updated keymap");
        };
        assert_eq!(bindings, vec!["alt-enter"]);
        assert_eq!(
            keymap_config.composer.submit,
            Some(KeybindingsSpec::One(KeybindingSpec(
                "alt-enter".to_string()
            )))
        );
    }

    #[test]
    fn replace_one_rejects_stale_old_key() {
        let runtime = RuntimeKeymap::defaults();
        let err = keymap_with_edit(
            &TuiKeymap::default(),
            &runtime,
            "composer",
            "submit",
            "ctrl-enter",
            &KeymapEditIntent::ReplaceOne {
                old_key: "alt-enter".to_string(),
            },
        )
        .expect_err("stale old key");

        assert!(err.contains("composer.submit"));
        assert!(err.contains("alt-enter"));
    }

    #[test]
    fn clear_removes_custom_binding() {
        let keymap =
            keymap_with_replacement(&TuiKeymap::default(), "composer", "submit", "ctrl-enter")
                .expect("replace binding");

        assert_eq!(has_custom_binding(&keymap, "composer", "submit"), Ok(true));

        let cleared =
            keymap_without_custom_binding(&keymap, "composer", "submit").expect("clear binding");

        assert_eq!(cleared.composer.submit, None);
        assert_eq!(
            has_custom_binding(&cleared, "composer", "submit"),
            Ok(false)
        );
    }

    #[test]
    fn replacement_rejects_unknown_action() {
        let err = keymap_with_replacement(&TuiKeymap::default(), "composer", "nope", "ctrl-enter")
            .expect_err("unknown action");

        assert!(err.contains("composer.nope"));
    }
}
