//! Dashboard rows, paging, metadata clipping and status-filter regressions.

use super::*;
use crate::app::owned_transcript::tests::buffer_text;
use assert_matches::assert_matches;
use pretty_assertions::assert_eq;

fn painted_row(text: &str, label: &str) -> u16 {
    text.lines()
        .position(|line| line.contains(label))
        .unwrap_or_else(|| panic!("missing {label:?} in rendered screen:\n{text}")) as u16
}

pub(super) fn screen(view: &AgentsOverviewView, width: u16, height: u16) -> String {
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
async fn live_center_keeps_status_order_across_progress_refreshes() {
    let mut app = make_test_app_with_chat().await;
    app.agents_overview.view_state.lock().unwrap().grouping = AgentsOverviewGrouping::Status;
    let ids = [1, 2, 3, 4, 5].map(ThreadId::from_u128);
    let mut threads = [
        (
            "Alpha",
            ThreadStatus::Active {
                active_flags: vec![],
            },
            10,
        ),
        (
            "Bravo",
            ThreadStatus::Active {
                active_flags: vec![],
            },
            20,
        ),
        (
            "Charlie",
            ThreadStatus::Active {
                active_flags: vec![],
            },
            20,
        ),
        ("Ready task", ThreadStatus::Idle, 40),
        ("Needs input", ThreadStatus::SystemError, 0),
    ]
    .into_iter()
    .zip(ids)
    .map(|((name, status, updated_at), id)| {
        let mut thread = overview_thread(id, /*parent_thread_id*/ None, name, status);
        // Future timestamps render as "now", keeping the visual fixture clock-independent.
        thread.updated_at = 4_000_000_000 + updated_at;
        thread
    })
    .collect::<Vec<_>>();
    threads.reverse();
    let view = app.agents_overview_view(threads.clone(), Some(ids[0]));
    let expected = vec![ids[4], ids[1], ids[2], ids[0], ids[3]];
    assert_eq!(view.thread_ids(), expected);
    let before = screen(&view, /*width*/ 100, /*height*/ 18);
    app.agents_overview.visible_thread_ids = view.thread_ids();
    app.chat_widget.show_bottom_pane_view(Box::new(view));

    for (step, id) in [ids[0], ids[2], ids[1], ids[0]].into_iter().enumerate() {
        let thread = threads
            .iter_mut()
            .find(|thread| thread.id == id.to_string())
            .unwrap();
        thread.updated_at = 4_000_000_100 + step as i64;
        thread.preview = format!("Progress update {step}");
        threads.rotate_left(/*mid*/ 1);
        app.agents_overview.threads = threads
            .iter()
            .cloned()
            .map(|thread| (ThreadId::from_string(&thread.id).unwrap(), Some(thread)))
            .collect();
        app.repaint_agents_overview();
        assert_eq!(app.agents_overview.visible_thread_ids[1], id);
        let selected = app
            .chat_widget
            .selected_index_for_present_view(AGENTS_OVERVIEW_VIEW_ID)
            .unwrap();
        assert_eq!(app.agents_overview.visible_thread_ids[selected], ids[0]);
    }
    app.agents_overview.pins_by_backend = [(
        crate::backend_id::BackendId::new(),
        Some(vec![ids[3], ids[2]]),
    )]
    .into();
    for parent in [ids[0], ids[3]] {
        threads.push(overview_thread(
            ThreadId::new(),
            Some(parent),
            "Failed subagent",
            ThreadStatus::SystemError,
        ));
    }
    let view = app.agents_overview_view(threads, Some(ids[0]));
    assert!(
        view.rows[view.selected_index().unwrap()]
            .details
            .lines
            .is_empty()
    );
    let after = screen(&view, /*width*/ 100, /*height*/ 18);
    insta::assert_snapshot!(format!(
        "Initial display:\n{before}\n\nAfter subagents fail under Alpha and Ready task:\n{after}"
    ));
}

#[tokio::test]
async fn failed_background_worker_only_needs_attention_for_a_pending_request() {
    let app = make_test_app().await;
    let worker_id = ThreadId::new();
    let mut worker = overview_thread(
        worker_id,
        /*parent_thread_id*/ None,
        "Background review",
        ThreadStatus::SystemError,
    );
    worker.source = SessionSource::SubAgent(SubAgentSource::Review);
    let view = app.agents_overview_view(vec![worker.clone()], Some(worker_id));
    assert_eq!(view.rows[0].group, AgentsOverviewGroup::Finished);
    assert!(view.rows[0].details.lines.is_empty());

    let child = overview_thread(
        ThreadId::new(),
        Some(worker_id),
        "Check dependencies",
        ThreadStatus::Active {
            active_flags: vec![ThreadActiveFlag::WaitingOnApproval],
        },
    );
    let view = app.agents_overview_view(vec![worker, child], Some(worker_id));
    assert_eq!(view.rows[0].group, AgentsOverviewGroup::NeedsYou);
    let rendered = screen(&view, /*width*/ 100, /*height*/ 32);
    assert!(rendered.contains("● Needs input"));
    assert!(rendered.contains("Waiting for approval."));
}

#[tokio::test]
async fn live_center_sorts_arrivals_and_status_transitions_by_update_time() {
    let (app, mut events, _ops) = make_test_app_with_channels().await;
    app.agents_overview.view_state.lock().unwrap().grouping = AgentsOverviewGrouping::Status;
    let [alpha, bravo, charlie, delta, child] = [1, 2, 3, 4, 5].map(ThreadId::from_u128);
    let working = ThreadStatus::Active {
        active_flags: vec![],
    };
    let mut threads = vec![
        overview_thread(
            alpha,
            /*parent_thread_id*/ None,
            "Alpha",
            working.clone(),
        ),
        overview_thread(
            bravo,
            /*parent_thread_id*/ None,
            "Bravo",
            working.clone(),
        ),
        overview_thread(
            charlie,
            /*parent_thread_id*/ None,
            "Charlie",
            ThreadStatus::Idle,
        ),
    ];
    assert_eq!(
        app.agents_overview_view(threads.clone(), Some(bravo))
            .thread_ids(),
        vec![alpha, bravo, charlie]
    );
    let mut arrival = overview_thread(delta, /*parent_thread_id*/ None, "Delta", working);
    arrival.updated_at += 100;
    threads.push(arrival);
    let mut child = overview_thread(
        child,
        /*parent_thread_id*/ None,
        "Child",
        ThreadStatus::SystemError,
    );
    child.parent_thread_id = Some(bravo.to_string());
    threads.push(child);
    assert_eq!(
        app.agents_overview_view(threads.clone(), Some(bravo))
            .thread_ids(),
        vec![delta, alpha, bravo, charlie]
    );
    threads[1].status = ThreadStatus::Idle;
    let mut view = app.agents_overview_view(threads.clone(), Some(bravo));
    view.dock_focused = Some(true);
    assert_eq!(view.thread_ids(), vec![delta, alpha, bravo, charlie]);
    let selected = &view.rows[view.selected_index().unwrap()];
    assert_eq!(selected.group, AgentsOverviewGroup::Ready);
    assert!(selected.details.lines.is_empty());
    // Capture the actual compact rows before the refresh moves Alpha to a different index.
    let area = Rect::new(
        /*x*/ 17, /*y*/ 8, /*width*/ 48, /*height*/ 28,
    );
    let mut buf = ratatui::buffer::Buffer::empty(area);
    view.render(area, &mut buf);
    let text = buffer_text(&buf);
    let alpha_y = painted_row(&text, "Alpha");
    assert_eq!(
        text.lines().nth(usize::from(alpha_y) + 1).unwrap().trim(),
        "Working"
    );
    let point = |y| (area.right() - 3, area.y + y).into();

    // A request from a descendant can still bring its idle parent to the top.
    threads.last_mut().unwrap().status = ThreadStatus::Active {
        active_flags: vec![ThreadActiveFlag::WaitingOnApproval],
    };
    let mut view = app.agents_overview_view(threads.clone(), Some(bravo));
    assert_eq!(view.thread_ids(), vec![bravo, delta, alpha, charlie]);
    // A heading and a blank row in the list don't select or activate a task.
    let heading = painted_row(&text, "Working  ");
    let blank = area.height - 3;
    assert!(
        text.lines()
            .nth(usize::from(blank))
            .unwrap()
            .trim()
            .is_empty()
    );
    for y in [heading, blank] {
        view.click_row(point(y));
    }
    assert_eq!(view.rows[view.selected_index().unwrap()].thread_id, bravo);
    assert!(events.try_recv().is_err());

    view.handle_key_event(KeyCode::Char('?').into());
    view.click_row(point(alpha_y));
    assert_eq!(view.rows[view.selected_index().unwrap()].thread_id, bravo);
    assert!(events.try_recv().is_err());
    view.handle_key_event(KeyCode::Esc.into());
    for y in [alpha_y, alpha_y + 1] {
        view.handle_key_event(KeyCode::Char('/').into());
        view.handle_paste("Alpha".into());
        assert_matches!(events.try_recv(), Ok(AppEvent::SearchAgentsOverview(..)));
        view.click_row(point(y));
        assert_matches!(
            events.try_recv(),
            Ok(AppEvent::SelectAgentsOverviewThread { thread_id }) if thread_id == alpha
        );
    }
    view.handle_key_event(KeyCode::Tab.into()); // Needs you hides Alpha before the next paint.
    view.click_row(point(alpha_y));
    assert_eq!(view.rows[view.selected_index().unwrap()].thread_id, bravo);
    assert!(events.try_recv().is_err());
    view.handle_key_event(KeyCode::BackTab.into());
    threads.pop();
    let view = app.agents_overview_view(threads.clone(), Some(bravo));
    assert_eq!(view.thread_ids(), vec![delta, alpha, bravo, charlie]);
    assert_eq!(view.rows[view.selected_index().unwrap()].thread_id, bravo);
    // Rediscovery restores the task to its position by update time.
    let removed = threads.remove(/*index*/ 0);
    let mut view = app.agents_overview_view(threads.clone(), Some(bravo));
    assert_eq!(view.thread_ids(), vec![delta, bravo, charlie]);
    view.click_row(point(alpha_y));
    assert_eq!(view.rows[view.selected_index().unwrap()].thread_id, bravo);
    assert!(events.try_recv().is_err());
    threads.push(removed);
    assert_eq!(
        app.agents_overview_view(threads, Some(bravo)).thread_ids(),
        vec![delta, alpha, bravo, charlie]
    );
}

#[tokio::test]
async fn live_center_status_order_tracks_updates_while_filtered_and_pinned() {
    let mut app = make_test_app().await;
    app.agents_overview.view_state.lock().unwrap().grouping = AgentsOverviewGrouping::Status;
    let ids = [1, 2, 3].map(ThreadId::from_u128);
    let mut threads = ["Alpha", "Bravo", "Charlie"]
        .into_iter()
        .zip(ids)
        .map(|(name, id)| {
            overview_thread(
                id,
                /*parent_thread_id*/ None,
                name,
                ThreadStatus::Active {
                    active_flags: vec![],
                },
            )
        })
        .collect::<Vec<_>>();
    let rendered_titles = |view: &AgentsOverviewView| {
        screen(view, /*width*/ 100, /*height*/ 18)
            .lines()
            .filter_map(|line| {
                let (_, task_column) = line.split('│').next()?.split_once("● ")?;
                let task_column = task_column.trim_start_matches('📌').trim_start();
                ["Alpha", "Bravo", "Charlie"]
                    .into_iter()
                    .find(|title| task_column.starts_with(title))
            })
            .collect::<Vec<_>>()
    };
    let mut view = app.agents_overview_view(threads.clone(), Some(ids[1]));
    // Hidden rows also pick up timestamp changes when the filter is cleared.
    view.handle_key_event(KeyCode::Char('/').into());
    view.handle_paste("Bravo".into());
    threads[2].updated_at += 100;
    view = app.agents_overview_view(threads.clone(), Some(ids[1]));
    assert_eq!(rendered_titles(&view), vec!["Bravo"]);
    view.handle_key_event(KeyCode::Esc.into());
    assert_eq!(rendered_titles(&view), vec!["Charlie", "Alpha", "Bravo"]);
    // Update another working task while the empty Needs input tab is selected.
    view.handle_key_event(KeyCode::Tab.into());
    assert!(rendered_titles(&view).is_empty());
    threads[1].updated_at += 200;
    view = app.agents_overview_view(threads.clone(), /*selected_thread_id*/ None);
    view.handle_key_event(KeyCode::Tab.into());
    assert_eq!(rendered_titles(&view), vec!["Bravo", "Charlie", "Alpha"]);
    view.handle_key_event(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT));
    view.handle_key_event(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT));
    app.agents_overview.pins_by_backend = [(
        crate::backend_id::BackendId::new(),
        Some(vec![ids[0], ids[2]]),
    )]
    .into();
    view = app.agents_overview_view(threads.clone(), Some(ids[1]));
    assert_eq!(rendered_titles(&view), vec!["Alpha", "Charlie", "Bravo"]);
    app.agents_overview.pins_by_backend =
        [(crate::backend_id::BackendId::new(), Some(vec![]))].into();
    view = app.agents_overview_view(threads, Some(ids[1]));
    assert_eq!(rendered_titles(&view), vec!["Bravo", "Charlie", "Alpha"]);
}

