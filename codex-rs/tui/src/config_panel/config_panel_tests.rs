//! Snapshot each configuration group and exercise shared panel interactions.

use super::*;
use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::bottom_pane::BottomPaneView;
use crate::bottom_pane::CancellationEvent;
use crate::keymap::RuntimeKeymap;
use crate::render::renderable::Renderable;
use codex_config::types::CopyOnSelect;
use codex_config::types::Notifications;
use codex_config::types::ResumeCwdMode;
use codex_config::types::RightClickPaste;
use codex_config::types::SessionPickerViewMode;
use codex_config::types::Tui;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;
use pretty_assertions::assert_eq;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::sync::mpsc::unbounded_channel;

const STATUS: ConfigPanelStatus = ConfigPanelStatus {
    copy_on_select: true,
    right_click_paste: Ok(false),
    custom_notification_filters: false,
};

fn panel() -> (ConfigPanel, UnboundedReceiver<AppEvent>) {
    let (tx, rx) = unbounded_channel();
    (
        ConfigPanel::new(
            &Tui::default(),
            AppEventSender::new(tx),
            RuntimeKeymap::defaults().list,
            STATUS,
        ),
        rx,
    )
}

fn render(view: &impl Renderable, width: u16) -> String {
    let area = Rect::new(
        /*x*/ 0,
        /*y*/ 0,
        width,
        view.desired_height(width),
    );
    let mut buffer = Buffer::empty(area);
    view.render(area, &mut buffer);
    (0..area.height)
        .map(|y| {
            (0..area.width)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
                .trim_end()
                .to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn config_panel_renders_each_group_at_a_stable_height() {
    let (mut panel, _rx) = panel();
    let height = panel.desired_height(/*width*/ 140);
    for section in ConfigSection::ALL {
        assert_eq!(panel.picker.active_tab_id(), Some(section.title()));
        assert_eq!(panel.desired_height(/*width*/ 140), height);
        insta::assert_snapshot!(
            format!("config_{}", section.title()),
            render(&panel, /*width*/ 140)
        );
        panel
            .picker
            .handle_key_event(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
    }
}

#[test]
fn deselect_after_copy_requires_effective_copy_on_select() {
    let (mut panel, mut rx) = self::panel();
    panel.handle_key_event(KeyCode::Left.into());
    panel.handle_key_event(KeyCode::Char('/').into());
    assert!(panel.handle_paste("deselect after copy".to_string()));
    panel.update_config_panel(&ConfigPanelUpdate {
        tui: Tui::default(),
        keymap: RuntimeKeymap::defaults().list,
        status: ConfigPanelStatus {
            copy_on_select: false,
            ..STATUS
        },
        notice: None,
    });
    insta::assert_snapshot!(
        "deselect_after_copy_unavailable",
        render(&panel, /*width*/ 140)
    );
    panel.handle_key_event(KeyCode::Enter.into());
    assert!(rx.try_recv().is_err());

    let (mut panel, mut rx) = self::panel();
    panel.handle_key_event(KeyCode::Left.into());
    panel.handle_key_event(KeyCode::Char('/').into());
    assert!(panel.handle_paste("deselect after copy".to_string()));
    panel.handle_key_event(KeyCode::Enter.into());
    assert!(matches!(
        rx.try_recv().expect("save toggle event"),
        AppEvent::SaveConfigPreference(ConfigPreference::Toggle {
            setting: ConfigToggle::CopyOnSelectClearSelection,
            enabled: true,
        })
    ));
}

#[test]
fn notification_controls_render_dependency_states_and_condition_picker() {
    let panel_with_notifications = |notifications, custom_notification_filters| {
        let (tx, _rx) = unbounded_channel();
        let mut tui = Tui::default();
        tui.notification_settings.notifications = notifications;
        ConfigPanel::new(
            &tui,
            AppEventSender::new(tx),
            RuntimeKeymap::defaults().list,
            ConfigPanelStatus {
                custom_notification_filters,
                ..STATUS
            },
        )
    };
    let mut panel = panel_with_notifications(
        Notifications::Enabled(true),
        /*custom_notification_filters*/ true,
    );
    panel.handle_key_event(KeyCode::Right.into());
    insta::assert_snapshot!("config_notifications_custom", render(&panel, /*width*/ 100));

    let (tx, _rx) = unbounded_channel();
    let keymap = RuntimeKeymap::defaults().list;
    let picker = ListSelectionView::new(
        notification_condition(&Tui::default(), &keymap),
        AppEventSender::new(tx),
        keymap,
    );
    insta::assert_snapshot!(
        "config_notification_condition_choice",
        render(&picker, /*width*/ 100)
    );

    let mut panel = panel_with_notifications(
        Notifications::Enabled(false),
        /*custom_notification_filters*/ false,
    );
    panel.handle_key_event(KeyCode::Right.into());
    insta::assert_snapshot!("config_notifications_off", render(&panel, /*width*/ 100));
}

#[test]
fn filtered_selection_opens_the_matching_existing_editor() {
    let (mut panel, mut rx) = panel();
    panel.handle_key_event(KeyEvent::from(KeyCode::Char('/')));
    assert!(panel.handle_paste("terminal title".to_string()));
    panel.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    assert!(matches!(
        rx.try_recv().expect("open editor event"),
        AppEvent::OpenConfigEditor(handoff) if handoff.editor == ConfigEditor::TerminalTitle
    ));
    assert!(rx.try_recv().is_err());
    assert!(!panel.is_complete());
}

#[test]
fn on_demand_search_preserves_printable_list_bindings() {
    let (tx, mut rx) = unbounded_channel();
    let mut keymap = RuntimeKeymap::defaults().list;
    keymap.accept = vec![crate::key_hint::plain(KeyCode::Char('t'))];
    keymap.cancel = vec![crate::key_hint::plain(KeyCode::Char('h'))];
    let mut panel = ConfigPanel::new(
        &Tui::default(),
        AppEventSender::new(tx),
        keymap.clone(),
        STATUS,
    );

    panel.handle_key_event(KeyEvent::from(KeyCode::Char('t')));
    assert!(matches!(
        rx.try_recv().expect("configured accept binding"),
        AppEvent::OpenConfigEditor(handoff) if handoff.editor == ConfigEditor::Theme
    ));

    let (tx, mut rx) = unbounded_channel();
    let mut panel = ConfigPanel::new(&Tui::default(), AppEventSender::new(tx), keymap, STATUS);
    panel.handle_key_event(KeyEvent::from(KeyCode::Char('/')));

    for character in "theme".chars() {
        panel.handle_key_event(KeyEvent::from(KeyCode::Char(character)));
    }
    assert_eq!(panel.picker.search_query(), Some("theme"));
    let area = Rect::new(
        /*x*/ 0,
        /*y*/ 0,
        /*width*/ 84,
        panel.desired_height(/*width*/ 84),
    );
    let mut buffer = Buffer::empty(area);
    panel.render(area, &mut buffer);
    assert!(panel.cursor_pos(area).is_some());
    panel.handle_key_event(KeyEvent::from(KeyCode::Enter));
    assert!(matches!(
        rx.try_recv().expect("open theme editor event"),
        AppEvent::OpenConfigEditor(handoff) if handoff.editor == ConfigEditor::Theme
    ));
}

#[test]
fn search_matches_displayed_values() {
    let (tx, mut rx) = unbounded_channel();
    let mut panel = ConfigPanel::new(
        &Tui {
            theme: Some("Dracula".to_string()),
            ..Default::default()
        },
        AppEventSender::new(tx),
        RuntimeKeymap::defaults().list,
        STATUS,
    );
    panel.handle_key_event(KeyEvent::from(KeyCode::Char('/')));
    assert!(panel.handle_paste("Dracula".to_string()));
    panel.handle_key_event(KeyEvent::from(KeyCode::Enter));
    assert!(matches!(
        rx.try_recv().expect("open displayed theme editor event"),
        AppEvent::OpenConfigEditor(handoff) if handoff.editor == ConfigEditor::Theme
    ));
}

#[test]
fn session_picker_layout_uses_editor_handoff_and_saves_choice() {
    let (mut panel, mut rx) = panel();
    for _ in 0..3 {
        panel.handle_key_event(KeyCode::Right.into());
    }
    panel.handle_key_event(KeyCode::Char('/').into());
    assert!(panel.handle_paste("session picker layout".to_string()));
    panel.handle_key_event(KeyCode::Enter.into());
    assert!(matches!(
        rx.try_recv().expect("open editor event"),
        AppEvent::OpenConfigEditor(handoff)
            if handoff.editor == ConfigEditor::SessionPickerLayout
    ));

    let (tx, mut rx) = unbounded_channel();
    let keymap = RuntimeKeymap::defaults().list;
    let mut picker = ListSelectionView::new(
        session_picker_layout(&Tui::default(), &keymap),
        AppEventSender::new(tx),
        keymap,
    );
    insta::assert_snapshot!(
        "config_session_picker_layout_choice",
        render(&picker, /*width*/ 100)
    );
    picker.handle_key_event(KeyCode::Down.into());
    picker.handle_key_event(KeyCode::Enter.into());

    assert!(matches!(
        rx.try_recv().expect("save choice event"),
        AppEvent::SaveConfigPreference(ConfigPreference::SessionPickerLayout(
            SessionPickerViewMode::Comfortable,
        ))
    ));
}

#[test]
fn copy_on_select_uses_editor_handoff_and_saves_choice() {
    let (mut panel, mut rx) = panel();
    for _ in 0..4 {
        panel.handle_key_event(KeyCode::Right.into());
    }
    panel.handle_key_event(KeyCode::Char('/').into());
    assert!(panel.handle_paste("copy on select".to_string()));
    panel.handle_key_event(KeyCode::Enter.into());
    assert!(matches!(
        rx.try_recv().expect("open editor event"),
        AppEvent::OpenConfigEditor(handoff) if handoff.editor == ConfigEditor::CopyOnSelect
    ));

    let (tx, mut rx) = unbounded_channel();
    let keymap = RuntimeKeymap::defaults().list;
    let mut picker = ListSelectionView::new(
        copy_on_select(&Tui::default(), &keymap),
        AppEventSender::new(tx),
        keymap,
    );
    insta::assert_snapshot!(
        "config_copy_on_select_choice",
        render(&picker, /*width*/ 100)
    );
    picker.handle_key_event(KeyCode::Down.into());
    picker.handle_key_event(KeyCode::Enter.into());

    assert!(matches!(
        rx.try_recv().expect("save choice event"),
        AppEvent::SaveConfigPreference(ConfigPreference::CopyOnSelect(CopyOnSelect::Always))
    ));
}

#[test]
fn unavailable_right_click_paste_remains_editable() {
    let (tx, mut rx) = unbounded_channel();
    let mut panel = ConfigPanel::new(
        &Tui {
            right_click_paste: RightClickPaste::On,
            ..Tui::default()
        },
        AppEventSender::new(tx),
        RuntimeKeymap::defaults().list,
        ConfigPanelStatus {
            right_click_paste: Err(
                "Unavailable in inline mode. Use your terminal's paste command.",
            ),
            ..STATUS
        },
    );
    for _ in 0..4 {
        panel.handle_key_event(KeyCode::Right.into());
    }
    panel.handle_key_event(KeyCode::Down.into());
    panel.handle_key_event(KeyCode::Down.into());
    insta::assert_snapshot!(
        "config_right_click_paste_unavailable",
        render(&panel, /*width*/ 140)
    );
    panel.handle_key_event(KeyCode::Enter.into());
    assert!(matches!(
        rx.try_recv().expect("open editor event"),
        AppEvent::OpenConfigEditor(handoff) if handoff.editor == ConfigEditor::RightClickPaste
    ));

    let (tx, mut rx) = unbounded_channel();
    let keymap = RuntimeKeymap::defaults().list;
    let mut picker = ListSelectionView::new(
        right_click_paste(&Tui::default(), &keymap),
        AppEventSender::new(tx),
        keymap,
    );
    insta::assert_snapshot!(
        "config_right_click_paste_choice",
        render(&picker, /*width*/ 100)
    );
    picker.handle_key_event(KeyCode::Down.into());
    picker.handle_key_event(KeyCode::Enter.into());
    assert!(matches!(
        rx.try_recv().expect("save choice event"),
        AppEvent::SaveConfigPreference(ConfigPreference::RightClickPaste(RightClickPaste::On))
    ));
}

#[test]
fn resume_directory_disables_current_without_remote_cwd_override() {
    let (tx, mut rx) = unbounded_channel();
    let keymap = RuntimeKeymap::defaults().list;
    let mut picker = ListSelectionView::new(
        resume_directory(
            &Tui::default(),
            &keymap,
            ResumeCurrentAvailability::RequiresCwdOverride,
        ),
        AppEventSender::new(tx),
        keymap,
    );
    insta::assert_snapshot!(
        "config_resume_directory_remote",
        render(&picker, /*width*/ 100)
    );
    picker.handle_key_event(KeyCode::Down.into());
    picker.handle_key_event(KeyCode::Enter.into());

    assert!(matches!(
        rx.try_recv().expect("save resume directory event"),
        AppEvent::SaveConfigPreference(ConfigPreference::ResumeDirectory(Some(
            ResumeCwdMode::Session,
        )))
    ));
}

#[test]
fn refresh_preserves_search_and_updates_visible_state() {
    let (mut panel, mut rx) = panel();
    panel.handle_key_event(KeyEvent::from(KeyCode::Char('/')));
    assert!(panel.handle_paste("animations".to_string()));
    panel.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(matches!(
        rx.try_recv().expect("save toggle event"),
        AppEvent::SaveConfigPreference(ConfigPreference::Toggle {
            setting: ConfigToggle::Animations,
            enabled: true,
        })
    ));
    assert!(panel.saving);
    let mut keymap = RuntimeKeymap::defaults().list;
    keymap.move_left.clear();
    keymap.move_right.clear();
    panel.update_config_panel(&ConfigPanelUpdate {
        tui: Tui {
            animations: true,
            ..Default::default()
        },
        keymap,
        status: STATUS,
        notice: Some("Saved · restart Codex to apply".to_string()),
    });

    assert_eq!(panel.picker.search_query(), Some("animations"));
    assert!(!panel.saving);
    panel.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(matches!(
        rx.try_recv().expect("immediate second press saves again"),
        AppEvent::SaveConfigPreference(ConfigPreference::Toggle {
            setting: ConfigToggle::Animations,
            enabled: false,
        })
    ));
    insta::assert_snapshot!(render(&panel, /*width*/ 140));
}

#[test]
fn config_panel_wraps_complete_save_error() {
    let (mut panel, _rx) = panel();
    panel.update_config_panel(&ConfigPanelUpdate {
        tui: Tui::default(),
        keymap: RuntimeKeymap::defaults().list,
        status: STATUS,
        notice: Some(
            "Failed to save status line settings: permission denied while writing config.toml"
                .to_string(),
        ),
    });

    insta::assert_snapshot!(render(&panel, /*width*/ 50));
}

#[test]
fn editor_handoff_blocks_input_until_acknowledged() {
    let (tx, mut rx) = unbounded_channel();
    let mut keymap = RuntimeKeymap::defaults().list;
    keymap.accept = vec![crate::key_hint::plain(KeyCode::BackTab)];
    let mut panel = ConfigPanel::new(&Tui::default(), AppEventSender::new(tx), keymap, STATUS);
    panel.handle_key_event(KeyEvent::from(KeyCode::Char('/')));
    assert!(panel.handle_paste("terminal title".to_string()));
    panel.handle_key_event(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT));
    let AppEvent::OpenConfigEditor(handoff) = rx.try_recv().expect("open editor event") else {
        panic!("expected config editor handoff");
    };
    assert_eq!(
        (handoff.editor, handoff.activation_key),
        (
            ConfigEditor::TerminalTitle,
            crate::key_hint::plain(KeyCode::BackTab)
        )
    );
    let activated_at = handoff.active_at().expect("active handoff");

    panel.handle_key_event(KeyEvent::new_with_kind(
        KeyCode::Tab,
        KeyModifiers::NONE,
        KeyEventKind::Repeat,
    ));
    assert!(
        handoff
            .active_at()
            .is_some_and(|seen_at| seen_at > activated_at)
    );
    panel.handle_key_release(KeyEvent::new_with_kind(
        KeyCode::Tab,
        KeyModifiers::NONE,
        KeyEventKind::Release,
    ));
    assert_eq!(handoff.active_at(), None);
    panel.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert_eq!(panel.on_ctrl_c(), CancellationEvent::Handled);
    assert!(!panel.handle_paste(" ignored".to_string()));
    assert!(!panel.is_complete());
    assert!(rx.try_recv().is_err());

    panel.update_config_panel(&ConfigPanelUpdate {
        tui: Tui::default(),
        keymap: RuntimeKeymap::defaults().list,
        status: STATUS,
        notice: None,
    });
    panel.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(rx.try_recv().is_err());

    panel.config_editor_opened();
    panel.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(matches!(
        rx.try_recv().expect("reopened editor event"),
        AppEvent::OpenConfigEditor(handoff) if handoff.editor == ConfigEditor::TerminalTitle
    ));
}
