//! Configured child-menu navigation and disabled-description alignment.

use super::*;
use crate::bottom_pane::BottomPaneView;
use crate::bottom_pane::ListSelectionView;
use crate::key_hint;
use crate::render::renderable::Renderable;
use pretty_assertions::assert_eq;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use tokio::sync::mpsc::unbounded_channel;

fn child_menus(runtime: &RuntimeKeymap) -> [(&'static str, SelectionViewParams); 3] {
    let config = TuiKeymap::default();
    let conflicting = keymap_with_replacement(&config, "composer", "submit", "tab")
        .expect("valid key specification");
    let error = RuntimeKeymap::from_config(&conflicting).expect_err("tab already queues a message");
    [
        (
            "action",
            build_keymap_action_menu_params(
                "global".to_string(),
                "open_transcript".to_string(),
                runtime,
                &config,
            ),
        ),
        (
            "replace",
            build_keymap_replace_binding_menu_params(
                "global".to_string(),
                "open_transcript".to_string(),
                runtime,
            ),
        ),
        (
            "conflict",
            build_keymap_conflict_params(
                "composer".to_string(),
                "submit".to_string(),
                "tab".to_string(),
                KeymapEditIntent::ReplaceAll,
                error,
                runtime,
            ),
        ),
    ]
}

fn render_menu(view: &ListSelectionView) -> Vec<String> {
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 40, /*height*/ 16,
    );
    let mut buf = Buffer::empty(area);
    view.render(area, &mut buf);
    (0..area.height)
        .map(|y| {
            (0..area.width)
                .map(|x| buf[(x, y)].symbol())
                .collect::<String>()
                .trim_end()
                .to_owned()
        })
        .collect()
}

#[test]
fn child_menu_hints_and_actions_follow_configured_list_bindings() {
    let mut runtime = RuntimeKeymap::defaults();
    runtime.list.accept = vec![key_hint::plain(KeyCode::F(/*n*/ 3))];
    runtime.list.cancel = vec![key_hint::plain(KeyCode::F(/*n*/ 2))];
    for (_, params) in child_menus(&runtime) {
        let (tx, mut rx) = unbounded_channel();
        let mut view =
            ListSelectionView::new(params, AppEventSender::new(tx), runtime.list.clone());
        let rows = render_menu(&view);
        assert_eq!(rows.last().unwrap().trim(), "f3 select · f2 back");
        view.handle_key_event(KeyEvent::from(KeyCode::Enter));
        assert!(rx.try_recv().is_err());
        view.handle_key_event(KeyEvent::from(KeyCode::F(/*n*/ 3)));
        assert!(matches!(
            rx.try_recv(),
            Ok(AppEvent::OpenKeymapCapture {
                capture_mode: KeymapCaptureMode::SingleKey,
                ..
            })
        ));
    }

    for (_, params) in child_menus(&runtime) {
        let (tx, mut rx) = unbounded_channel();
        let mut view =
            ListSelectionView::new(params, AppEventSender::new(tx), runtime.list.clone());
        view.handle_key_event(KeyEvent::from(KeyCode::F(/*n*/ 2)));
        assert!(view.is_complete());
        assert!(rx.try_recv().is_err());
    }
}

#[test]
fn disabled_reason_wraps_in_the_description_column() {
    let runtime = RuntimeKeymap::defaults();
    let params = build_keymap_action_menu_params(
        "global".to_string(),
        "open_transcript".to_string(),
        &runtime,
        &TuiKeymap::default(),
    );
    let (tx, _rx) = unbounded_channel();
    let view = ListSelectionView::new(params, AppEventSender::new(tx), runtime.list);
    let width = 64;
    let area = Rect::new(
        /*x*/ 0,
        /*y*/ 0,
        width,
        view.desired_height(width),
    );
    let mut buf = Buffer::empty(area);
    view.render(area, &mut buf);
    let rows = buf
        .content()
        .chunks(usize::from(width))
        .map(|row| {
            row.iter()
                .map(ratatui::buffer::Cell::symbol)
                .collect::<String>()
        })
        .collect::<Vec<_>>();
    let y = rows
        .iter()
        .position(|row| row.contains("Remove custom binding (disabled)"))
        .expect("disabled action stays visible");
    let reason_start = rows[y].find("No custom root override").unwrap();
    let continuation_start = rows[y + 1].find("to remove").unwrap();
    assert_eq!(
        crate::width::display_width(&rows[y + 1][..continuation_start]),
        crate::width::display_width(&rows[y][..reason_start])
    );
}
