//! Shortcut picker construction for `/keymap`.

use codex_config::types::TuiKeymap;
use crossterm::event::KeyCode;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use unicode_width::UnicodeWidthStr;

use crate::app_event::AppEvent;
use crate::bottom_pane::ColumnWidthMode;
use crate::bottom_pane::SelectionItem;
use crate::bottom_pane::SelectionRowDisplay;
use crate::bottom_pane::SelectionTab;
use crate::bottom_pane::SelectionViewParams;
use crate::key_hint;
use crate::keymap::RuntimeKeymap;
use crate::keymap::primary_binding;
use crate::render::renderable::ColumnRenderable;
use crate::render::renderable::Renderable;

use super::actions::KEYMAP_ACTIONS;
use super::actions::action_label;
use super::actions::bindings_for_action;
use super::actions::format_binding_summary;
use super::has_custom_binding;

pub(crate) const KEYMAP_PICKER_VIEW_ID: &str = "keymap-picker";
pub(super) const KEYMAP_ALL_TAB_ID: &str = "all-shortcuts";
pub(super) const KEYMAP_COMMON_TAB_ID: &str = "common-shortcuts";
pub(super) const KEYMAP_CUSTOM_TAB_ID: &str = "custom-shortcuts";
pub(super) const KEYMAP_UNBOUND_TAB_ID: &str = "unbound-shortcuts";
const KEYMAP_CONTEXT_LABEL_WIDTH: usize = 12;
const KEYMAP_ROW_PREFIX_WIDTH: usize = KEYMAP_CONTEXT_LABEL_WIDTH + 3;

#[derive(Clone, Debug)]
struct KeymapActionRow {
    context: &'static str,
    context_label: &'static str,
    action: &'static str,
    label: String,
    description: &'static str,
    binding_summary: String,
    custom_binding: bool,
}

impl KeymapActionRow {
    fn is_unbound(&self) -> bool {
        self.binding_summary == "unbound"
    }
}

struct KeymapContextTab {
    id: &'static str,
    label: &'static str,
    description: &'static str,
    contexts: &'static [&'static str],
}

const KEYMAP_COMMON_ACTIONS: &[(&str, &str)] = &[
    ("composer", "submit"),
    ("editor", "insert_newline"),
    ("composer", "queue"),
    ("global", "open_external_editor"),
    ("global", "copy"),
    ("global", "toggle_vim_mode"),
    ("chat", "reasoning_down"),
    ("chat", "reasoning_up"),
    ("editor", "delete_backward_word"),
    ("editor", "delete_forward_word"),
    ("editor", "move_word_left"),
    ("editor", "move_word_right"),
    ("global", "open_transcript"),
    ("pager", "close"),
    ("pager", "page_up"),
    ("pager", "page_down"),
    ("approval", "open_fullscreen"),
    ("approval", "approve"),
    ("approval", "approve_for_session"),
    ("approval", "decline"),
    ("approval", "cancel"),
];

const KEYMAP_CONTEXT_TABS: &[KeymapContextTab] = &[
    KeymapContextTab {
        id: "app-shortcuts",
        label: "App",
        description: "Global and chat-level shortcuts.",
        contexts: &["global", "chat"],
    },
    KeymapContextTab {
        id: "composer-shortcuts",
        label: "Composer",
        description: "Composer submission and queue shortcuts.",
        contexts: &["composer"],
    },
    KeymapContextTab {
        id: "editor-shortcuts",
        label: "Editor",
        description: "Inline editor movement and editing shortcuts.",
        contexts: &["editor"],
    },
    KeymapContextTab {
        id: "vim-shortcuts",
        label: "Vim",
        description: "Vim normal-mode and operator shortcuts.",
        contexts: &["vim_normal", "vim_operator"],
    },
    KeymapContextTab {
        id: "navigation-shortcuts",
        label: "Navigation",
        description: "Pager and selection-list navigation shortcuts.",
        contexts: &["pager", "list"],
    },
    KeymapContextTab {
        id: "approval-shortcuts",
        label: "Approval",
        description: "Approval prompt shortcuts.",
        contexts: &["approval"],
    },
];

pub(crate) fn build_keymap_picker_params(
    runtime_keymap: &RuntimeKeymap,
    keymap_config: &TuiKeymap,
) -> SelectionViewParams {
    build_keymap_picker_params_for_action(
        runtime_keymap,
        keymap_config,
        /*selected_action*/ None,
    )
}

