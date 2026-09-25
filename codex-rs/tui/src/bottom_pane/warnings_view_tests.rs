//! Independent warning navigation, full-text copying, and bounded scrolling.

use super::*;
use crate::history_cell::WarningId;
use crate::render::renderable::Renderable;
use crossterm::event::KeyCode;
use crossterm::event::KeyModifiers;
use pretty_assertions::assert_eq;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use tokio::sync::mpsc::unbounded_channel;

fn entries() -> Vec<WarningEntry> {
    vec![
        WarningEntry {
            id: WarningId::McpServer("example".into()),
            source: "MCP · example".into(),
            details: "MCP example could not connect\nSign in again using codex mcp login example"
                .into(),
        },
        WarningEntry {
            id: WarningId::Message("config".into()),
            source: "Startup".into(),
            details: "Unknown setting `old_option`\nRemove it from config.toml".into(),
        },
    ]
}

fn key(view: &mut WarningsView, code: KeyCode) -> bool {
    view.handle_key(KeyEvent::new(code, KeyModifiers::NONE))
}

fn draw(view: &WarningsView, width: u16, height: u16) -> String {
    let area = Rect::new(/*x*/ 0, /*y*/ 0, width, height);
    let mut buffer = Buffer::empty(area);
    view.render(area, &mut buffer);
    buffer
        .content
        .chunks(usize::from(width))
        .map(|row| {
            row.iter()
                .map(ratatui::buffer::Cell::symbol)
                .collect::<String>()
                .trim_end()
                .to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn warning_pages_copy_only_the_current_diagnostic() {
    let (tx, mut rx) = unbounded_channel();
    let mut view = WarningsView::new(
        entries(),
        Arc::default(),
        RuntimeKeymap::from_config(&toml::from_str("[list]\nmove_right = 'x'").unwrap()).unwrap(),
        AppEventSender::new(tx),
    );
    let (code, modifiers) = RuntimeKeymap::defaults().app.copy[0].parts();
    for (navigation, index) in [(KeyCode::Char('x'), 1), (KeyCode::Left, 0)] {
        key(&mut view, navigation);
        let screen = draw(&view, /*width*/ 80, /*height*/ 12);
        assert!(screen.contains(&format!("{} of 2", index + 1)));
        assert!(screen.contains(entries()[index].details.lines().next().unwrap()));
        view.handle_key(KeyEvent::new(code, modifiers));
        assert!(
            matches!(rx.try_recv(), Ok(AppEvent::CopyWarning(text)) if text == entries()[index].details)
        );
    }
    assert!(key(&mut view, KeyCode::Esc));
}

#[test]
fn warning_pages_wrap_and_scroll_long_diagnostics() {
    let (tx, _) = unbounded_channel();
    let mut data = entries();
    data[0].details = format!(
        "Long diagnostic 日本語\n{}\nEND OF DIAGNOSTIC",
        "Details retained in full, including remediation instructions.\n".repeat(/*n*/ 20)
    );
    let mut view = WarningsView::new(
        data,
        Arc::default(),
        RuntimeKeymap::defaults(),
        AppEventSender::new(tx),
    );
    insta::assert_snapshot!("warnings_narrow", draw(&view, /*width*/ 40, /*height*/ 9));
    key(&mut view, KeyCode::PageDown);
    assert!(!draw(&view, /*width*/ 40, /*height*/ 9).contains("Long diagnostic"));
    key(&mut view, KeyCode::End);
    assert!(draw(&view, /*width*/ 40, /*height*/ 9).contains("END OF DIAGNOSTIC"));
    key(&mut view, KeyCode::Right);
    assert!(draw(&view, /*width*/ 40, /*height*/ 9).contains("Unknown setting"));
    key(&mut view, KeyCode::Left);
    assert!(draw(&view, /*width*/ 40, /*height*/ 9).contains("Long diagnostic"));
}

#[test]
fn warnings_dismiss_only_drawn_pages_after_navigation_skips_a_frame() {
    let (tx, mut rx) = unbounded_channel();
    let mut data = entries();
    data.push(WarningEntry {
        id: WarningId::Message("later".into()),
        source: "Startup".into(),
        details: "Not yet viewed".into(),
    });
    let mut view = WarningsView::new(
        data,
        Arc::default(),
        RuntimeKeymap::defaults(),
        AppEventSender::new(tx),
    );
    assert!(draw(&view, /*width*/ 80, /*height*/ 12).contains("1 of 3"));
    assert!(!key(&mut view, KeyCode::Right));
    assert!(!key(&mut view, KeyCode::Right));
    assert!(draw(&view, /*width*/ 80, /*height*/ 12).contains("3 of 3"));
    key(&mut view, KeyCode::Left);
    assert!(key(&mut view, KeyCode::F(2)));
    view.close();
    let Ok(AppEvent::UpdateWarnings { dismissed, .. }) = rx.try_recv() else {
        panic!("expected warning decisions");
    };
    assert_eq!(
        dismissed,
        vec![
            entries()[0].clone(),
            WarningEntry {
                id: WarningId::Message("later".into()),
                source: "Startup".into(),
                details: "Not yet viewed".into(),
            },
        ]
    );
}

#[test]
fn warnings_keep_current_and_next_without_dismissing_unvisited_pages() {
    let (tx, mut rx) = unbounded_channel();
    let mut data = entries();
    data.push(WarningEntry {
        id: WarningId::Message("later".into()),
        source: "Startup".into(),
        details: "Not yet viewed".into(),
    });
    let expected = vec![data[1].clone()];
    // Keep owns k in this viewer even when a list action is remapped to it.
    let keymap = RuntimeKeymap::from_config(
        &toml::from_str("[list]\nmove_up = 'up'\ncancel = 'k'").unwrap(),
    )
    .unwrap();
    let mut view = WarningsView::new(data, Arc::default(), keymap, AppEventSender::new(tx));
    assert!(!key(&mut view, KeyCode::Char('k')));
    let screen = draw(&view, /*width*/ 80, /*height*/ 12);
    assert!(screen.contains("2 of 3"));
    assert!(screen.contains("ctrl+c dismiss & close"));
    let repeat =
        KeyEvent::new_with_kind(KeyCode::Char('k'), KeyModifiers::NONE, KeyEventKind::Repeat);
    assert!(!view.handle_key(repeat));
    assert!(draw(&view, /*width*/ 80, /*height*/ 12).contains("2 of 3"));
    key(&mut view, KeyCode::Left);
    assert!(key(&mut view, KeyCode::F(2)));
    view.close();
    let Ok(AppEvent::UpdateWarnings {
        dismissed, kept, ..
    }) = rx.try_recv()
    else {
        panic!("expected warning decisions");
    };
    assert_eq!((dismissed, kept), (expected, vec![entries()[0].clone()]));
}
