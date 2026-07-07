use super::*;
use crate::AltScreenBehavior;
use crate::app::conversation_panes::ConversationPane;
use crate::app::conversation_panes::ConversationPaneInit;
use crate::app::owned_screen_frame::OwnedScreenRightRailContent;
use crate::app::test_support::make_test_app;
use crate::app::test_support::make_test_app_with_channels;
use crate::app_event::OwnedScreenPanel;
use crate::app_event::OwnedScreenPanelPreference;
use crate::app_event::PaneSlot;
use crate::chatwidget::tests::constructor::make_chatwidget_for_pane_with_sender;
use crate::file_search::FileSearchManager;
use crate::legacy_core::config::ConfigBuilder;
use crate::legacy_core::config::ConfigOverrides;
use codex_config::LoaderOverrides;
use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::NetworkSandboxPolicy;
use color_eyre::eyre::Result;
use crossterm::event::KeyModifiers;
use pretty_assertions::assert_eq;
use ratatui::layout::Rect;
use std::time::Duration;

use crate::tui::MousePrimaryEvent;
use crate::tui::MousePrimaryEventKind;

#[test]
fn terminal_browser_requests_require_the_displayed_thread() {
    let displayed = ThreadId::new();
    let inactive = ThreadId::new();

    assert!(terminal_browser_request_matches_thread(
        Some(displayed),
        &displayed.to_string(),
    ));
    assert!(!terminal_browser_request_matches_thread(
        Some(displayed),
        &inactive.to_string(),
    ));
    assert!(!terminal_browser_request_matches_thread(
        /*active_thread_id*/ None,
        &displayed.to_string(),
    ));
}

#[test]
fn terminal_browser_control_click_outside_viewport_returns_to_app() {
    let viewport = Rect::new(
        /*x*/ 100, /*y*/ 1, /*width*/ 40, /*height*/ 20,
    );
    let outside_press = MousePrimaryEvent {
        kind: MousePrimaryEventKind::Press,
        column: 50,
        row: 10,
        modifiers: KeyModifiers::NONE,
    };

    assert!(App::terminal_browser_control_click_returns_to_app(
        outside_press,
        Some(viewport),
    ));
    assert!(!App::terminal_browser_control_click_returns_to_app(
        MousePrimaryEvent {
            column: 110,
            ..outside_press
        },
        Some(viewport),
    ));
    assert!(!App::terminal_browser_control_click_returns_to_app(
        MousePrimaryEvent {
            kind: MousePrimaryEventKind::Release,
            ..outside_press
        },
        Some(viewport),
    ));
}

#[tokio::test]
async fn terminal_browser_tools_remain_enabled_without_an_owned_screen() {
    let mut app = make_test_app().await;
    app.chat_widget
        .set_feature_enabled(Feature::TerminalBrowser, /*enabled*/ true);

    assert!(!app.has_owned_screen());
    assert!(app.terminal_browser_enabled());
    assert!(!app.terminal_browser_human_control_active());
}

#[tokio::test]
async fn profile_approval_expires_when_the_browser_generation_changes() {
    let mut app = make_test_app().await;
    let thread_id = ThreadId::new();
    app.chat_widget
        .by_slot_mut(PaneSlot::Parent)
        .expect("parent pane")
        .attach_thread(thread_id, /*receiver*/ None);
    app.terminal_browser = Some(Arc::new(TerminalBrowser::discover()));
    app.terminal_browser_owner_thread_id = Some(thread_id);
    app.terminal_browser_generation = 3;
    let approval = app
        .terminal_browser_profile_approval(TerminalBrowserProfileCommand::Ephemeral)
        .expect("current browser should produce an approval token");

    assert!(app.terminal_browser_profile_approval_is_current(&approval));
    app.terminal_browser_generation += 1;
    assert!(!app.terminal_browser_profile_approval_is_current(&approval));
}

#[tokio::test]
async fn stale_control_target_is_rejected_after_the_browser_epoch_changes() {
    let mut app = make_test_app().await;
    let thread_id = ThreadId::new();
    app.chat_widget
        .by_slot_mut(PaneSlot::Parent)
        .expect("parent pane")
        .attach_thread(thread_id, /*receiver*/ None);
    let browser = Arc::new(TerminalBrowser::discover());
    browser.set_visibility(/*visible*/ true);
    app.terminal_browser = Some(Arc::clone(&browser));
    app.terminal_browser_owner_thread_id = Some(thread_id);
    let stale_target = app
        .terminal_browser_control_target()
        .expect("current browser target");

    browser.set_visibility(/*visible*/ false);

    assert!(!app.terminal_browser_control_target_is_current(stale_target));
}

