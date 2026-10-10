//! Render the configuration catalog and its child choice picker.

use super::ConfigPanel;
use super::ConfigPreference;
use super::ConfigSection;
use super::ConfigSetting;
use super::ConfigSettingValue;
use super::PickerState;
use super::ResumeCurrentAvailability;
use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::bottom_pane::ColumnWidthMode;
use crate::bottom_pane::ListSelectionView;
use crate::bottom_pane::PickerSurface;
use crate::bottom_pane::SearchMode;
use crate::bottom_pane::SelectionItem;
use crate::bottom_pane::SelectionRowDisplay;
use crate::bottom_pane::SelectionTab;
use crate::bottom_pane::SelectionViewParams;
use crate::bottom_pane::footer_hint_items_line;
use crate::bottom_pane::popup_consts::picker_hint_line_for_keymap;
use crate::key_hint::KeyBinding;
use crate::keymap::ListAction;
use crate::keymap::ListKeymap;
use crate::render::renderable::Renderable;
use crate::wrapping::word_wrap_lines;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use unicode_width::UnicodeWidthStr;

pub(crate) fn notification_condition(
    tui: &codex_config::types::Tui,
    keymap: &ListKeymap,
) -> SelectionViewParams {
    use codex_config::types::NotificationCondition;

    let current = tui.notification_settings.condition;
    let conditions = [
        ("Unfocused", NotificationCondition::Unfocused),
        ("Always", NotificationCondition::Always),
    ];
    SelectionViewParams {
        title: Some("Notify when".to_string()),
        subtitle: Some(
            "Only notify while you are away, or allow notifications while this terminal is focused too."
                .to_string(),
        ),
        footer_hint: Some(picker_hint_line_for_keymap(keymap)),
        initial_selected_idx: conditions
            .iter()
            .position(|(_, condition)| *condition == current),
        allow_input_when_disconnected: true,
        items: conditions
            .into_iter()
            .map(|(label, condition)| SelectionItem {
                name: label.to_string(),
                is_current: condition == current,
                actions: vec![Box::new(move |tx| {
                    tx.send(AppEvent::SaveConfigPreference(
                        ConfigPreference::NotificationCondition(condition),
                    ));
                })],
                dismiss_on_select: true,
                ..Default::default()
            })
            .collect(),
        ..SelectionViewParams::picker()
    }
}

pub(crate) fn copy_on_select(
    tui: &codex_config::types::Tui,
    keymap: &ListKeymap,
) -> SelectionViewParams {
    use codex_config::types::CopyOnSelect;

    let current = tui.copy_on_select;
    let modes = [
        ("Auto (default)", CopyOnSelect::Auto),
        ("On", CopyOnSelect::Always),
        ("Off", CopyOnSelect::Never),
    ];
    SelectionViewParams {
        title: Some("Copy on select".to_string()),
        subtitle: Some(
            "Copy transcript selections on release. Auto uses terminal defaults.".to_string(),
        ),
        footer_hint: Some(picker_hint_line_for_keymap(keymap)),
        initial_selected_idx: modes.iter().position(|(_, mode)| *mode == current),
        allow_input_when_disconnected: true,
        items: modes
            .into_iter()
            .map(|(label, mode)| SelectionItem {
                name: label.to_string(),
                is_current: mode == current,
                actions: vec![Box::new(move |tx| {
                    tx.send(AppEvent::SaveConfigPreference(
                        ConfigPreference::CopyOnSelect(mode),
                    ));
                })],
                dismiss_on_select: true,
                ..Default::default()
            })
            .collect(),
        ..SelectionViewParams::picker()
    }
}

pub(crate) fn right_click_paste(
    tui: &codex_config::types::Tui,
    keymap: &ListKeymap,
) -> SelectionViewParams {
    use codex_config::types::RightClickPaste;

    let current = tui.right_click_paste;
    let modes = [
        ("Auto (default)", RightClickPaste::Auto),
        ("On", RightClickPaste::On),
        ("Off", RightClickPaste::Off),
    ];
    SelectionViewParams {
        title: Some("Right-click paste".to_string()),
        subtitle: Some(super::RIGHT_CLICK_PASTE_DESCRIPTION.to_string()),
        footer_hint: Some(picker_hint_line_for_keymap(keymap)),
        initial_selected_idx: modes.iter().position(|(_, mode)| *mode == current),
        allow_input_when_disconnected: true,
        items: modes
            .into_iter()
            .map(|(label, mode)| SelectionItem {
                name: label.to_string(),
                is_current: mode == current,
                actions: vec![Box::new(move |tx| {
                    tx.send(AppEvent::SaveConfigPreference(
                        ConfigPreference::RightClickPaste(mode),
                    ));
                })],
                dismiss_on_select: true,
                ..Default::default()
            })
            .collect(),
        ..SelectionViewParams::picker()
    }
}

