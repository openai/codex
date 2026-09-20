//! Independent warning navigation, full-text copying, and bounded scrolling.

use super::*;
use crate::history_cell::WarningId;
use crate::render::renderable::Renderable;
use crossterm::event::KeyCode;
use crossterm::event::KeyModifiers;
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
    let mut view = WarningsView::new(data, RuntimeKeymap::defaults(), AppEventSender::new(tx));
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
