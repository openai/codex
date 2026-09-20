//! Dashboard rows, paging, metadata clipping and status-filter regressions.

use super::*;
use pretty_assertions::assert_eq;

fn screen(view: &AgentsOverviewView, width: u16, height: u16) -> String {
    let area = Rect::new(/*x*/ 0, /*y*/ 0, width, height);
    let mut buf = ratatui::buffer::Buffer::empty(area);
    view.render(area, &mut buf);
    crate::chatwidget::tests::helpers::normalize_agent_center_snapshot(
        buf.content
            .chunks(usize::from(width))
            .map(|row| {
                row.iter()
                    .map(ratatui::buffer::Cell::symbol)
                    .collect::<String>()
                    .trim_end()
                    .to_owned()
            })
            .collect::<Vec<_>>()
            .join("\n"),
    )
}

#[tokio::test]
async fn live_center_columns() {
    let mut app = make_test_app().await;
    let current = ThreadId::from_u128(/*value*/ 42);
    app.primary_thread_id = Some(current);
    let now = chrono::Utc::now().timestamp();
    let fixtures = [
        ("Current task", ThreadStatus::Idle, -3600),
        (
            "A longer task title that uses the available column",
            ThreadStatus::Idle,
            330,
        ),
        (
            "Needs an answer",
            ThreadStatus::Active {
                active_flags: vec![ThreadActiveFlag::WaitingOnUserInput],
            },
            7230,
        ),
        ("Unloaded task", ThreadStatus::NotLoaded, 172830),
        (
            "Working task",
            ThreadStatus::Active {
                active_flags: Vec::new(),
            },
            86430,
        ),
        ("Failed task", ThreadStatus::SystemError, 3630),
    ];
    let threads = fixtures
        .iter()
        .enumerate()
        .map(|(index, (title, status, age))| {
            let mut thread = overview_thread(
                ThreadId::from_u128(index as u128 + 42),
                /*parent_thread_id*/ None,
                title,
                status.clone(),
            );
            thread.updated_at = now - age;
            thread
        })
        .collect::<Vec<_>>();
    let view = app.agents_overview_view(threads, Some(current));
    insta::assert_snapshot!(screen(&view, /*width*/ 160, /*height*/ 22));
}

#[tokio::test]
async fn live_center_pages_visible_rows_without_wrapping() {
    for (count, offline) in [(5, false), (30, true)] {
        let app = make_test_app().await;
        let first = ThreadId::from_u128(/*value*/ 100);
        let threads = (0..count)
            .map(|index| {
                let mut thread = overview_thread(
                    ThreadId::from_u128(100 + index),
                    /*parent_thread_id*/ None,
                    &format!("Paging task {index:02}"),
                    ThreadStatus::Idle,
                );
                thread.updated_at -= index as i64;
                if index >= 10 {
                    thread.cwd = test_path_buf("/tmp/second-project").abs();
                }
                thread
            })
            .collect();
        let mut view = app.agents_overview_view(threads, Some(first));
        if offline {
            app.agents_overview
                .view_state
                .lock()
                .unwrap()
                .connection_notice = Some("Reconnecting…");
            view.handle_key_event(KeyCode::Char('f').into());
            view.handle_paste("Paging".into());
        }
        screen(&view, /*width*/ 110, /*height*/ 18);
        view.handle_key_event(KeyCode::PageDown.into());
        // Eleven visible rows: skip the next heading/gap, or clamp a short list.
        assert_eq!(
            view.rows[view.selected_index().unwrap()].thread_id,
            ThreadId::from_u128(100 + (count - 1).min(10)),
        );
        let page_down = view.selected_index().unwrap();
        view.handle_key_event(KeyCode::PageUp.into());
        assert!(view.selected_index().unwrap() < page_down);
        view.handle_key_event(KeyCode::PageUp.into());
        assert_eq!(view.rows[view.selected_index().unwrap()].thread_id, first);
        // Repeated pages eventually reach the last filtered task and stay there.
        for _ in 0..count {
            view.handle_key_event(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::CONTROL));
            screen(&view, /*width*/ 110, /*height*/ 18);
        }
        let last = view.rows[view.selected_index().unwrap()].thread_id;
        view.handle_key_event(KeyCode::PageDown.into());
        assert_eq!(
            (last, view.rows[view.selected_index().unwrap()].thread_id),
            (ThreadId::from_u128(100 + count - 1), last)
        );
        view.handle_key_event(KeyCode::Down.into());
        assert!(
            screen(&view, /*width*/ 110, /*height*/ 18)
                .lines()
                .any(|line| line.trim_start().starts_with("/tmp/project"))
        );
    }
}

