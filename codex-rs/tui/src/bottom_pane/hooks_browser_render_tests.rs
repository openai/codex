//! Regression coverage for wide hook selection styling and URL-preserving detail truncation.

use super::super::tests::hook;
use super::super::tests::render_buffer;
use super::super::tests::render_lines;
use super::*;
use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::bottom_pane::bottom_pane_view::BottomPaneView;
use crate::test_support::PathBufExt;
use crate::test_support::test_path_buf;
use codex_app_server_protocol::HookEventName;
use codex_app_server_protocol::HookSource;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use insta::assert_snapshot;
use pretty_assertions::assert_eq;
use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Style;
use tokio::sync::mpsc::unbounded_channel;

#[test]
fn wide_titles_preserve_reverse_video_selection_and_dim_index() {
    let style = Style::default()
        .fg(Color::Reset)
        .bg(Color::Reset)
        .bold()
        .not_dim()
        .reversed();
    let row = Line::from(vec![
        "› [x] ".into(),
        " 1".dim(),
        "  检查 🦀 shell commands".into(),
    ])
    .style(style);
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 40, /*height*/ 1,
    );
    let mut buf = Buffer::empty(area);

    render_line_rows(area, &mut buf, vec![row], ScrollState::new());

    assert_eq!(
        buf.content
            .iter()
            .map(|cell| (cell.fg, cell.bg, cell.modifier.contains(Modifier::REVERSED)))
            .collect::<Vec<_>>(),
        vec![(Color::Reset, Color::Reset, true); usize::from(area.width)]
    );
    assert!(buf[(7, 0)].modifier.contains(Modifier::DIM));
    assert!(!buf[(10, 0)].modifier.contains(Modifier::DIM));
    assert_eq!(buf[(10, 0)].symbol(), "检");
}

#[test]
fn wrapped_hook_command_preserves_the_complete_url_destination() {
    let url = "https://github.com/openai/codex/pull/12345?diff=split&review=complete-destination";
    let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
    let mut url_hook = hook(
        "path:review",
        HookEventName::PreToolUse,
        HookSource::User,
        /*plugin_id*/ None,
        &format!("curl {url}"),
        /*enabled*/ true,
        /*is_managed*/ false,
        /*display_order*/ 0,
    );
    url_hook.source_path = test_path_buf("/tmp/h.json").abs();
    let mut view = HooksBrowserView::new(
        vec![url_hook],
        Vec::new(),
        Vec::new(),
        AppEventSender::new(tx_raw),
    );
    view.handle_key_event(KeyEvent::from(KeyCode::Enter));
    let buf = render_buffer(&view, /*width*/ 44);
    let linked = buf
        .content
        .iter()
        .filter(|cell| cell.symbol().contains("\x1b]8;;"))
        .map(|cell| {
            assert!(cell.symbol().starts_with(&format!("\x1b]8;;{url}\x07")));
            crate::terminal_hyperlinks::strip_osc8(cell.symbol())
        })
        .collect::<String>();
    assert_eq!(
        linked,
        "https://github.com/openai/codex/pull/12345?diff=split&review=complete-"
    );
    let ellipses = buf
        .content
        .iter()
        .filter(|cell| crate::terminal_hyperlinks::strip_osc8(cell.symbol()) == "…")
        .map(ratatui::buffer::Cell::symbol)
        .collect::<Vec<_>>();
    assert_eq!(ellipses, vec!["…"]);
    assert_snapshot!(
        "hooks_browser_wrapped_url",
        render_lines(&view, /*width*/ 44)
    );
}
