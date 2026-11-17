//! Render the configuration catalog and its child choice picker.

use super::ConfigPanel;
use super::ConfigPreference;
use super::ConfigSection;
use super::ConfigSetting;
use super::ConfigSettingValue;
use super::ConfigToggle;
use super::PickerState;
use super::ResumeCurrentAvailability;
use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::bottom_pane::ColumnWidthMode;
use crate::bottom_pane::ListSelectionView;
use crate::bottom_pane::PickerSurface;
use crate::bottom_pane::SELECTION_TOGGLE_UNAVAILABLE_PREFIX;
use crate::bottom_pane::SearchMode;
use crate::bottom_pane::SelectionDescriptionLayout;
use crate::bottom_pane::SelectionItem;
use crate::bottom_pane::SelectionRowDisplay;
use crate::bottom_pane::SelectionTab;
use crate::bottom_pane::SelectionToggle;
use crate::bottom_pane::SelectionToggleState;
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

pub(crate) const EFFECTS_VIEW_ID: &str = "config-effects";

pub(crate) fn effects(tui: &codex_config::types::Tui, keymap: &ListKeymap) -> SelectionViewParams {
    effects_with_toggle_states(tui, keymap, &[])
}

pub(crate) fn effects_with_toggle_states(
    tui: &codex_config::types::Tui,
    keymap: &ListKeymap,
    toggle_states: &[Option<SelectionToggleState>],
) -> SelectionViewParams {
    let footer_hint = [(ListAction::Accept, "toggle"), (ListAction::Cancel, "back")]
        .into_iter()
        .filter_map(|(action, label)| {
            keymap
                .primary_hint(action)
                .map(|hint| (hint.display_label(), label.into()))
        })
        .collect::<Vec<_>>();
    let settings = [
        (
            "Animations",
            "Animate the welcome screen, working indicator, and transitions",
            ConfigToggle::Animations,
        ),
        (
            "Composer stars",
            "Show stars briefly for new Astra tasks",
            ConfigToggle::ComposerStars,
        ),
    ];
    let animations_enabled = toggle_states
        .first()
        .copied()
        .flatten()
        .map(|state| state.is_on)
        .unwrap_or(tui.animations);
    SelectionViewParams {
        view_id: Some(EFFECTS_VIEW_ID),
        title: Some("Effects".to_string()),
        subtitle: Some("Configure visual effects".to_string()),
        footer_hint: Some(footer_hint_items_line(&footer_hint)),
        allow_input_when_disconnected: true,
        items: settings
            .into_iter()
            .enumerate()
            .map(|(index, (label, description, setting))| {
                let toggle_state = toggle_states.get(index).copied().flatten();
                let enabled = toggle_state
                    .map(|state| state.is_on)
                    .unwrap_or_else(|| setting.enabled(tui));
                let available = setting != ConfigToggle::ComposerStars || animations_enabled;
                SelectionItem {
                    name: label.to_string(),
                    toggle: Some(SelectionToggle {
                        is_on: enabled,
                        action: Box::new(move |enabled, tx| {
                            tx.send(AppEvent::SaveConfigPreference(ConfigPreference::Toggle {
                                setting,
                                enabled,
                            }));
                        }),
                    }),
                    pending_toggle_actions: toggle_state
                        .map(|state| state.pending_actions)
                        .unwrap_or_default(),
                    checkbox_toggle: available,
                    toggle_placeholder: (!available).then_some(SELECTION_TOGGLE_UNAVAILABLE_PREFIX),
                    description: available.then(|| description.to_string()),
                    disabled_reason: (!available)
                        .then(|| "Turn on Animations to show Astra task stars".to_string()),
                    dismiss_on_select: false,
                    ..Default::default()
                }
            })
            .collect(),
        ..SelectionViewParams::picker()
    }
}

pub(crate) fn mouse_scroll_speed(
    tui: &codex_config::types::Tui,
    keymap: &ListKeymap,
) -> SelectionViewParams {
    let current = tui.mouse_scroll_speed.unwrap_or(1.0);
    let mut speeds = vec![0.5, 1.0, 1.5, 2.0, 3.0];
    let selected = match speeds.binary_search_by(|speed: &f64| speed.total_cmp(&current)) {
        Ok(index) => index,
        Err(index) => {
            speeds.insert(index, current);
            index
        }
    };
    SelectionViewParams {
        title: Some("Mouse scroll speed".to_string()),
        subtitle: Some("Choose a mouse-wheel speed for the fullscreen transcript".to_string()),
        footer_hint: Some(picker_hint_line_for_keymap(keymap)),
        initial_selected_idx: Some(selected),
        allow_input_when_disconnected: true,
        items: speeds
            .into_iter()
            .map(|speed| SelectionItem {
                name: if speed == 1.0 {
                    "1× (default)".to_string()
                } else {
                    format!("{speed}×")
                },
                is_current: speed == current,
                actions: vec![Box::new(move |tx| {
                    tx.send(AppEvent::SaveConfigPreference(
                        ConfigPreference::MouseScrollSpeed(speed),
                    ));
                })],
                dismiss_on_select: true,
                ..Default::default()
            })
            .collect(),
        ..SelectionViewParams::picker()
    }
}

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
            "Notify only while you are away or while this terminal is focused too".to_string(),
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
            "Copy transcript, footer, and composer selections on release; Auto follows terminal defaults"
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
            "Choose tighter rows or a roomier list when resuming and forking conversations"
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
            "Choose the working directory when resuming, with --cd taking precedence for this session"
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
                    Some("Requires --cd for remote sessions".to_string())
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