#[tokio::test]
async fn live_center_metadata_clips_at_grapheme_boundaries() {
    let app = make_test_app().await;
    let mut view = app.agents_overview_view(
        vec![overview_thread(
            ThreadId::new(),
            /*parent_thread_id*/ None,
            "Retained task",
            ThreadStatus::Idle,
        )],
        /*selected_thread_id*/ None,
    );
    let input = format!("{}日本語 e\u{301} 👩\u{200d}💻", "界".repeat(/*n*/ 100_000));
    for (key, label) in [('r', "Rename › "), ('f', "Search › ")] {
        view.handle_key_event(KeyCode::Char(key).into());
        view.handle_paste(input.clone());
        let rendered = screen(&view, /*width*/ 40, /*height*/ 18);
        let prompt = rendered.lines().find(|line| line.contains(label)).unwrap();
        assert!(prompt.contains("e\u{301}"));
        assert!(prompt.ends_with("👩\u{200d}💻"));
        assert_eq!(
            view.cursor_pos(Rect::new(
                /*x*/ 0, /*y*/ 0, /*width*/ 40, /*height*/ 18
            )),
            Some((36, 3))
        );
        if key == 'r' {
            insta::assert_snapshot!("live_center_narrow_rename", rendered);
        }
        view.handle_key_event(KeyCode::Esc.into());
    }
}

#[tokio::test]
async fn live_center_rename_retains_target_when_status_leaves_filter() -> Result<()> {
    let mut app = make_test_app().await;
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    app.app_event_tx = AppEventSender::new(tx);
    let id = ThreadId::new();
    let other = ThreadId::new();
    let mut threads = vec![
        overview_thread(
            id,
            /*parent_thread_id*/ None,
            "Target",
            ThreadStatus::Idle,
        ),
        overview_thread(
            other,
            /*parent_thread_id*/ None,
            "Other",
            ThreadStatus::Idle,
        ),
    ];
    for key in [KeyCode::Esc, KeyCode::Enter] {
        threads[0].status = ThreadStatus::Idle;
        let mut view = app.agents_overview_view(threads.clone(), Some(id));
        // Reuse the Ready tab after the first iteration.
        if key == KeyCode::Esc {
            for _ in 0..3 {
                view.handle_key_event(KeyCode::Tab.into());
            }
        }
        view.handle_key_event(KeyCode::Char('r').into());
        threads[0].status = ThreadStatus::SystemError;
        view = app.agents_overview_view(threads.clone(), Some(id));
        view.handle_key_event(KeyCode::Char('!').into());
        view.handle_key_event(key.into());
        assert_eq!(view.rows[view.selected_index().unwrap()].thread_id, other);
        if key == KeyCode::Enter {
            let rename = rx.try_recv().unwrap();
            assert!(
                matches!(&rename, AppEvent::RenameAgentsOverviewThread { thread_id, name } if *thread_id == id && name == "Target!")
            );
            app.agents_overview.threads = threads
                .iter()
                .map(|thread| {
                    (
                        ThreadId::from_string(&thread.id).unwrap(),
                        Some(thread.clone()),
                    )
                })
                .collect();
            app.agents_overview.visible_thread_ids = view.thread_ids();
            app.chat_widget.show_bottom_pane_view(Box::new(view));
            let mut server = crate::start_embedded_app_server_for_picker(&app.config).await?;
            let mut tui = crate::tui::test_support::make_test_tui()?;
            // Synthetic IDs have no rollout: the server rejects the rename.
            Box::pin(app.handle_event(&mut tui, &mut server, rename)).await?;
            let mut retry = app.agents_overview_view(threads.clone(), Some(other));
            assert_eq!(retry.rows[retry.selected_index().unwrap()].thread_id, id);
            retry.handle_key_event(KeyCode::Enter.into());
            assert!(std::iter::from_fn(|| rx.try_recv().ok()).any(|event| matches!(event, AppEvent::RenameAgentsOverviewThread { thread_id, name } if thread_id == id && name == "Target!")));
            server.shutdown().await?;
        } else {
            assert!(rx.try_recv().is_err());
        }
    }
    Ok(())
}