pub(crate) fn build_keymap_picker_params_for_selected_action(
    runtime_keymap: &RuntimeKeymap,
    keymap_config: &TuiKeymap,
    context: &str,
    action: &str,
) -> SelectionViewParams {
    build_keymap_picker_params_for_action(runtime_keymap, keymap_config, Some((context, action)))
}

fn build_keymap_picker_params_for_action(
    runtime_keymap: &RuntimeKeymap,
    keymap_config: &TuiKeymap,
    selected_action: Option<(&str, &str)>,
) -> SelectionViewParams {
    let rows = build_keymap_rows(runtime_keymap, keymap_config);
    let total = rows.len();
    let custom_count = rows.iter().filter(|row| row.custom_binding).count();
    let unbound_count = rows.iter().filter(|row| row.is_unbound()).count();
    let initial_selected_idx = selected_action.and_then(|(context, action)| {
        rows.iter()
            .position(|row| row.context == context && row.action == action)
    });
    let name_column_width = rows
        .iter()
        .map(|row| KEYMAP_ROW_PREFIX_WIDTH + UnicodeWidthStr::width(row.label.as_str()))
        .max();

    let mut tabs = Vec::new();
    tabs.push(SelectionTab {
        id: KEYMAP_ALL_TAB_ID.to_string(),
        label: "All".to_string(),
        header: keymap_header(
            "All configurable shortcuts.".to_string(),
            format!("{total} actions, {custom_count} customized, {unbound_count} unbound."),
        ),
        items: keymap_selection_items(
            rows.iter(),
            "No shortcuts available",
            "No configurable shortcuts are available.",
        ),
    });

    let common_rows = keymap_common_rows(&rows);
    let common_count = common_rows.len();
    tabs.push(SelectionTab {
        id: KEYMAP_COMMON_TAB_ID.to_string(),
        label: "Common".to_string(),
        header: keymap_header(
            "Frequently customized shortcuts.".to_string(),
            action_count_line(common_count),
        ),
        items: keymap_selection_items(
            common_rows,
            "No common shortcuts",
            "No common shortcut actions are available.",
        ),
    });

    let custom_rows = rows
        .iter()
        .filter(|row| row.custom_binding)
        .collect::<Vec<_>>();
    tabs.push(SelectionTab {
        id: KEYMAP_CUSTOM_TAB_ID.to_string(),
        label: format!("Customized ({custom_count})"),
        header: keymap_header(
            "Root-level shortcut overrides.".to_string(),
            action_count_line(custom_count),
        ),
        items: keymap_selection_items(
            custom_rows,
            "No customized shortcuts",
            "No root-level keymap overrides have been configured.",
        ),
    });

    let unbound_rows = rows
        .iter()
        .filter(|row| row.is_unbound())
        .collect::<Vec<_>>();
    tabs.push(SelectionTab {
        id: KEYMAP_UNBOUND_TAB_ID.to_string(),
        label: format!("Unbound ({unbound_count})"),
        header: keymap_header(
            "Actions without an active shortcut.".to_string(),
            action_count_line(unbound_count),
        ),
        items: keymap_selection_items(
            unbound_rows,
            "No unbound shortcuts",
            "Every configurable action currently has a shortcut.",
        ),
    });

    for tab in KEYMAP_CONTEXT_TABS {
        let tab_rows = rows
            .iter()
            .filter(|row| tab.contexts.contains(&row.context))
            .collect::<Vec<_>>();
        let count = tab_rows.len();
        tabs.push(SelectionTab {
            id: tab.id.to_string(),
            label: tab.label.to_string(),
            header: keymap_header(tab.description.to_string(), action_count_line(count)),
            items: keymap_selection_items(
                tab_rows,
                "No shortcuts in this group",
                "No configurable actions are available in this group.",
            ),
        });
    }

    SelectionViewParams {
        view_id: Some(KEYMAP_PICKER_VIEW_ID),
        header: Box::new(()),
        footer_hint: Some(keymap_picker_hint_line(runtime_keymap)),
        tabs,
        initial_tab_id: Some(KEYMAP_ALL_TAB_ID.to_string()),
        is_searchable: true,
        search_placeholder: Some("Type to search shortcuts".to_string()),
        col_width_mode: ColumnWidthMode::AutoAllRows,
        row_display: SelectionRowDisplay::SingleLine,
        name_column_width,
        initial_selected_idx,
        ..Default::default()
    }
}