#[tokio::test]
async fn live_center_columns() {
    let mut app = make_test_app().await;
    let current = ThreadId::from_u128(/*value*/ 42);
    app.primary_thread_id = Some(current);
    let cloud = crate::backend_id::BackendId::new();
    app.backend_registry = crate::app::backend_registry::BackendRegistry::new([cloud].into());
    assert!(app.backend_registry.observe(cloud, current));
    assert!(
        app.backend_registry
            .observe(cloud, ThreadId::from_u128(/*value*/ 43))
    );
    assert!(
        app.backend_registry
            .observe(cloud, ThreadId::from_u128(/*value*/ 47))
    );
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
    let mut threads = fixtures
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
    let child = ThreadId::from_u128(/*value*/ 48);
    threads.push(overview_thread(
        child,
        Some(ThreadId::from_u128(/*value*/ 43)),
        "Child",
        ThreadStatus::Idle,
    ));
    crate::chatwidget::activate_voice_for_thread(&mut app.chat_widget, child);
    app.agents_overview.pins_by_backend = [(
        crate::backend_id::BackendId::new(),
        Some(vec![current, ThreadId::from_u128(/*value*/ 44)]),
    )]
    .into();
    app.agents_overview
        .unread
        .insert(ThreadId::from_u128(/*value*/ 47));
    let mut view = app.agents_overview_view(threads, Some(current));
    let narrow = screen(&view, /*width*/ 50, /*height*/ 24);
    assert!(narrow.contains("● Error: ☁️  Failed task"));
    assert!(
        narrow
            .lines()
            .any(|line| { line.contains("☁️  A longer task title") && line.contains('…') })
    );
    insta::assert_snapshot!(screen(&view, /*width*/ 160, /*height*/ 22));
    view.pinned_thread_ranks = None;
    let mut selected_status_styles = Vec::new();
    for _ in 0..fixtures.len() {
        let area = Rect::new(
            /*x*/ 0, /*y*/ 0, /*width*/ 160, /*height*/ 22,
        );
        let mut buf = ratatui::buffer::Buffer::empty(area);
        view.render(area, &mut buf);
        let selected_row = buf
            .content
            .chunks(usize::from(area.width))
            .find(|row| row.iter().any(|cell| cell.symbol() == "›"))
            .unwrap();
        let marker = selected_row
            .iter()
            .position(|cell| cell.symbol() == "›")
            .unwrap();
        selected_status_styles.push(format!(
            "{} | {:?}",
            selected_row[marker + 2].symbol(),
            selected_row[marker + 2].style()
        ));
        view.handle_key_event(KeyCode::Down.into());
    }
    insta::assert_snapshot!(
        "live_center_selected_status_styles",
        selected_status_styles.join("\n")
    );
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
                    thread.cwd = test_path_buf(&format!("/tmp/second-project-{index:02}"))
                        .abs()
                        .into();
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
            view.handle_key_event(KeyCode::Char('/').into());
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
        // Page through a dense group followed by one-task groups in both layouts.
        let mut pages = screen(&view, /*width*/ 110, /*height*/ 18);
        for _ in 0..count {
            view.handle_key_event(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::CONTROL));
            pages.push_str(&screen(&view, /*width*/ 110, /*height*/ 18));
        }
        for index in 0..count {
            assert!(
                pages.contains(&format!("Paging task {index:02}")),
                "skipped task {index} paging down"
            );
        }
        let last = view.rows[view.selected_index().unwrap()].thread_id;
        view.handle_key_event(KeyCode::PageDown.into());
        assert_eq!(
            (last, view.rows[view.selected_index().unwrap()].thread_id),
            (ThreadId::from_u128(100 + count - 1), last)
        );
        let mut pages = screen(&view, /*width*/ 48, /*height*/ 18);
        for _ in 0..count {
            view.handle_key_event(KeyCode::PageUp.into());
            pages.push_str(&screen(&view, /*width*/ 48, /*height*/ 18));
        }
        assert_eq!(view.rows[view.selected_index().unwrap()].thread_id, first);
        for index in 0..count {
            assert!(
                pages.contains(&format!("Paging task {index:02}")),
                "skipped task {index} paging up"
            );
        }
    }
}