#[tokio::test]
async fn visible_browser_updates_select_the_shared_right_rail_and_hidden_updates_fall_back() {
    let mut app = make_test_app().await;
    app.chat_widget.owned_screen = App::owned_screen_for_behavior(
        AltScreenBehavior::Owned,
        &app.chat_widget,
        app.keymap.pager.clone(),
    );
    let thread_id = ThreadId::new();
    app.chat_widget
        .by_slot_mut(PaneSlot::Parent)
        .expect("parent pane")
        .attach_thread(thread_id, /*receiver*/ None);
    let browser = Arc::new(TerminalBrowser::discover());
    app.terminal_browser = Some(Arc::clone(&browser));
    app.terminal_browser_owner_thread_id = Some(thread_id);

    browser.set_visibility(/*visible*/ true);
    assert!(!app.sync_terminal_browser_panel());
    assert_eq!(
        app.owned_screen_frame.right_rail_content(),
        OwnedScreenRightRailContent::Browser
    );
    app.owned_screen_frame.focus_conversation();
    assert!(!app.sync_terminal_browser_panel());
    assert_eq!(
        app.owned_screen_frame.focus(),
        crate::app::owned_screen_frame::OwnedScreenFrameFocus::Conversation
    );

    app.hide_terminal_browser_panel();
    assert_eq!(
        app.owned_screen_frame.right_rail_content(),
        OwnedScreenRightRailContent::Summary
    );
    assert!(!browser.view().visible);
}