pub(crate) fn session_picker_layout(
    tui: &codex_config::types::Tui,
    keymap: &ListKeymap,
) -> SelectionViewParams {
    use codex_config::types::SessionPickerViewMode;

    let current = tui.session_picker_view.unwrap_or_default();
    let modes = [
        ("Dense", SessionPickerViewMode::Dense),
        ("Comfortable", SessionPickerViewMode::Comfortable),
    ];
    SelectionViewParams {
        title: Some("Session picker layout".to_string()),
        subtitle: Some(
            "Choose tighter rows or a roomier list when resuming and forking conversations."
                .to_string(),
        ),
        footer_hint: Some(picker_hint_line_for_keymap(keymap)),
        initial_selected_idx: modes.iter().position(|(_, mode)| *mode == current),
        allow_input_when_disconnected: true,
        items: modes
            .into_iter()
            .map(|(label, mode)| SelectionItem {
                name: label.to_string(),
                is_current: mode == current,
                actions: vec![Box::new(move |tx| {
                    tx.send(AppEvent::SaveConfigPreference(
                        ConfigPreference::SessionPickerLayout(mode),
                    ));
                })],
                dismiss_on_select: true,
                ..Default::default()
            })
            .collect(),
        ..SelectionViewParams::picker()
    }
}

pub(crate) fn resume_directory(
    tui: &codex_config::types::Tui,
    keymap: &ListKeymap,
    current_availability: ResumeCurrentAvailability,
) -> SelectionViewParams {
    use codex_config::types::ResumeCwdMode;

    let current = tui.resume_cwd;
    let modes = [
        ("Ask", None),
        ("Current directory", Some(ResumeCwdMode::Current)),
        ("Session directory", Some(ResumeCwdMode::Session)),
    ];
    SelectionViewParams {
        title: Some("Resume directory".to_string()),
        subtitle: Some(
            "Choose which working directory to use when the current directory and saved session differ. A directory passed with --cd takes precedence for that Codex session."
                .to_string(),
        ),
        footer_hint: Some(picker_hint_line_for_keymap(keymap)),
        initial_selected_idx: modes.iter().position(|(_, mode)| *mode == current),
        allow_input_when_disconnected: true,
        items: modes
            .into_iter()
            .map(|(label, mode)| SelectionItem {
                name: label.to_string(),
                is_current: mode == current,
                disabled_reason: if mode == Some(ResumeCwdMode::Current)
                    && matches!(
                        current_availability,
                        ResumeCurrentAvailability::RequiresCwdOverride
                    )
                {
                    Some("Requires --cd for remote sessions.".to_string())
                } else {
                    None
                },
                actions: vec![Box::new(move |tx| {
                    tx.send(AppEvent::SaveConfigPreference(
                        ConfigPreference::ResumeDirectory(mode),
                    ));
                })],
                dismiss_on_select: true,
                ..Default::default()
            })
            .collect(),
        ..SelectionViewParams::picker()
    }
}

struct ConfigPanelHeader(Vec<Line<'static>>);

impl Renderable for ConfigPanelHeader {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        Renderable::render(
            &Paragraph::new(word_wrap_lines(&self.0, usize::from(area.width))),
            area,
            buf,
        );
    }

    fn desired_height(&self, width: u16) -> u16 {
        word_wrap_lines(&self.0, usize::from(width)).len() as u16
    }
}

fn header(summary: String) -> Box<dyn Renderable> {
    Box::new(ConfigPanelHeader(vec![
        Line::from("Configuration".bold()),
        Line::from("Changes save automatically; some apply after restarting Codex.".dim()),
        Line::from(summary.dim()),
    ]))
}