#[tokio::test]
async fn live_center_pages_full_task_rows_and_restores_the_first_heading() {
    let app = make_test_app().await;
    let ids = (100..130).map(ThreadId::from_u128).collect::<Vec<_>>();
    let threads = ids
        .iter()
        .enumerate()
        .map(|(index, &id)| {
            let mut thread = overview_thread(
                id,
                /*parent_thread_id*/ None,
                &format!("Paging task {}", index + 1),
                ThreadStatus::Idle,
            );
            thread.updated_at -= index as i64;
            thread
        })
        .collect::<Vec<_>>();
    let selected = |view: &AgentsOverviewView| view.rows[view.selected_index().unwrap()].thread_id;
    let mut view = app.agents_overview_view(threads.clone(), Some(ids[9]));
    let first_page = screen(&view, /*width*/ 80, /*height*/ 16);
    assert!(first_page.contains("Paging task 1 "));
    assert!(first_page.contains("Paging task 10 "));
    for expected in [ids[19], ids[29]] {
        view.handle_key_event(KeyCode::PageDown.into());
        assert_eq!(selected(&view), expected);
    }

    let mut view = app.agents_overview_view(threads, Some(ids[0]));
    let initial = screen(&view, /*width*/ 80, /*height*/ 16);
    view.handle_key_event(KeyCode::PageDown.into());
    assert!(screen(&view, /*width*/ 80, /*height*/ 16).contains('↑'));
    view.handle_key_event(KeyCode::PageUp.into());
    assert_eq!(selected(&view), ids[0]);
    assert_eq!(screen(&view, /*width*/ 80, /*height*/ 16), initial);
}