#[tokio::test]
async fn terminal_browser_control_command_focuses_the_browser_rail() {
    let mut app = make_test_app().await;
    app.chat_widget.owned_screen = App::owned_screen_for_behavior(
        AltScreenBehavior::Owned,
        &app.chat_widget,
        app.keymap.pager.clone(),
    );
    app.chat_widget
        .set_feature_enabled(Feature::TerminalBrowser, /*enabled*/ true);
    let thread_id = ThreadId::new();
    app.chat_widget
        .by_slot_mut(PaneSlot::Parent)
        .expect("parent pane")
        .attach_thread(thread_id, /*receiver*/ None);
    let browser = Arc::new(TerminalBrowser::discover());
    browser.set_visibility(/*visible*/ true);
    app.terminal_browser = Some(browser);
    app.terminal_browser_owner_thread_id = Some(thread_id);
    app.owned_screen_frame
        .set_right_rail_content(OwnedScreenRightRailContent::Browser);

    app.toggle_terminal_browser_control().await;

    assert_eq!(
        app.owned_screen_frame.focus(),
        crate::app::owned_screen_frame::OwnedScreenFrameFocus::Summary
    );
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
#[tokio::test]
async fn managed_network_without_runtime_blocks_panel_but_not_doctor() {
    let (mut app, mut app_event_rx, _op_rx) = make_test_app_with_channels().await;
    app.chat_widget.owned_screen = App::owned_screen_for_behavior(
        AltScreenBehavior::Owned,
        &app.chat_widget,
        app.keymap.pager.clone(),
    );
    let thread_id = ThreadId::new();
    app.chat_widget
        .by_slot_mut(PaneSlot::Parent)
        .expect("parent pane")
        .attach_thread(thread_id, /*receiver*/ None);
    let codex_home = tempfile::tempdir().expect("terminal-browser config home");
    let managed_config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .loader_overrides(LoaderOverrides::without_managed_config_for_tests())
        .cli_overrides(vec![
            (
                "features.terminal_browser".to_string(),
                toml::Value::Boolean(true),
            ),
            (
                "features.network_proxy".to_string(),
                toml::Value::Boolean(true),
            ),
        ])
        .harness_overrides(ConfigOverrides {
            permission_profile: Some(PermissionProfile::workspace_write_with(
                &[],
                NetworkSandboxPolicy::Enabled,
                /*exclude_tmpdir_env_var*/ false,
                /*exclude_slash_tmp*/ false,
            )),
            ..ConfigOverrides::default()
        })
        .build()
        .await
        .expect("managed terminal-browser config should build");
    app.chat_widget
        .set_feature_enabled(Feature::TerminalBrowser, /*enabled*/ true);
    app.chat_widget
        .set_permission_profile_with_active_profile(
            managed_config.permissions.effective_permission_profile(),
            managed_config.permissions.active_permission_profile(),
        )
        .expect("managed permission profile should apply");
    app.chat_widget
        .set_permission_network(managed_config.permissions.network);

    app.show_terminal_browser().await;

    assert!(app.terminal_browser.is_none());
    assert_eq!(
        app.owned_screen_frame.right_rail_content(),
        OwnedScreenRightRailContent::Summary
    );
    let panel_message = std::iter::from_fn(|| app_event_rx.try_recv().ok())
        .find_map(|event| match event {
            AppEvent::InsertHistoryCell(cell) => Some(cell),
            AppEvent::FromConversation { event, .. } => match *event {
                AppEvent::InsertHistoryCell(cell) => Some(cell),
                _ => None,
            },
            _ => None,
        })
        .expect("managed network warning")
        .display_lines(/*width*/ 80)
        .into_iter()
        .map(|line| line.to_string())
        .collect::<Vec<_>>()
        .join("\n");

    insta::assert_snapshot!(panel_message, @"• Terminal browser is unavailable because the managed network proxy is not ready for this session.");

    app.doctor_terminal_browser().await;

    let doctor_completed = tokio::time::timeout(Duration::from_secs(/*secs*/ 15), async {
        loop {
            match app_event_rx.recv().await {
                Some(AppEvent::TerminalBrowserDoctorCompleted { .. }) => break true,
                Some(AppEvent::FromConversation { event, .. })
                    if matches!(*event, AppEvent::TerminalBrowserDoctorCompleted { .. }) =>
                {
                    break true;
                }
                Some(_) => {}
                None => break false,
            }
        }
    })
    .await
    .expect("terminal-browser doctor should complete");
    assert!(doctor_completed);
    assert!(app.terminal_browser.is_none());
}

#[tokio::test]
async fn closing_detaches_and_reuses_the_profile_owning_runtime() {
    let mut app = make_test_app().await;
    app.chat_widget
        .set_feature_enabled(Feature::TerminalBrowser, /*enabled*/ true);
    let thread_id = ThreadId::new();
    app.chat_widget
        .by_slot_mut(PaneSlot::Parent)
        .expect("parent pane")
        .attach_thread(thread_id, /*receiver*/ None);
    let closing_browser = app.discover_terminal_browser();
    closing_browser
        .create_profile("work")
        .await
        .expect("create named profile");
    closing_browser.set_visibility(/*visible*/ true);
    app.terminal_browser = Some(Arc::clone(&closing_browser));
    app.terminal_browser_owner_thread_id = Some(thread_id);
    let generation = app.terminal_browser_generation;

    app.close_terminal_browser();

    assert!(app.terminal_browser.is_none());
    assert_eq!(app.terminal_browser_owner_thread_id, None);
    assert_eq!(
        app.terminal_browser_generation,
        generation.wrapping_add(/*rhs*/ 1)
    );
    assert!(!closing_browser.view().visible);
    assert!(app.terminal_browser_reopenable.contains_key(&thread_id));

    let other_thread_id = ThreadId::new();
    let other_browser = app
        .terminal_browser_for_thread(other_thread_id)
        .await
        .expect("open another thread's browser runtime");
    assert!(!Arc::ptr_eq(&closing_browser, &other_browser));
    assert!(app.terminal_browser_reopenable.contains_key(&thread_id));

    let reopened_browser = app
        .terminal_browser_for_thread(thread_id)
        .await
        .expect("reopen browser runtime");
    assert!(Arc::ptr_eq(&closing_browser, &reopened_browser));
    assert_eq!(reopened_browser.selected_profile().as_deref(), Some("work"));

    app.close_terminal_browser();
    assert!(app.terminal_browser_reopenable.contains_key(&thread_id));
    app.discard_reopenable_terminal_browser(thread_id);
    assert!(!app.terminal_browser_reopenable.contains_key(&thread_id));
}

#[tokio::test]
async fn summary_command_from_an_unfocused_pane_does_not_hide_the_focused_browser() -> Result<()> {
    let mut app = make_test_app().await;
    let parent_widget =
        make_chatwidget_for_pane_with_sender(PaneSlot::Parent, app.app_event_tx.clone()).await;
    app.chat_widget
        .by_slot_mut(PaneSlot::Parent)
        .expect("parent pane")
        .chat_widget = parent_widget;
    app.chat_widget.owned_screen = App::owned_screen_for_behavior(
        AltScreenBehavior::Owned,
        &app.chat_widget,
        app.keymap.pager.clone(),
    );
    let parent_thread_id = ThreadId::new();
    let side_thread_id = ThreadId::new();
    app.chat_widget
        .by_slot_mut(PaneSlot::Parent)
        .expect("parent pane")
        .attach_thread(parent_thread_id, /*receiver*/ None);
    let parent_origin = app
        .chat_widget
        .by_slot(PaneSlot::Parent)
        .and_then(ConversationPane::origin)
        .expect("parent origin");
    let side_widget =
        make_chatwidget_for_pane_with_sender(PaneSlot::Side, app.app_event_tx.clone()).await;
    let file_search = FileSearchManager::new(
        side_widget.config_ref().cwd.to_path_buf(),
        side_widget.conversation_event_sender(),
    );
    let owned_screen = App::owned_screen_for_behavior(
        AltScreenBehavior::Owned,
        &side_widget,
        app.keymap.pager.clone(),
    );
    assert!(
        app.chat_widget
            .install_side(ConversationPaneInit {
                chat_widget: side_widget,
                file_search,
                owned_screen,
            })
            .is_ok(),
        "side pane should install"
    );
    app.chat_widget
        .by_slot_mut(PaneSlot::Side)
        .expect("side pane")
        .attach_thread(side_thread_id, /*receiver*/ None);
    assert!(app.chat_widget.focus(PaneSlot::Side));
    let browser = Arc::new(TerminalBrowser::discover());
    browser.set_visibility(/*visible*/ true);
    app.terminal_browser = Some(Arc::clone(&browser));
    app.terminal_browser_owner_thread_id = Some(side_thread_id);
    app.owned_screen_frame
        .select_right_rail_content(OwnedScreenRightRailContent::Browser);
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let mut app_server =
        crate::start_embedded_app_server_for_picker(app.chat_widget.config_ref()).await?;

    app.handle_event(
        &mut tui,
        &mut app_server,
        AppEvent::FromConversation {
            target: parent_origin,
            event: Box::new(AppEvent::SetOwnedScreenPanel {
                panel: OwnedScreenPanel::Summary,
                preference: Some(OwnedScreenPanelPreference::Shown),
            }),
        },
    )
    .await?;

    assert!(browser.view().visible);
    assert_eq!(
        app.owned_screen_frame.right_rail_content(),
        OwnedScreenRightRailContent::Browser
    );
    app_server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn network_reconciliation_follows_browser_ownership_across_pane_focus() -> Result<()> {
    let mut app = make_test_app().await;
    let parent_widget =
        make_chatwidget_for_pane_with_sender(PaneSlot::Parent, app.app_event_tx.clone()).await;
    app.chat_widget
        .by_slot_mut(PaneSlot::Parent)
        .expect("parent pane")
        .chat_widget = parent_widget;
    let parent_thread_id = ThreadId::new();
    let side_thread_id = ThreadId::new();
    let parent = app
        .chat_widget
        .by_slot_mut(PaneSlot::Parent)
        .expect("parent pane");
    parent.attach_thread(parent_thread_id, /*receiver*/ None);
    parent
        .set_permission_profile_with_active_profile(
            PermissionProfile::read_only(),
            /*active_permission_profile*/ None,
        )
        .expect("set restricted parent permissions");
    let parent_origin = parent.origin().expect("parent origin");
    let side_widget =
        make_chatwidget_for_pane_with_sender(PaneSlot::Side, app.app_event_tx.clone()).await;
    let file_search = FileSearchManager::new(
        side_widget.config_ref().cwd.to_path_buf(),
        side_widget.conversation_event_sender(),
    );
    let owned_screen = App::owned_screen_for_behavior(
        AltScreenBehavior::Owned,
        &side_widget,
        app.keymap.pager.clone(),
    );
    let installed = app.chat_widget.install_side(ConversationPaneInit {
        chat_widget: side_widget,
        file_search,
        owned_screen,
    });
    assert!(installed.is_ok(), "side pane should install");
    app.chat_widget
        .by_slot_mut(PaneSlot::Side)
        .expect("side pane")
        .attach_thread(side_thread_id, /*receiver*/ None);
    let browser = Arc::new(TerminalBrowser::discover());
    browser.set_visibility(/*visible*/ true);
    app.terminal_browser = Some(Arc::clone(&browser));
    app.terminal_browser_owner_thread_id = Some(side_thread_id);
    assert!(app.chat_widget.focus(PaneSlot::Parent));
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let mut app_server =
        crate::start_embedded_app_server_for_picker(app.chat_widget.config_ref()).await?;

    app.handle_event(
        &mut tui,
        &mut app_server,
        AppEvent::FromConversation {
            target: parent_origin,
            event: Box::new(AppEvent::ReconcileTerminalBrowserNetworkPolicy {
                thread_id: parent_thread_id,
            }),
        },
    )
    .await?;

    assert!(
        browser.view().visible,
        "focused non-owner must not reconcile"
    );

    app.terminal_browser_owner_thread_id = Some(parent_thread_id);
    browser.set_visibility(/*visible*/ true);
    assert!(app.chat_widget.focus(PaneSlot::Side));
    app.handle_event(
        &mut tui,
        &mut app_server,
        AppEvent::FromConversation {
            target: parent_origin,
            event: Box::new(AppEvent::ReconcileTerminalBrowserNetworkPolicy {
                thread_id: parent_thread_id,
            }),
        },
    )
    .await?;

    assert!(
        !browser.view().visible,
        "unfocused owner must still reconcile its restricted policy"
    );
    app_server.shutdown().await?;
    Ok(())
}