#[tokio::test]
async fn live_center_navigation_and_complete_hints() {
    let mut app = make_test_app().await;
    app.config.features.enable(Feature::Worktrees).unwrap();
    let ready = ThreadId::from_u128(/*value*/ 42);
    let needs_you = ThreadId::from_u128(/*value*/ 43);
    let unloaded = ThreadId::from_u128(/*value*/ 44);
    let mut view = app.agents_overview_view(
        vec![
            overview_thread(
                ready,
                /*parent_thread_id*/ None,
                "Ready task",
                ThreadStatus::Idle,
            ),
            overview_thread(
                needs_you,
                /*parent_thread_id*/ None,
                "Needs input",
                ThreadStatus::SystemError,
            ),
            overview_thread(
                unloaded,
                /*parent_thread_id*/ None,
                "Unloaded task",
                ThreadStatus::NotLoaded,
            ),
        ],
        Some(ready),
    );
    let selected = |view: &AgentsOverviewView| view.rows[view.selected_index().unwrap()].thread_id;
    view.handle_key_event(KeyCode::Tab.into());
    assert_eq!(selected(&view), needs_you);
    view.handle_key_event(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT));
    view.handle_key_event(KeyEvent::new(KeyCode::Tab, KeyModifiers::SHIFT));
    assert_eq!(selected(&view), unloaded);
    insta::assert_snapshot!(
        "live_center_status_filter",
        screen(&view, /*width*/ 40, /*height*/ 12)
    );
    view.handle_key_event(KeyCode::Tab.into());

    let repeat_help =
        KeyEvent::new_with_kind(KeyCode::Char('?'), KeyModifiers::NONE, KeyEventKind::Repeat);
    view.handle_key_event(KeyCode::Char('?').into());
    view.handle_key_event(repeat_help);
    for (name, width, height) in [
        ("live_center_help_three_columns", 100, 24),
        ("live_center_help_two_columns", 60, 24),
        ("live_center_help_one_column", 40, 32),
        ("live_center_navigation_and_complete_hints", 40, 24),
    ] {
        insta::assert_snapshot!(name, screen(&view, width, height));
    }
    view.handle_key_event(KeyCode::Esc.into());
    view.handle_key_event(repeat_help);
    assert!(screen(&view, /*width*/ 40, /*height*/ 24).contains("Tasks"));
    assert!(!view.is_complete());
    view.handle_key_event(KeyCode::Esc.into());
    assert!(view.is_complete());
}

#[tokio::test]
async fn live_center_fixed_shortcuts_yield_to_configured_actions() {
    let mut app = make_test_app().await;
    let config: TuiKeymap = toml::from_str("[list]\ncancel = 'q'").unwrap();
    app.keymap = RuntimeKeymap::from_config(&config).unwrap();
    for editor_key in ['f', 'r'] {
        let mut view = app.agents_overview_view(
            vec![overview_thread(
                ThreadId::from_u128(/*value*/ 42),
                /*parent_thread_id*/ None,
                "Task",
                ThreadStatus::Idle,
            )],
            /*selected_thread_id*/ None,
        );
        view.handle_key_event(KeyCode::Char(editor_key).into());
        assert!(
            app.agents_overview
                .view_state
                .lock()
                .unwrap()
                .editing_metadata()
        );
        view.handle_key_event(KeyCode::Char('q').into());
        assert!(
            !app.agents_overview
                .view_state
                .lock()
                .unwrap()
                .editing_metadata()
        );
        assert!(!view.is_complete());
    }
    for (binding, key) in [
        ("tab", KeyEvent::from(KeyCode::Tab)),
        ("?", KeyEvent::from(KeyCode::Char('?'))),
        (
            "shift-?",
            KeyEvent::new(KeyCode::Char('?'), KeyModifiers::SHIFT),
        ),
        ("f9", KeyEvent::from(KeyCode::F(9))),
        ("enter", KeyEvent::from(KeyCode::Enter)),
    ] {
        let mut app = make_test_app().await;
        let config: TuiKeymap = toml::from_str(&format!(
            "[list]\ncancel = 'f9'\n[agents]\nresume = '{binding}'"
        ))
        .unwrap();
        app.keymap = RuntimeKeymap::from_config(&config).unwrap();
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        app.app_event_tx = AppEventSender::new(tx);
        let mut view = app.agents_overview_view(Vec::new(), /*selected_thread_id*/ None);
        view.handle_key_event(KeyCode::Esc.into());
        assert!(!view.is_complete());
        if key.code == KeyCode::Char('?') {
            assert!(!screen(&view, /*width*/ 100, /*height*/ 24).contains("? help"));
        }
        if key.code == KeyCode::Enter {
            assert!(!screen(&view, /*width*/ 100, /*height*/ 24).contains("enter open"));
            view.handle_key_event(KeyCode::Char('?').into());
            assert!(!screen(&view, /*width*/ 100, /*height*/ 24).contains("enter open"));
            app.agents_overview
                .view_state
                .lock()
                .unwrap()
                .server_version_notice = Some("Version notice".into());
            assert!(screen(&view, /*width*/ 100, /*height*/ 24).contains("f9 back"));
            view.handle_key_event(KeyCode::Esc.into());
            assert!(screen(&view, /*width*/ 100, /*height*/ 24).contains("Task shortcuts"));
            view.handle_key_event(KeyCode::F(9).into());
            assert!(!screen(&view, /*width*/ 100, /*height*/ 24).contains("Task shortcuts"));
            app.agents_overview
                .view_state
                .lock()
                .unwrap()
                .server_version_notice = None;
            view.handle_key_event(KeyCode::Char('f').into());
            assert!(!screen(&view, /*width*/ 100, /*height*/ 24).contains("enter open"));
        }
        view.handle_key_event(key);
        assert!(!view.is_complete());
        assert!(
            matches!(rx.try_recv(), Ok(AppEvent::OpenResumePicker)),
            "{binding}"
        );
        assert!(rx.try_recv().is_err());
    }
}