#[tokio::test]
async fn live_center_navigates_sections() {
    let mut app = make_test_app().await;
    let config: TuiKeymap =
        toml::from_str("[agents]\nnext_group = ['shift-down', 'shift-j', 'f8']").unwrap();
    app.keymap = RuntimeKeymap::from_config(&config).unwrap();
    let threads = (0..6)
        .map(|index| {
            let mut thread = overview_thread(
                ThreadId::from_u128(100 + index),
                /*parent_thread_id*/ None,
                &format!("Section task {index}"),
                ThreadStatus::Idle,
            );
            thread.updated_at -= index as i64;
            thread.cwd = test_path_buf(&format!("/tmp/project-{}", index / 2))
                .abs()
                .into();
            thread
        })
        .collect();
    let mut view = app.agents_overview_view(threads, Some(ThreadId::from_u128(/*value*/ 101)));
    let selected = |view: &AgentsOverviewView| view.rows[view.selected_index().unwrap()].thread_id;
    for (key, expected) in [
        (KeyEvent::new(KeyCode::Down, KeyModifiers::SHIFT), 102),
        (KeyCode::Char('J').into(), 104),
        (KeyEvent::new(KeyCode::Char('j'), KeyModifiers::SHIFT), 104),
        (KeyEvent::new(KeyCode::Up, KeyModifiers::SHIFT), 102),
        (KeyCode::Char('K').into(), 100),
        (KeyEvent::new(KeyCode::Char('k'), KeyModifiers::SHIFT), 100),
    ] {
        view.handle_key_event(key);
        assert_eq!(selected(&view), ThreadId::from_u128(expected));
    }
    view.handle_key_event(KeyCode::Char('/').into());
    view.handle_key_event(KeyCode::Char('J').into());
    assert!(screen(&view, /*width*/ 80, /*height*/ 18).contains("Search › J"));
    view.handle_key_event(KeyCode::Esc.into());
    view.handle_key_event(KeyCode::Char('r').into());
    view.handle_key_event(KeyCode::F(8).into());
    assert_eq!(selected(&view), ThreadId::from_u128(/*value*/ 100));
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
    for (key, label) in [('r', "Rename › "), ('/', "Search › ")] {
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
            let cursor = view
                .cursor_pos(Rect::new(
                    /*x*/ 0, /*y*/ 0, /*width*/ 24, /*height*/ 18,
                ))
                .unwrap();
            assert!(cursor.0 < 24);
            insta::assert_snapshot!("live_center_narrow_rename", rendered);
        }
        view.handle_key_event(KeyCode::Esc.into());
    }
}

