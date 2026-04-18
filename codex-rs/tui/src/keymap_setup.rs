//! Guided keymap remapping UI for `/keymap`.
//!
//! Pick an action, choose whether to set or remove its root-level custom
//! binding, then validate and persist the resulting runtime keymap.

mod actions;
mod details;

use std::sync::Arc;
use std::sync::Mutex;

use codex_config::types::KeybindingSpec;
use codex_config::types::KeybindingsSpec;
use codex_config::types::TuiKeymap;
use crossterm::cursor::SetCursorStyle;
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
use crate::app_event_sender::AppEventSender;
use crate::bottom_pane::BottomPaneView;
use crate::bottom_pane::CancellationEvent;
use crate::bottom_pane::ColumnWidthMode;
use crate::bottom_pane::SelectionItem;
use crate::bottom_pane::SelectionViewParams;
use crate::bottom_pane::SideContentWidth;
use crate::bottom_pane::popup_consts::standard_popup_hint_line;
use crate::keymap::RuntimeKeymap;
use crate::render::renderable::Renderable;
use actions::KEYMAP_ACTIONS;
use actions::action_label;
use actions::binding_slot;
use actions::bindings_for_action;
use actions::format_binding_summary;
use details::KeymapActionDetailsLayout;
use details::KeymapActionDetailsRenderable;
use details::build_action_details;

const KEYMAP_PICKER_VIEW_ID: &str = "keymap-picker";
pub(crate) const KEYMAP_ACTION_MENU_VIEW_ID: &str = "keymap-action-menu";
const KEYMAP_DETAIL_PANEL_WIDTH: u16 = 46;
const KEYMAP_DETAIL_PANEL_MIN_WIDTH: u16 = 40;

pub(crate) fn build_keymap_picker_params(
    runtime_keymap: &RuntimeKeymap,
    keymap_config: &TuiKeymap,
) -> SelectionViewParams {
    let details = Arc::new(build_action_details(runtime_keymap, keymap_config));
    let selected_detail_idx = Arc::new(Mutex::new(0usize));
    let selected_detail_idx_for_callback = selected_detail_idx.clone();

    let items = KEYMAP_ACTIONS
        .iter()
        .copied()
        .map(|descriptor| {
            let bindings =
                bindings_for_action(runtime_keymap, descriptor.context, descriptor.action)
                    .unwrap_or(&[]);
            let binding_summary = format_binding_summary(bindings);
            let context = descriptor.context.to_string();
            let action = descriptor.action.to_string();
            let label = action_label(descriptor.action);
            let search_value = format!(
                "{} {} {} {} {}",
                descriptor.context_label,
                descriptor.action,
                label,
                descriptor.description,
                binding_summary
            );

            SelectionItem {
                name: label,
                name_prefix_spans: vec![format!("{:<12} ", descriptor.context_label).dim()],
                description: Some(binding_summary),
                actions: vec![Box::new(move |tx| {
                    tx.send(AppEvent::OpenKeymapActionMenu {
                        context: context.clone(),
                        action: action.clone(),
                    });
                })],
                search_value: Some(search_value),
                ..Default::default()
            }
        })
        .collect();

    let on_selection_changed = Some(Box::new(move |idx: usize, _tx: &_| {
        if let Ok(mut selected_idx) = selected_detail_idx_for_callback.lock() {
            *selected_idx = idx;
        }
    })
        as Box<dyn Fn(usize, &crate::app_event_sender::AppEventSender) + Send + Sync>);

    SelectionViewParams {
        view_id: Some(KEYMAP_PICKER_VIEW_ID),
        title: Some("Remap Shortcut".to_string()),
        subtitle: Some("Search actions. Enter edits the selected shortcut.".to_string()),
        footer_note: Some(Line::from(vec![
            "Saves to root ".dim(),
            "`tui.keymap.*`".cyan(),
            " so shortcuts stay consistent across profiles.".dim(),
        ])),
        footer_hint: Some(standard_popup_hint_line()),
        items,
        is_searchable: true,
        search_placeholder: Some("Search actions...".to_string()),
        col_width_mode: ColumnWidthMode::AutoAllRows,
        side_content: Box::new(KeymapActionDetailsRenderable::new(
            details.clone(),
            selected_detail_idx.clone(),
            KeymapActionDetailsLayout::Wide,
        )),
        side_content_width: SideContentWidth::Fixed(KEYMAP_DETAIL_PANEL_WIDTH),
        side_content_min_width: KEYMAP_DETAIL_PANEL_MIN_WIDTH,
        stacked_side_content: Some(Box::new(KeymapActionDetailsRenderable::new(
            details,
            selected_detail_idx,
            KeymapActionDetailsLayout::NarrowFooter,
        ))),
        on_selection_changed,
        ..Default::default()
    }
}