fn build_keymap_rows(
    runtime_keymap: &RuntimeKeymap,
    keymap_config: &TuiKeymap,
) -> Vec<KeymapActionRow> {
    KEYMAP_ACTIONS
        .iter()
        .map(|descriptor| {
            let bindings =
                bindings_for_action(runtime_keymap, descriptor.context, descriptor.action)
                    .unwrap_or(&[]);
            KeymapActionRow {
                context: descriptor.context,
                context_label: descriptor.context_label,
                action: descriptor.action,
                label: action_label(descriptor.action),
                description: descriptor.description,
                binding_summary: format_binding_summary(bindings),
                custom_binding: has_custom_binding(
                    keymap_config,
                    descriptor.context,
                    descriptor.action,
                )
                .unwrap_or(false),
            }
        })
        .collect()
}

fn keymap_common_rows(rows: &[KeymapActionRow]) -> Vec<&KeymapActionRow> {
    KEYMAP_COMMON_ACTIONS
        .iter()
        .filter_map(|(context, action)| {
            rows.iter()
                .find(|row| row.context == *context && row.action == *action)
        })
        .collect()
}

fn keymap_selection_items<'a>(
    rows: impl IntoIterator<Item = &'a KeymapActionRow>,
    empty_name: &str,
    empty_description: &str,
) -> Vec<SelectionItem> {
    let items = rows
        .into_iter()
        .map(keymap_selection_item)
        .collect::<Vec<_>>();
    if items.is_empty() {
        return vec![SelectionItem {
            name: empty_name.to_string(),
            description: Some(empty_description.to_string()),
            is_disabled: true,
            ..Default::default()
        }];
    }

    items
}

fn keymap_selection_item(row: &KeymapActionRow) -> SelectionItem {
    let context = row.context.to_string();
    let action = row.action.to_string();
    let source = if row.custom_binding {
        "Custom"
    } else {
        "Default"
    };
    let search_value = format!(
        "{} {} {} {} {} {}",
        row.context_label, row.action, row.label, row.description, row.binding_summary, source
    );

    SelectionItem {
        name: row.label.clone(),
        name_prefix_spans: keymap_row_prefix(row),
        description: Some(row.binding_summary.clone()),
        selected_footer_note: Some(format!("{}: {}", row.label, row.description)),
        actions: vec![Box::new(move |tx| {
            tx.send(AppEvent::OpenKeymapActionMenu {
                context: context.clone(),
                action: action.clone(),
            });
        })],
        search_value: Some(search_value),
        ..Default::default()
    }
}

fn keymap_row_prefix(row: &KeymapActionRow) -> Vec<Span<'static>> {
    let indicator = if row.custom_binding {
        "*".cyan()
    } else if row.is_unbound() {
        "-".dim()
    } else {
        " ".into()
    };

    vec![
        format!(
            "{:<width$} ",
            row.context_label,
            width = KEYMAP_CONTEXT_LABEL_WIDTH
        )
        .dim(),
        indicator,
        " ".dim(),
    ]
}

fn keymap_header(description: String, summary: String) -> Box<dyn Renderable> {
    let mut header = ColumnRenderable::new();
    header.push(Line::from("Keymap".bold()));
    header.push(Line::from(description.dim()));
    header.push(Line::from(summary.dim()));
    Box::new(header)
}

fn action_count_line(count: usize) -> String {
    match count {
        1 => "1 action.".to_string(),
        _ => format!("{count} actions."),
    }
}

fn keymap_picker_hint_line(runtime_keymap: &RuntimeKeymap) -> Line<'static> {
    let accept = primary_binding(&runtime_keymap.list.accept)
        .unwrap_or_else(|| key_hint::plain(KeyCode::Enter));
    let cancel = primary_binding(&runtime_keymap.list.cancel)
        .unwrap_or_else(|| key_hint::plain(KeyCode::Esc));
    Line::from(vec![
        "left/right".cyan(),
        " group · ".dim(),
        accept.into(),
        " edit shortcut · ".dim(),
        "*".cyan(),
        " custom · ".dim(),
        "-".cyan(),
        " unbound · ".dim(),
        cancel.into(),
        " close".dim(),
    ])
}