#[tokio::test]
async fn live_center_rename_retains_target_when_status_leaves_filter() -> Result<()> {
    let mut app = make_test_app().await;
    app.chat_widget.toggle_vim_mode_and_notify();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    app.app_event_tx = AppEventSender::new(tx);
    let id = ThreadId::new();
    let other = ThreadId::new();
    let mut threads = vec![
        overview_thread(
            id,
            /*parent_thread_id*/ None,
            "Tar\nget",
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
        assert_eq!(
            view.cursor_style(Rect::default()),
            crossterm::cursor::SetCursorStyle::SteadyBar
        );
        threads[0].status = ThreadStatus::SystemError;
        view = app.agents_overview_view(threads.clone(), Some(id));
        if key == KeyCode::Enter {
            for edit in [
                KeyCode::Esc,
                KeyCode::Char('0'),
                KeyCode::Char('c'),
                KeyCode::Char('w'),
            ] {
                view.handle_key_event(edit.into());
            }
            view.handle_paste("Target".into());
        }
        view.handle_key_event(KeyCode::Char('!').into());
        view.handle_key_event(key.into());
        if key == KeyCode::Esc {
            assert_eq!(
                view.cursor_style(Rect::default()),
                crossterm::cursor::SetCursorStyle::DefaultUserShape
            );
            view.handle_key_event(KeyCode::Esc.into());
        }
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
async fn escape_returns_to_a_chat_that_is_still_starting() {
    let mut app = make_test_app().await;
    app.pending_startup_thread_start = true;
    let mut view = app.agents_overview_view(Vec::new(), /*selected_thread_id*/ None);
    view.handle_key_event(KeyCode::Esc.into());
    assert!(view.is_complete());
}

#[tokio::test]
async fn live_center_navigation_and_complete_hints() {
    let app = make_test_app_with_chat().await;
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
    for editor_key in ['/', 'r'] {
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
            view.handle_key_event(KeyCode::Char('/').into());
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
    let mut app = make_test_app_with_chat().await;
    let config: TuiKeymap = toml::from_str(
        r#"
[list]
move_up = 'k'
move_down = 'j'
page_up = []
[agents]
archive = []
rename = 'z r'
toggle_pin = 'z p'
"#,
    )
    .unwrap();
    app.keymap = RuntimeKeymap::from_config(&config).unwrap();
    app.agents_overview.pins_by_backend =
        [(crate::backend_id::BackendId::new(), Some(Vec::new()))].into();
    let view = app.agents_overview_view(Vec::new(), /*selected_thread_id*/ None);
    insta::assert_snapshot!(
        "live_center_custom_footer",
        screen(&view, /*width*/ 100, /*height*/ 12)
    );
    let thread_id = ThreadId::from_u128(/*value*/ 42);
    let mut view = app.agents_overview_view(
        vec![overview_thread(
            thread_id,
            /*parent_thread_id*/ None,
            "Task",
            ThreadStatus::Idle,
        )],
        Some(thread_id),
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
    let mut thread = overview_thread(
        ThreadId::from_u128(/*value*/ 0xabcdef),
        /*parent_thread_id*/ None,
        "Task to find",
        ThreadStatus::Idle,
    );
    thread.git_info = Some(codex_app_server_protocol::GitInfo {
        sha: None,
        branch: Some("Fix/Resume".to_string()),
        origin_url: None,
    });
    let mut view = app.agents_overview_view(vec![thread], /*selected_thread_id*/ None);
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
    for key in [KeyCode::Char('/'), KeyCode::F(3)] {
        view.handle_key_event(key.into());
        view.handle_paste("find".into());
        insta::assert_snapshot!(
            "live_center_search_active",
            screen(&view, area.width, area.height)
        );
        view.handle_key_event(KeyCode::Esc.into());
        assert_eq!(screen(&view, area.width, area.height), idle);
    }

    for query in ["FIX/RES", "ABCDEF", "find Task"] {
        view.handle_key_event(KeyCode::Char('/').into());
        view.handle_paste(query.into());
        assert!(screen(&view, area.width, area.height).contains("Task to find"));
        view.handle_key_event(KeyCode::Esc.into());
    }
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
    for key in ['/', 'r'] {
        view.handle_key_event(KeyCode::Char(key).into());
        let initial = screen(&view, /*width*/ 88, /*height*/ 16);
        view.handle_paste("!".into());
        view.handle_key_event(KeyCode::Backspace.into());
        assert_eq!(screen(&view, /*width*/ 88, /*height*/ 16), initial);
        if key == '/' {
            assert_matches!(rx.try_recv(), Ok(AppEvent::SearchAgentsOverview(..)));
        }
        assert!(rx.try_recv().is_err());
        view.handle_key_event(KeyCode::Esc.into());
    }
}

#[tokio::test]
async fn overview_clears_voice_badge_after_async_close() -> Result<()> {
    let (mut app, mut events, _) = crate::app::tests::make_test_app_with_channels().await;
    let owner = ThreadId::new();
    crate::chatwidget::activate_voice_for_thread(&mut app.chat_widget, owner);
    app.chat_widget.park_voice();
    let (visible, _, _, _) = crate::chatwidget::tests::make_chatwidget_manual_with_sender().await;
    app.background_voice = Some(Box::new(std::mem::replace(&mut app.chat_widget, visible)));
    app.primary_thread_id = Some(owner);
    let thread = overview_thread(
        owner,
        /*parent_thread_id*/ None,
        "Voice owner",
        ThreadStatus::Idle,
    );
    app.agents_overview
        .threads
        .insert(owner, Some(thread.clone()));
    let view = app.agents_overview_view(vec![thread], Some(owner));
    app.agents_overview.visible_thread_ids = view.thread_ids();
    app.chat_widget.show_bottom_pane_view(Box::new(view));
    assert!(render_bottom_popup(&app.chat_widget, /*width*/ 100).contains("  voice"));
    let mut server = crate::start_embedded_app_server_for_picker(&app.config).await?;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    app.deliver_background_voice_notification(
        owner,
        &ServerNotification::ThreadRealtimeClosed(
            codex_app_server_protocol::ThreadRealtimeClosedNotification {
                thread_id: owner.to_string(),
                reason: Some("requested".into()),
            },
        ),
    );
    while let Ok(event) = events.try_recv() {
        Box::pin(app.handle_event(&mut tui, &mut server, event)).await?;
    }
    assert!(!render_bottom_popup(&app.chat_widget, /*width*/ 100).contains("  voice"));
    server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn live_center_show_more_navigation_search_and_loading() {
    let mut app = make_test_app_with_chat().await;
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    app.app_event_tx = AppEventSender::new(tx);
    app.agents_overview.view_state.lock().unwrap().has_more = true;
    let thread = overview_thread(
        ThreadId::new(),
        /*parent_thread_id*/ None,
        "Recent task",
        ThreadStatus::NotLoaded,
    );
    let mut view = app.agents_overview_view(vec![thread], /*selected_thread_id*/ None);
    view.handle_key_event(KeyCode::Down.into());
    insta::assert_snapshot!(
        "live_center_show_more",
        screen(&view, /*width*/ 80, /*height*/ 12)
    );
    view.handle_key_event(KeyCode::Enter.into());
    view.handle_key_event(KeyCode::Enter.into());
    assert_matches!(rx.try_recv(), Ok(AppEvent::ShowMoreAgentsOverview));
    assert!(rx.try_recv().is_err());
    assert!(screen(&view, /*width*/ 80, /*height*/ 12).contains("Loading more"));
    {
        let mut state = app.agents_overview.view_state.lock().unwrap();
        state.loading = false;
        state.has_more = false;
        state.refresh_failed = true;
    }
    view.handle_key_event(KeyCode::Char('/').into());
    view.handle_paste("Older task".into());
    let id = assert_matches!(rx.try_recv(), Ok(AppEvent::SearchAgentsOverview(id, _)) => id);
    insta::assert_snapshot!(screen(&view, /*width*/ 80, /*height*/ 12).lines().nth(6).unwrap(), @"  ›   Searching history…");
    let backend_id = crate::backend_id::BackendId::new();
    app.apply_agents_overview_search_page(
        id,
        backend_id,
        Vec::new(),
        Default::default(),
        /*ok*/ false,
    );
    insta::assert_snapshot!(
        "live_center_show_more_search_retry",
        screen(&view, /*width*/ 80, /*height*/ 12)
    );
    let rendered = screen(&view, /*width*/ 80, /*height*/ 12);
    let retry = painted_row(&rendered, "Show more (retry)");
    view.click_row(ratatui::layout::Position::new(/*x*/ 6, retry));
    let id = assert_matches!(rx.try_recv(), Ok(AppEvent::SearchAgentsOverview(id, _)) => id);
    app.apply_agents_overview_search_page(
        id,
        backend_id,
        Vec::new(),
        Default::default(),
        /*ok*/ true,
    );
    insta::assert_snapshot!(screen(&view, /*width*/ 80, /*height*/ 12).lines().nth(6).unwrap(), @"  ›   Show more matches");
}

#[tokio::test]
async fn workspace_navigation_preserves_the_draft_and_reading_position() -> Result<()> {
    use crate::app::owned_transcript::tests::attach_thread;
    use crate::app::owned_transcript::tests::buffer_text;

    let (mut app, mut events, _ops) = make_test_app_with_channels().await;
    let mut server = Box::pin(crate::start_embedded_app_server_for_picker(&app.config)).await?;
    app.local_settings.tui.animations = false;
    let wide = Size::new(/*width*/ 90, /*height*/ 14);
    let mut tui = crate::tui::test_support::make_test_tui_with_size(wide)?;
    tui.set_owned_screen(/*owned*/ true)?;
    app.config.features.enable(Feature::TuiWorkspace)?;
    app.keymap =
        RuntimeKeymap::for_session(&app.local_settings.tui.keymap, /*workspace*/ true).unwrap();
    app.local_settings.transcript_mode = crate::transcript_mode::TranscriptMode::Owned;
    let id = ThreadId::from_u128(/*value*/ 12);
    attach_thread(&mut app, id);
    let mut title = "Investigate retry";
    let mut task = overview_thread(
        id,
        /*parent_thread_id*/ None,
        title,
        ThreadStatus::Idle,
    );
    task.git_info = Some(codex_app_server_protocol::GitInfo {
        branch: Some("retry-fix".into()),
        sha: None,
        origin_url: None,
    });
    task.updated_at = 4_000_000_000;
    app.agents_overview.initialized.insert(server.backend_id());
    app.agents_overview.threads.insert(id, Some(task));
    app.agents_overview
        .pins_by_backend
        .insert(server.backend_id(), Some(vec![id]));
    let rows = (0..40)
        .map(|i| format!("Response row {i}").into())
        .collect();
    app.transcript_cells
        .push(Arc::new(history_cell::PlainHistoryCell::new(rows)));
    let paste = TuiEvent::Paste("Keep this draft".into());
    crate::boxed_future!(app.handle_tui_event(&mut tui, &mut server, paste)).await?;

    app.render_chat_widget_frame(&mut tui, wide)?;
    app.transcript_view
        .scroll(&app.transcript_cells, /*rows*/ -10);
    app.render_chat_widget_frame(&mut tui, wide)?;
    let before = buffer_text(tui.terminal.previous_buffer());
    let first = before.lines().next().unwrap().trim();
    assert!(first.starts_with("Response row"));
    assert_eq!(
        app.chat_widget.composer_text_with_pending(),
        "Keep this draft"
    );

    let narrow = Size::new(50, 14);
    let mut screens = Vec::new();
    let mut wide_styles = Vec::new();
    for (label, size, center_focused) in [
        ("Wide Command Center", wide, true),
        ("Wide conversation", wide, false),
        ("Narrow conversation", narrow, false),
        ("Narrow Command Center", narrow, true),
        ("Restored dock", wide, true),
    ] {
        tui.terminal.resize(size)?;
        if center_focused && !app.workspace_center_owns_input() {
            crate::boxed_future!(app.handle_event(
                &mut tui,
                &mut server,
                AppEvent::OpenAgentsOverview
            ))
            .await?;
        } else if !center_focused && app.workspace_center_owns_input() {
            let click = crossterm::event::MouseEvent {
                kind: crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
                column: wide.width - 1,
                row: 0,
                modifiers: KeyModifiers::NONE,
            };
            crate::boxed_future!(app.handle_tui_event(
                &mut tui,
                &mut server,
                TuiEvent::Mouse(click)
            ))
            .await?;
        }
        if size == wide && !center_focused {
            title = "Investigate retry again";
            let renamed = ServerNotification::ThreadNameUpdated(ThreadNameUpdatedNotification {
                thread_id: id.to_string(),
                thread_name: Some(title.into()),
            });
            app.handle_app_server_event(
                &server,
                AppServerEvent::ServerNotification(Box::new(renamed)),
            )
            .await;
        }
        tui.screen_size_for_event(&TuiEvent::Resize(size))?;
        crate::terminal_palette::with_test_default_colors(
            crate::terminal_probe::DefaultColors {
                fg: (230, 230, 230),
                bg: (20, 20, 20),
            },
            || app.render_chat_widget_frame(&mut tui, size),
        )?;
        let buffer = tui.terminal.previous_buffer();
        let screen = buffer_text(buffer);
        assert_eq!(app.workspace_center_owns_input(), center_focused, "{label}");
        if size == wide && wide_styles.len() < 2 {
            wide_styles.push([title, "All 1", "Keep this draft"].map(|text| {
                let (row, prefix) = screen
                    .lines()
                    .enumerate()
                    .find_map(|(row, line)| Some((row, line.split_once(text)?.0)))
                    .expect("visible task, filter and draft");
                buffer[(prefix.chars().count() as u16, row as u16)].style()
            }));
        }
        if matches!(label, "Wide Command Center" | "Narrow Command Center") {
            let screen = normalize_agent_center_snapshot(screen);
            screens.push(format!("{label}\n{screen}"));
        }
    }
    let [center, conversation] = wide_styles.as_slice() else {
        panic!("expected both wide focus states");
    };
    for (center_style, conversation_style) in center.iter().zip(conversation) {
        assert_ne!(center_style, conversation_style);
    }
    assert_ne!(center[2].fg, conversation[2].fg);
    assert_ne!(center[2].bg, conversation[2].bg);

    crate::boxed_future!(app.handle_tui_event(
        &mut tui,
        &mut server,
        TuiEvent::Key(KeyCode::Tab.into())
    ))
    .await?;
    app.render_chat_widget_frame(&mut tui, wide)?;
    assert!(buffer_text(tui.terminal.previous_buffer()).contains("No matching tasks"));
    insta::assert_snapshot!("workspace_responsive_focus", screens.join("\n\n"));
    crate::boxed_future!(app.handle_tui_event(
        &mut tui,
        &mut server,
        TuiEvent::Paste("ignored".into())
    ))
    .await?;
    for key in [KeyCode::BackTab, KeyCode::Enter] {
        crate::boxed_future!(app.handle_tui_event(
            &mut tui,
            &mut server,
            TuiEvent::Key(key.into())
        ))
        .await?;
    }
    let selected = std::iter::from_fn(|| events.try_recv().ok())
        .find(|event| {
            matches!(event, AppEvent::SelectAgentsOverviewThread { thread_id } if *thread_id == id)
        })
        .expect("Enter selects the displayed task");
    crate::boxed_future!(app.handle_event(&mut tui, &mut server, selected)).await?;
    let bottom = app.render_chat_widget_frame(&mut tui, wide)?;
    let screen = buffer_text(tui.terminal.previous_buffer());
    assert!(screen.contains(title));
    assert!(screen.lines().next().unwrap().ends_with(first));
    assert!(bottom.contains(tui.terminal.last_known_cursor_pos));
    assert_eq!(
        app.chat_widget.composer_text_with_pending(),
        "Keep this draft"
    );

    crate::boxed_future!(app.handle_event(&mut tui, &mut server, AppEvent::OpenAgentsOverview))
        .await?;
    app.sidebar_mut().show(
        crate::app::sidebar::SidebarContent::Chooser,
        crate::app::sidebar::PaneFocus::Sidebar,
    );
    let three_panes = Size::new(185, 18);
    let combined = crate::app::test_support::side_frame(&mut app, &mut tui, three_panes)?;
    let center = app.conversation_frame.command_center.unwrap();
    let side = app.conversation_frame.panel_area().unwrap();
    let middle = app.conversation_area(Rect::new(0, 0, 185, 18));
    assert!(center.right() <= middle.x && middle.right() <= side.x && !middle.is_empty());
    let labels = ["Agent command center", "Keep this draft", "Open in sidebar"].map(|label| {
        combined
            .lines()
            .find_map(|line| line.find(label))
            .map(|_| label)
    });
    insta::assert_snapshot!(format!("{labels:?}"),@"[Some(\"Agent command center\"), Some(\"Keep this draft\"), Some(\"Open in sidebar\")]");
    let divider = app
        .conversation_frame
        .divider_area()
        .expect("right divider");
    let mouse = |kind, column| {
        TuiEvent::Mouse(crossterm::event::MouseEvent {
            kind,
            column,
            row: divider.y,
            modifiers: KeyModifiers::NONE,
        })
    };
    use crossterm::event::MouseButton::Left;
    use crossterm::event::MouseEventKind::Down;
    use crossterm::event::MouseEventKind::Drag;
    tui.screen_size_for_event(&TuiEvent::Resize(three_panes))?;
    crate::boxed_future!(app.handle_tui_event(&mut tui, &mut server, mouse(Down(Left), divider.x)))
        .await?;
    crate::boxed_future!(app.handle_tui_event(
        &mut tui,
        &mut server,
        mouse(Drag(Left), divider.x - 6)
    ))
    .await?;
    crate::app::test_support::side_frame(&mut app, &mut tui, three_panes)?;
    let preview = app
        .conversation_frame
        .divider_area()
        .expect("right divider");
    assert_eq!(preview.x, divider.x - 6);

    crate::boxed_future!(app.handle_tui_event(
        &mut tui,
        &mut server,
        TuiEvent::Key(KeyCode::Enter.into())
    ))
    .await?;
    let selected = std::iter::from_fn(|| events.try_recv().ok())
        .find(|event| {
            matches!(event, AppEvent::SelectAgentsOverviewThread { thread_id } if *thread_id == id)
        })
        .expect("Enter selects the displayed task with the chooser open");
    crate::boxed_future!(app.handle_event(&mut tui, &mut server, selected)).await?;
    assert_eq!(
        app.panel_focus(),
        Some(crate::app::sidebar::PaneFocus::Main)
    );
    crate::boxed_future!(app.handle_tui_event(&mut tui, &mut server, TuiEvent::Paste("!".into())))
        .await?;
    assert!(app.chat_widget.composer_text_with_pending().contains('!'));
    crate::boxed_future!(app.handle_tui_event(
        &mut tui,
        &mut server,
        TuiEvent::Key(KeyCode::Backspace.into()),
    ))
    .await?;

    app.open_agents_overview(&server);
    app.sidebar_mut().hide();
    crate::boxed_future!(app.handle_tui_event(
        &mut tui,
        &mut server,
        TuiEvent::Key(KeyCode::Esc.into())
    ))
    .await?;
    tui.screen_size_for_event(&TuiEvent::Resize(wide))?;
    app.render_chat_widget_frame(&mut tui, wide)?;
    assert_eq!(buffer_text(tui.terminal.previous_buffer()), before);

    app.local_settings.tui.keymap = serde_json::from_value(serde_json::json!({
        "global":{"leader":"f12","open_agents":"leader a","focus_main_composer":["leader c", "f10"]},
        "agents":{"toggle_grouping":"f11 g","new_task":"f10"},
    }))?;
    app.keymap =
        RuntimeKeymap::for_session(&app.local_settings.tui.keymap, /*workspace*/ true).unwrap();
    for key in [KeyCode::F(12), KeyCode::Char('a')] {
        crate::boxed_future!(app.handle_tui_event(
            &mut tui,
            &mut server,
            TuiEvent::Key(key.into())
        ))
        .await?;
    }
    assert!(app.workspace_center_owns_input());
    let repeat =
        |ch| KeyEvent::new_with_kind(KeyCode::Char(ch), KeyModifiers::NONE, KeyEventKind::Repeat);
    crate::boxed_future!(app.handle_tui_event(&mut tui, &mut server, TuiEvent::Key(repeat('a'))))
        .await?;
    assert!(
        app.agents_overview
            .view_state
            .lock()
            .unwrap()
            .removal
            .is_none()
    );
    crate::boxed_future!(app.handle_tui_event(
        &mut tui,
        &mut server,
        TuiEvent::Key(KeyCode::F(10).into())
    ))
    .await?;
    assert!(app.workspace_center_owns_input());
    assert!(
        std::iter::from_fn(|| events.try_recv().ok())
            .any(|event| matches!(event, AppEvent::NewAgentsOverviewSession { .. }))
    );
    for key in [KeyCode::F(11), KeyCode::Char('g')] {
        crate::boxed_future!(app.handle_tui_event(
            &mut tui,
            &mut server,
            TuiEvent::Key(key.into())
        ))
        .await?;
    }
    assert_eq!(
        app.agents_overview.view_state.lock().unwrap().grouping,
        AgentsOverviewGrouping::Directory,
    );
    for key in [KeyCode::F(12), KeyCode::Char('c')] {
        crate::boxed_future!(app.handle_tui_event(
            &mut tui,
            &mut server,
            TuiEvent::Key(key.into())
        ))
        .await?;
    }
    assert!(!app.workspace_center_owns_input());
    crate::boxed_future!(app.handle_tui_event(&mut tui, &mut server, TuiEvent::Key(repeat('c'))))
        .await?;
    assert_eq!(
        app.chat_widget.composer_text_with_pending(),
        "Keep this draft"
    );
    server.shutdown().await?;
    Ok(())
}