fn hint(keymap: &ListKeymap, search: Option<KeyBinding>) -> Line<'static> {
    let key_label = |action| {
        keymap.primary_hint(action).map(|hint| {
            hint.display_label()
                .replace('←', "left")
                .replace('→', "right")
        })
    };
    let mut items = Vec::new();
    let keys = [ListAction::MoveLeft, ListAction::MoveRight]
        .into_iter()
        .filter_map(key_label)
        .collect::<Vec<_>>()
        .join("/");
    if !keys.is_empty() {
        items.push((keys, "group".into()));
    }
    if let Some(search) = search {
        items.push((search.display_label(), "search".into()));
    }
    items.extend(
        [
            (ListAction::Accept, "edit setting"),
            (ListAction::Cancel, "close"),
        ]
        .into_iter()
        .filter_map(|(action, label)| key_label(action).map(|key| (key, label.into()))),
    );
    footer_hint_items_line(&items)
}

pub(super) fn picker(
    settings: &[ConfigSetting],
    app_event_tx: AppEventSender,
    keymap: ListKeymap,
    state: PickerState,
) -> ListSelectionView {
    let summary = |count| {
        state
            .notice
            .clone()
            .unwrap_or_else(|| format!("Settings in this group: {count}."))
    };
    let search = crate::key_hint::plain(KeyCode::Char('/'));
    let (search_code, search_modifiers) = search.parts();
    let search_event = KeyEvent::new(search_code, search_modifiers);
    let search_hint = (keymap.action_for(search_event).is_none()
        && !keymap.has_chord_prefix(search_event))
    .then_some(search);
    let mut params = SelectionViewParams {
        picker_surface: PickerSurface::Panel,
        max_visible_rows: ConfigSection::ALL
            .iter()
            .map(|section| {
                settings
                    .iter()
                    .filter(|setting| setting.section == *section)
                    .count()
            })
            .max()
            .unwrap_or_default(),
        reserve_result_rows: true,
        row_display: SelectionRowDisplay::SingleLine,
        col_width_mode: ColumnWidthMode::AutoAllRows,
        footer_hint: Some(hint(&keymap, search_hint)),
        is_searchable: true,
        search_mode: SearchMode::OnDemand(search),
        search_placeholder: Some("Type to search settings".to_string()),
        name_column_width: settings
            .iter()
            .map(|setting| UnicodeWidthStr::width(setting.label))
            .max(),
        tabs: ConfigSection::ALL
            .iter()
            .map(|section| {
                let group = settings
                    .iter()
                    .filter(|setting| setting.section == *section)
                    .collect::<Vec<_>>();
                let title = section.title();
                SelectionTab {
                    id: title.to_string(),
                    label: title.to_string(),
                    header: header(summary(group.len())),
                    items: group
                        .into_iter()
                        .map(|setting| {
                            let unavailable = match &setting.value {
                                ConfigSettingValue::DependentToggle {
                                    dependency_met: false,
                                    unavailable_reason,
                                    ..
                                } => Some(*unavailable_reason),
                                ConfigSettingValue::Unavailable { reason } => Some(*reason),
                                ConfigSettingValue::Toggle { .. }
                                | ConfigSettingValue::DependentToggle { .. }
                                | ConfigSettingValue::Editor { .. }
                                | ConfigSettingValue::NotificationTest => None,
                            };
                            let value_label = setting.value.label();
                            SelectionItem {
                                name: setting.display_label(),
                                description: Some(format!(
                                    "{:<19}  {}",
                                    value_label,
                                    unavailable.unwrap_or(setting.description)
                                )),
                                is_disabled: unavailable.is_some(),
                                search_value: Some(format!(
                                    "{} {} {} {} {}",
                                    section.title(),
                                    setting.label,
                                    value_label,
                                    setting.keywords,
                                    setting.description
                                )),
                                ..Default::default()
                            }
                        })
                        .collect(),
                }
            })
            .collect(),
        ..Default::default()
    };
    params.initial_selected_idx = state.selected;
    params.initial_search_query = state.query;
    params.initial_tab_id = state.tab;
    ListSelectionView::new(params, app_event_tx, keymap)
}

impl Renderable for ConfigPanel {
    fn desired_height(&self, width: u16) -> u16 {
        self.picker.desired_height(width)
    }

    fn render(&self, area: Rect, buf: &mut Buffer) {
        self.picker.render(area, buf);
    }

    fn cursor_pos(&self, area: Rect) -> Option<(u16, u16)> {
        self.picker.cursor_pos(area)
    }
}
