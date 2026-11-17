use super::*;
use crate::config_panel::ConfigPreference;
use crate::config_panel::ConfigToggle;
use pretty_assertions::assert_eq;

fn dispatch_inline(
    chat: &mut ChatWidget,
    events: &mut tokio::sync::mpsc::UnboundedReceiver<AppEvent>,
    command: SlashCommand,
    args: &str,
) -> Vec<AppEvent> {
    chat.dispatch_command_with_args(command, args.to_string(), Vec::new());
    std::iter::from_fn(|| events.try_recv().ok()).collect()
}

fn rendered_history(events: Vec<AppEvent>) -> String {
    events
        .into_iter()
        .filter_map(|event| match event {
            AppEvent::InsertHistoryCell(cell) => {
                Some(lines_to_single_string(&cell.display_lines(/*width*/ 80)))
            }
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn built_in_statusline_accepts_a_second_enter() {
    let (mut chat, mut events, _ops) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.restore_user_message_to_composer("/statusline".into());
    chat.handle_key_event(KeyCode::Enter.into());
    chat.handle_key_event(KeyCode::Enter.into());
    assert!(
        std::iter::from_fn(|| events.try_recv().ok())
            .any(|event| matches!(event, AppEvent::StatusLineSetup { .. }))
    );
}

#[tokio::test]
async fn inline_status_surface_commands_are_session_only_and_atomic() {
    let (mut chat, mut events, _ops) = make_chatwidget_manual(Some("gpt-5.5")).await;
    let saved_tui = chat.config_panel_tui.clone();
    super::status_surface_previews::cache_rate_limit_snapshot(&mut chat);

    let title_events = dispatch_inline(
        &mut chat,
        &mut events,
        SlashCommand::Title,
        "model-name monthly-limit",
    );
    assert_eq!(
        chat.local_settings.tui.terminal_title,
        Some(vec!["model".into(), "five-hour-limit".into()])
    );
    assert!(matches!(
        title_events.as_slice(),
        [AppEvent::SettingsSelectionClosed]
    ));

    let status_events = dispatch_inline(
        &mut chat,
        &mut events,
        SlashCommand::Statusline,
        "monthly-limit weekly-limit --no-colors",
    );
    assert_eq!(
        (
            chat.local_settings.tui.status_line.clone(),
            chat.local_settings.tui.status_line_use_colors,
        ),
        (
            Some(vec!["five-hour-limit".into(), "weekly-limit".into()]),
            false,
        )
    );
    assert!(matches!(
        status_events.as_slice(),
        [AppEvent::SettingsSelectionClosed]
    ));
    assert_eq!(chat.config_panel_tui, saved_tui);

    chat.rate_limit_snapshots_by_limit_id
        .get_mut("codex")
        .and_then(|snapshot| snapshot.secondary.as_mut())
        .expect("secondary usage limit")
        .window_minutes = Some(30 * 24 * 60);
    let before_ambiguous = chat.status_surface_overrides.clone();
    let ambiguous_events = dispatch_inline(
        &mut chat,
        &mut events,
        SlashCommand::Statusline,
        "monthly-limit",
    );
    assert_eq!(chat.status_surface_overrides, before_ambiguous);
    let rendered = rendered_history(ambiguous_events);
    insta::assert_snapshot!(rendered, @"■ Ambiguous status-line item 'monthly-limit'. Use 'five-hour-limit' or 'weekly-limit'.\n");

    dispatch_inline(
        &mut chat,
        &mut events,
        SlashCommand::Statusline,
        "model reasoning",
    );
    assert!(!chat.local_settings.tui.status_line_use_colors);

    let before_invalid = chat.status_surface_overrides.clone();
    let invalid_events = dispatch_inline(
        &mut chat,
        &mut events,
        SlashCommand::Statusline,
        "current-dir --colors --no-colors",
    );
    assert_eq!(chat.status_surface_overrides, before_invalid);
    assert!(
        invalid_events
            .iter()
            .any(|event| matches!(event, AppEvent::SettingsSelectionClosed))
    );
    let rendered = rendered_history(invalid_events);
    insta::assert_snapshot!(rendered, @"■ Specify only one of --colors or --no-colors.\n");

    dispatch_inline(&mut chat, &mut events, SlashCommand::Statusline, "--colors");
    assert_eq!(
        (
            chat.local_settings.tui.status_line.clone(),
            chat.local_settings.tui.status_line_use_colors,
        ),
        (Some(vec!["model".into(), "reasoning".into()]), true)
    );
    dispatch_inline(&mut chat, &mut events, SlashCommand::Title, "--clear");
    dispatch_inline(
        &mut chat,
        &mut events,
        SlashCommand::Statusline,
        "--clear --no-colors",
    );
    assert_eq!(
        (
            chat.local_settings.tui.terminal_title.clone(),
            chat.local_settings.tui.status_line.clone(),
            chat.local_settings.tui.status_line_use_colors,
        ),
        (Some(Vec::new()), Some(Vec::new()), false)
    );
}

#[tokio::test]
async fn session_status_surface_overrides_follow_thread_state_and_editor_saves() {
    let (mut chat, mut events, _ops) = make_chatwidget_manual(Some("gpt-5.5")).await;
    dispatch_inline(
        &mut chat,
        &mut events,
        SlashCommand::Title,
        "model reasoning",
    );
    dispatch_inline(
        &mut chat,
        &mut events,
        SlashCommand::Statusline,
        "current-dir --no-colors",
    );
    chat.preview_terminal_title(vec![TerminalTitleItem::AppName]);
    let state = chat
        .capture_thread_input_state()
        .expect("thread input state");

    chat.cancel_terminal_title_setup();
    assert_eq!(
        chat.local_settings.tui.terminal_title,
        Some(vec!["model".into(), "reasoning".into()])
    );

    let (mut restored, mut restored_events, _ops) = make_chatwidget_manual(Some("gpt-5.5")).await;
    restored.local_settings.tui.terminal_title = Some(vec!["app-name".into()]);
    restored.local_settings.tui.status_line = Some(vec!["codex-version".into()]);
    restored.restore_thread_input_state(
        Some(state),
        ThreadInputStateRestoreMode {
            preserve_in_flight_turn: false,
        },
    );
    assert_eq!(
        (
            restored.local_settings.tui.terminal_title.clone(),
            restored.local_settings.tui.status_line.clone(),
            restored.local_settings.tui.status_line_use_colors,
        ),
        (
            Some(vec!["model".into(), "reasoning".into()]),
            Some(vec!["current-dir".into()]),
            false,
        )
    );

    restored.restore_thread_input_state(
        /*input_state*/ None,
        ThreadInputStateRestoreMode {
            preserve_in_flight_turn: false,
        },
    );
    assert_eq!(
        restored.local_settings.tui.terminal_title,
        Some(vec!["model".into(), "reasoning".into()])
    );

    restored.setup_terminal_title(vec![TerminalTitleItem::Project]);
    restored.setup_status_line(
        vec![StatusLineItem::ModelName],
        /*use_theme_colors*/ true,
    );
    assert_eq!(
        restored.status_surface_overrides,
        StatusSurfaceOverrides::default()
    );

    dispatch_inline(
        &mut restored,
        &mut restored_events,
        SlashCommand::Statusline,
        "reasoning --no-colors",
    );
    let mut saved_tui = restored.local_settings.tui.clone();
    saved_tui.status_line_use_colors = true;
    restored.apply_config_preference(
        ConfigPreference::Toggle {
            setting: ConfigToggle::StatusLineColors,
            enabled: true,
        },
        &saved_tui,
    );
    assert_eq!(
        restored.status_surface_overrides,
        StatusSurfaceOverrides {
            status_line: Some(vec!["reasoning".into()]),
            ..Default::default()
        }
    );
}