#[tokio::test]
async fn live_center_hints_follow_configured_bindings() {
    let mut app = make_test_app().await;
    app.config.features.disable(Feature::Worktrees).unwrap();
    let config: TuiKeymap = toml::from_str(
        r#"
[list]
move_up = 'k'
move_down = 'j'
page_up = []
[agents]
archive = []
rename = 'z r'
"#,
    )
    .unwrap();
    app.keymap = RuntimeKeymap::from_config(&config).unwrap();
    let mut view = app.agents_overview_view(Vec::new(), /*selected_thread_id*/ None);
    insta::assert_snapshot!(
        "live_center_custom_footer",
        screen(&view, /*width*/ 100, /*height*/ 12)
    );
    view.handle_key_event(KeyCode::Char('?').into());
    insta::assert_snapshot!(
        "live_center_custom_help",
        screen(&view, /*width*/ 100, /*height*/ 24)
    );
}

#[tokio::test]
async fn live_center_search_row_appears_only_while_editing() {
    let app = make_test_app().await;
    let mut view = app.agents_overview_view(
        vec![overview_thread(
            ThreadId::from_u128(/*value*/ 42),
            /*parent_thread_id*/ None,
            "Task to find",
            ThreadStatus::Idle,
        )],
        /*selected_thread_id*/ None,
    );
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 88, /*height*/ 16,
    );
    let mut buf = ratatui::buffer::Buffer::empty(area);
    view.render(area, &mut buf);
    let header = &buf.content[3 * 88..4 * 88];
    let text = header
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect::<String>();
    let styles =
        ["Tasks", "Status", "Updated"].map(|label| header[text.find(label).unwrap()].style());
    assert_eq!(styles, [styles[0]; 3]);
    let idle = screen(&view, area.width, area.height);
    view.handle_key_event(KeyCode::Char('f').into());
    view.handle_paste("find".into());
    insta::assert_snapshot!(
        "live_center_search_active",
        screen(&view, area.width, area.height)
    );
    view.handle_key_event(KeyCode::Esc.into());
    assert_eq!(screen(&view, area.width, area.height), idle);
}

#[tokio::test]
async fn backspace_edits_search_and_rename_without_deleting_tasks() {
    let mut app = make_test_app().await;
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    app.app_event_tx = AppEventSender::new(tx);
    let mut view = app.agents_overview_view(
        vec![overview_thread(
            ThreadId::new(),
            /*parent_thread_id*/ None,
            "Task",
            ThreadStatus::Idle,
        )],
        /*selected_thread_id*/ None,
    );
    for key in ['f', 'r'] {
        view.handle_key_event(KeyCode::Char(key).into());
        let initial = screen(&view, /*width*/ 88, /*height*/ 16);
        view.handle_paste("!".into());
        view.handle_key_event(KeyCode::Backspace.into());
        assert_eq!(screen(&view, /*width*/ 88, /*height*/ 16), initial);
        assert!(rx.try_recv().is_err());
        view.handle_key_event(KeyCode::Esc.into());
    }
}
