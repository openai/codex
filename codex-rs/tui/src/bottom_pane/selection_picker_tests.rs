//! Picker panels retain controls and page by the results that actually fit.

use super::*;
use crate::app_event::AppEvent;
use pretty_assertions::assert_eq;
use ratatui::style::Color;
use tokio::sync::mpsc::unbounded_channel;

fn browser() -> ListSelectionView {
    let (tx, _rx) = unbounded_channel::<AppEvent>();
    ListSelectionView::new(
        SelectionViewParams {
            appearance: crate::bottom_pane::SelectionAppearance::Picker,
            picker_surface: PickerSurface::Panel,
            max_visible_rows: 24,
            title: Some("Keymap".to_owned()),
            subtitle: Some("All configurable shortcuts.".to_owned()),
            is_searchable: true,
            search_placeholder: Some("Type to search shortcuts".to_owned()),
            footer_hint: Some("left/right group · enter edit · esc close".into()),
            row_display: SelectionRowDisplay::SingleLine,
            tabs: ["All", "Common", "Customized (3)", "Unbound (4)", "App"]
                .into_iter()
                .map(|label| SelectionTab {
                    id: label.to_owned(),
                    label: label.to_owned(),
                    header: Box::new(()),
                    items: (0..30)
                        .map(|index| SelectionItem {
                            name: format!("Action {index:02}"),
                            description: Some(format!("Ctrl+{index}")),
                            search_value: Some(format!("Action {index:02}")),
                            ..Default::default()
                        })
                        .collect(),
                })
                .collect(),
            ..Default::default()
        },
        AppEventSender::new(tx),
        crate::keymap::RuntimeKeymap::defaults().list,
    )
}

fn render_browser(view: &ListSelectionView, width: u16, height: u16) -> (String, Buffer) {
    let area = Rect::new(/*x*/ 0, /*y*/ 0, width, height);
    let mut buf = Buffer::empty(area);
    view.render(area, &mut buf);
    let text = (0..height)
        .map(|y| {
            (0..width)
                .map(|x| buf[(x, y)].symbol())
                .collect::<String>()
                .trim_end()
                .to_owned()
        })
        .collect::<Vec<_>>()
        .join("\n");
    (text, buf)
}

#[test]
fn shared_menu_presentation_at_wide_and_narrow_sizes() {
    let keymap = crate::keymap::RuntimeKeymap::defaults().list;
    let (tx, _rx) = unbounded_channel();
    let view = ListSelectionView::new(
        SelectionViewParams {
            appearance: crate::bottom_pane::SelectionAppearance::Picker,
            title: Some("Choose an option".into()),
            subtitle: Some("A menu with descriptions and disabled choices.".into()),
            items: vec![
                SelectionItem {
                    name: "Recommended option".into(),
                    description: Some("The description column is visible when it fits.".into()),
                    ..Default::default()
                },
                SelectionItem {
                    name: "A long option label that still wraps at narrow widths".into(),
                    description: Some("Additional details".into()),
                    ..Default::default()
                },
                SelectionItem {
                    name: "Unavailable".into(),
                    disabled_reason: Some("This option needs a connected server.".into()),
                    ..Default::default()
                },
            ],
            footer_hint: Some(super::super::popup_consts::picker_hint_line_for_keymap(
                &keymap,
            )),
            ..SelectionViewParams::picker()
        },
        AppEventSender::new(tx),
        keymap,
    );
    let mut snapshots = Vec::new();
    for (width, height) in [(80, 24), (40, 16)] {
        let (text, buffer) = render_browser(&view, width, height);
        let selected_y = text.lines().position(|line| line.starts_with('›')).unwrap() as u16;
        assert_eq!(
            (0..width)
                .map(|x| buffer[(x, selected_y)].bg)
                .collect::<Vec<_>>(),
            vec![selection_style().bg.unwrap(); usize::from(width)]
        );
        assert!(text.contains("enter select · esc back"));
        snapshots.push(format!("{width}x{height}\n{text}"));
    }
    insta::assert_snapshot!(snapshots.join("\n\n"));
}

#[test]
fn tiny_viewports_preserve_query_and_selection_when_enlarged() {
    let mut view = browser();
    view.handle_key_event(KeyEvent::from(KeyCode::Char('2')));
    view.handle_key_event(KeyEvent::from(KeyCode::Down));
    let selected = view.selected_actual_idx();
    let before = render_browser(&view, /*width*/ 80, /*height*/ 24).0;

    for (width, height) in [(0, 0), (1, 1), (4, 1), (12, 3), (40, 8)] {
        render_browser(&view, width, height);
        assert_eq!(
            (view.search_query.as_str(), view.selected_actual_idx()),
            ("2", selected)
        );
    }

    assert_eq!(render_browser(&view, /*width*/ 80, /*height*/ 24).0, before);
}