fn header(notice: Option<String>) -> Box<dyn Renderable> {
    let subtitle = notice.unwrap_or_else(|| "Changes save automatically".to_string());
    Box::new(ConfigPanelHeader(vec![
        Line::from("Configuration".bold()),
        Line::from(subtitle.dim()),
    ]))
}

fn hint(
    keymap: &ListKeymap,
    search: Option<KeyBinding>,
    accept_label: Option<&str>,
) -> Line<'static> {
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
        accept_label
            .map(|label| (ListAction::Accept, label))
            .into_iter()
            .chain([(ListAction::Cancel, "clear search/close")])
            .filter_map(|(action, label)| key_label(action).map(|key| (key, label.into()))),
    );
    footer_hint_items_line(&items)
}

#[derive(Clone, Copy)]
enum SettingItemContext {
    Group,
    Search,
}

fn setting_item(
    setting: &ConfigSetting,
    keymap: &ListKeymap,
    search_hint: Option<KeyBinding>,
    context: SettingItemContext,
) -> SelectionItem {
    let (unavailable, accept_label) = match &setting.value {
        ConfigSettingValue::DependentToggle {
            dependency_met: false,
            unavailable_reason,
            ..
        } => (Some(*unavailable_reason), None),
        ConfigSettingValue::Unavailable { reason } => (Some(*reason), None),
        ConfigSettingValue::Toggle { .. } | ConfigSettingValue::DependentToggle { .. } => {
            (None, Some("toggle"))
        }
        ConfigSettingValue::Editor { .. } => (None, Some("open")),
        ConfigSettingValue::NotificationTest => (None, Some("test")),
    };
    let value_label = setting.value.label();
    let description = unavailable.unwrap_or(setting.description);
    let search_footer_hint = footer_hint_items_line(
        &accept_label
            .map(|label| ("enter".to_string(), label.to_string()))
            .into_iter()
            .chain([("esc".to_string(), "clear search".to_string())])
            .collect::<Vec<_>>(),
    );
    let description = match context {
        SettingItemContext::Group => description.to_string(),
        SettingItemContext::Search => {
            format!("{} · {description}", setting.section.title())
        }
    };
    SelectionItem {
        name: setting.label.to_string(),
        category_tag: Some(format!("{value_label:<20}")),
        description: Some(description),
        is_disabled: unavailable.is_some(),
        footer_hint: Some(hint(keymap, search_hint, accept_label)),
        search_footer_hint: Some(search_footer_hint),
        search_value: Some(format!(
            "{} {} {} {} {}",
            setting.section.title(),
            setting.label,
            value_label,
            setting.keywords,
            setting.description
        )),
        ..Default::default()
    }
}

pub(super) fn picker(
    settings: &[ConfigSetting],
    app_event_tx: AppEventSender,
    keymap: ListKeymap,
    state: PickerState,
) -> ListSelectionView {
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
        description_layout: SelectionDescriptionLayout::SelectedBelowWhenNarrow {
            min_description_width: 45,
        },
        footer_hint: Some(hint(&keymap, search_hint, /*accept_label*/ None)),
        is_searchable: true,
        search_mode: SearchMode::OnDemand(search),
        search_placeholder: Some("Type to search settings".to_string()),
        search_items: settings
            .iter()
            .map(|setting| setting_item(setting, &keymap, search_hint, SettingItemContext::Search))
            .collect(),
        name_column_width: settings
            .iter()
            .map(|setting| UnicodeWidthStr::width(setting.label))
            .max(),
        tabs: ConfigSection::ALL
            .iter()
            .map(|section| {
                let title = section.title();
                SelectionTab {
                    id: title.to_string(),
                    label: title.to_string(),
                    header: header(state.notice.clone()),
                    items: settings
                        .iter()
                        .filter(|setting| setting.section == *section)
                        .map(|setting| {
                            setting_item(setting, &keymap, search_hint, SettingItemContext::Group)
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