pub(crate) fn build_keymap_action_menu_params(
    context: String,
    action: String,
    runtime_keymap: &RuntimeKeymap,
    keymap_config: &TuiKeymap,
) -> SelectionViewParams {
    let current_binding = format_binding_summary(
        bindings_for_action(runtime_keymap, &context, &action).unwrap_or(&[]),
    );
    let custom_binding = has_custom_binding(keymap_config, &context, &action).unwrap_or(false);
    let remove_disabled_reason = (!custom_binding)
        .then(|| "There is no custom root binding for this action to remove.".to_string());
    let label = action_label(&action);
    let set_context = context.clone();
    let set_action = action.clone();
    let remove_context = context.clone();
    let remove_action = action.clone();

    SelectionViewParams {
        view_id: Some(KEYMAP_ACTION_MENU_VIEW_ID),
        title: Some("Edit Shortcut".to_string()),
        subtitle: Some(format!("{label}  {context}.{action}")),
        footer_note: Some(Line::from(vec![
            "Remove clears the root ".dim(),
            "`tui.keymap.*`".cyan(),
            " entry and falls back to the default keymap.".dim(),
        ])),
        footer_hint: Some(standard_popup_hint_line()),
        items: vec![
            SelectionItem {
                name: "Set new key".to_string(),
                description: Some(format!("Current: {current_binding}")),
                selected_description: Some(format!(
                    "Current: {current_binding}. Capture one key and replace this action's custom binding."
                )),
                actions: vec![Box::new(move |tx| {
                    tx.send(AppEvent::OpenKeymapCapture {
                        context: set_context.clone(),
                        action: set_action.clone(),
                    });
                })],
                ..Default::default()
            },
            SelectionItem {
                name: "Remove custom binding".to_string(),
                description: Some("Restore the default binding for this action.".to_string()),
                selected_description: Some(
                    "Delete the root custom binding and use the default keymap again.".to_string(),
                ),
                disabled_reason: remove_disabled_reason,
                actions: vec![Box::new(move |tx| {
                    tx.send(AppEvent::KeymapCleared {
                        context: remove_context.clone(),
                        action: remove_action.clone(),
                    });
                })],
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

pub(crate) fn build_keymap_conflict_params(
    context: String,
    action: String,
    key: String,
    error: String,
) -> SelectionViewParams {
    let retry_context = context.clone();
    let retry_action = action.clone();
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
    runtime_keymap: &RuntimeKeymap,
    app_event_tx: AppEventSender,
) -> KeymapCaptureView {
    let current_binding = format_binding_summary(
        bindings_for_action(runtime_keymap, &context, &action).unwrap_or(&[]),
    );
    let label = action_label(&action);
    KeymapCaptureView::new(context, action, label, current_binding, app_event_tx)
}

pub(crate) fn keymap_with_replacement(
    keymap: &TuiKeymap,
    context: &str,
    action: &str,
    key: &str,
) -> Result<TuiKeymap, String> {
    let mut keymap = keymap.clone();
    let slot = binding_slot(&mut keymap, context, action).ok_or_else(|| {
        format!("Unknown keymap action `{context}.{action}`. Reopen /keymap and choose an action.")
    })?;
    *slot = Some(KeybindingsSpec::One(KeybindingSpec(key.to_string())));
    Ok(keymap)
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
        label: String,
        current_binding: String,
        app_event_tx: AppEventSender,
    ) -> Self {
        Self {
            context,
            action,
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

    fn cursor_style(&self, _area: Rect) -> SetCursorStyle {
        SetCursorStyle::BlinkingBar
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
    use super::*;
    use crate::bottom_pane::BottomPane;
    use crate::bottom_pane::BottomPaneParams;
    use crate::bottom_pane::ListSelectionView;
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
                line
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

    #[test]
    fn picker_covers_every_replaceable_action() {
        let runtime = RuntimeKeymap::defaults();
        let params = build_keymap_picker_params(&runtime, &TuiKeymap::default());

        assert_eq!(params.items.len(), KEYMAP_ACTIONS.len());
        assert!(
            params.items.iter().all(|item| !item.dismiss_on_select),
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
    fn picker_content_snapshot() {
        let runtime = RuntimeKeymap::defaults();
        let params = build_keymap_picker_params(&runtime, &TuiKeymap::default());
        let snapshot = params
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

        assert_snapshot!("keymap_picker_first_actions", snapshot);
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
    fn picker_detail_tracks_selection() {
        let runtime = RuntimeKeymap::defaults();
        let params = build_keymap_picker_params(&runtime, &TuiKeymap::default());
        let mut view =
            ListSelectionView::new(params, app_event_sender(), RuntimeKeymap::defaults().list);

        view.handle_key_event(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));

        let rendered = render_picker_from_view(&view, /*width*/ 120);
        assert!(rendered.contains("Open the current draft in an external editor."));
    }

    #[test]
    fn picker_narrow_uses_compact_detail() {
        let runtime = RuntimeKeymap::defaults();
        let params = build_keymap_picker_params(&runtime, &TuiKeymap::default());
        let rendered = render_picker(params, /*width*/ 78);

        assert!(rendered.contains("Open the transcript overlay."));
        assert!(!rendered.contains("Current: ctrl-t"));
        assert!(!rendered.contains("Source: default keymap"));
    }

    #[test]
    fn action_menu_content_snapshot() {
        let keymap =
            keymap_with_replacement(&TuiKeymap::default(), "composer", "submit", "ctrl-enter")
                .expect("replace binding");
        let runtime = RuntimeKeymap::from_config(&keymap).expect("runtime keymap");
        let params = build_keymap_action_menu_params(
            "composer".to_string(),
            "submit".to_string(),
            &runtime,
            &keymap,
        );
        let snapshot = params
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
        assert_eq!(
            params.items[1].disabled_reason.as_deref(),
            Some("There is no custom root binding for this action to remove.")
        );
        assert!(
            !params.items[0].dismiss_on_select,
            "set-key should keep the action menu under key capture"
        );
        assert!(
            !params.items[1].dismiss_on_select,
            "clear-key should keep the action menu open for refresh"
        );
        assert!(
            params.items[2].dismiss_on_select,
            "cancel should dismiss the action menu"
        );
    }

    #[test]
    fn capture_view_snapshot() {
        let view = KeymapCaptureView::new(
            "composer".to_string(),
            "submit".to_string(),
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
    fn capture_completion_returns_to_keymap_action_menu() {
        let (mut pane, tx, mut rx) = test_pane();
        let runtime = RuntimeKeymap::defaults();
        pane.show_selection_view(build_keymap_action_menu_params(
            "composer".to_string(),
            "submit".to_string(),
            &runtime,
            &TuiKeymap::default(),
        ));

        pane.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        let AppEvent::OpenKeymapCapture { context, action } =
            rx.try_recv().expect("open capture event")
        else {
            panic!("expected OpenKeymapCapture event");
        };
        assert_eq!(pane.active_view_id(), Some(KEYMAP_ACTION_MENU_VIEW_ID));

        pane.show_view(Box::new(build_keymap_capture_view(
            context, action, &runtime, tx,
        )));
        pane.handle_key_event(KeyEvent::new(KeyCode::Char('K'), KeyModifiers::CONTROL));

        let AppEvent::KeymapCaptured {
            context,
            action,
            key,
        } = rx.try_recv().expect("captured key event")
        else {
            panic!("expected KeymapCaptured event");
        };
        assert_eq!(context, "composer");
        assert_eq!(action, "submit");
        assert_eq!(key, "ctrl-shift-k");
        assert_eq!(pane.active_view_id(), Some(KEYMAP_ACTION_MENU_VIEW_ID));

        let keymap =
            keymap_with_replacement(&TuiKeymap::default(), &context, &action, &key).unwrap();
        let runtime = RuntimeKeymap::from_config(&keymap).unwrap();
        let params = build_keymap_action_menu_params(context, action, &runtime, &keymap);
        assert!(
            pane.replace_selection_view_if_active(KEYMAP_ACTION_MENU_VIEW_ID, params),
            "active action menu should refresh after a successful assignment"
        );
        let area = Rect::new(0, 0, 80, pane.desired_height(/*width*/ 80));
        let mut buf = Buffer::empty(area);
        pane.render(area, &mut buf);
        let rendered = render_buffer(&buf);
        assert!(
            rendered.contains("Current: ctrl-shift-k"),
            "rendered action menu did not include updated binding:\n{rendered}"
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