#[test]
fn short_browser_keeps_header_tabs_search_and_footer() {
    let view = browser();
    let mut snapshots = Vec::new();
    for (width, height) in [(48, 6), (48, 7), (48, 13), (24, 6), (24, 7)] {
        let (text, _) = render_browser(&view, width, height);
        assert!(text.contains("Keymap"));
        assert!(text.contains("Common"));
        assert!(text.contains("Type to search"));
        assert!(text.contains("left/right group"));
        assert!(view.rendered_item_count() > 0);
        snapshots.push(format!("{width}×{height}:\n{text}"));
    }
    insta::assert_snapshot!(snapshots.join("\n---\n"));
}

#[test]
fn browser_page_navigation_uses_rows_that_fit() {
    let mut view = browser();
    let (_, initial_buf) = render_browser(&view, /*width*/ 60, /*height*/ 18);
    let visible = view.rendered_item_count();
    assert!(visible < view.max_visible_rows);
    view.handle_key_event(KeyEvent::from(KeyCode::PageDown));
    let (text, buf) = render_browser(&view, /*width*/ 60, /*height*/ 18);
    insta::assert_snapshot!(text);
    assert_eq!(view.selected_actual_idx(), Some(visible));
    let selected_row = text
        .lines()
        .position(|line| line.starts_with('›'))
        .expect("focused row");
    assert_eq!(
        buf[(0, selected_row as u16)].bg,
        selection_style().bg.unwrap()
    );
    assert_eq!(buf[(0, 1)].bg, initial_buf[(0, 1)].bg);
}

#[test]
fn shrinking_the_browser_keeps_selection_visible_without_navigation() {
    for row_display in [
        SelectionRowDisplay::SingleLine,
        SelectionRowDisplay::Wrapped,
    ] {
        let mut view = browser();
        view.row_display = row_display;
        view.state.selected_idx = Some(7);
        view.state.scroll_top = 0;
        render_browser(&view, /*width*/ 60, /*height*/ 24);
        let initial_count = view.rendered_item_count();
        assert!(initial_count >= 8);

        let (text, _) = render_browser(&view, /*width*/ 60, /*height*/ 13);
        assert!(view.rendered_item_count() < initial_count);
        assert_eq!(view.selected_actual_idx(), Some(7));
        let focused = text
            .lines()
            .find(|line| line.starts_with('›'))
            .expect("focused row");
        assert!(focused.contains("Action 07"), "{text}");
    }
}

#[test]
fn overflow_arrows_follow_the_painted_picker_surface() {
    for (fg, bg) in [
        ((32, 32, 32), (255, 255, 255)),
        ((240, 240, 240), (24, 24, 24)),
        ((240, 240, 240), (80, 80, 80)),
        ((32, 32, 32), (130, 130, 130)),
    ] {
        crate::terminal_palette::with_test_default_colors(
            crate::terminal_probe::DefaultColors { fg, bg },
            || {
                for surface in [PickerSurface::Panel, PickerSurface::Terminal] {
                    let mut view = browser();
                    view.picker_surface = surface;
                    view.state.selected_idx = Some(15);
                    view.state.scroll_top = 12;
                    let (text, buf) = render_browser(&view, /*width*/ 60, /*height*/ 18);
                    let selected_y = text
                        .lines()
                        .position(|line| line.starts_with('›'))
                        .expect("focused row") as u16;
                    let selected_background = selection_style().bg.unwrap();
                    assert_eq!(
                        (0..buf.area.width)
                            .map(|x| buf[(x, selected_y)].bg)
                            .collect::<Vec<_>>(),
                        vec![selected_background; usize::from(buf.area.width)],
                    );
                    let background = match surface {
                        PickerSurface::Panel => crate::style::user_message_style().bg.unwrap(),
                        PickerSurface::Terminal => Color::Reset,
                    };
                    let arrows = (0..buf.area.height)
                        .filter(|&y| matches!(buf[(0, y)].symbol(), "↑" | "↓"))
                        .map(|y| (y, buf[(0, y)].clone(), buf[(1, y)].clone()))
                        .collect::<Vec<_>>();
                    assert_eq!(
                        arrows
                            .iter()
                            .map(|(_, cell, _)| cell.symbol())
                            .collect::<Vec<_>>(),
                        vec!["↑", "↓"],
                    );
                    for (y, cell, adjacent) in &arrows {
                        assert_ne!(*y, selected_y);
                        assert_ne!(cell.bg, selected_background);
                        assert_eq!(
                            (cell.fg, cell.bg, adjacent.bg),
                            (
                                crate::style::accent_color_on(Some(background)),
                                background,
                                background,
                            ),
                        );
                    }
                }
            },
        );
    }
}
