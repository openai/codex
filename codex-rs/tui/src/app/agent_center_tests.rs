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
