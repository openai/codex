//! Keep the production shortcut picker's controls anchored while groups and results change.

use super::*;
use crate::app_event_sender::AppEventSender;
use crate::bottom_pane::BottomPaneView;
use crate::bottom_pane::ListSelectionView;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use pretty_assertions::assert_eq;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use tokio::sync::mpsc::unbounded_channel;

#[test]
fn tabs_and_search_keep_production_picker_geometry_stable() {
    for width in [80, 40] {
        for height_limit in [u16::MAX, 16] {
            let runtime = RuntimeKeymap::defaults();
            let params = build_keymap_picker_params(&runtime, &TuiKeymap::default());
            let tab_count = params.tabs.len();
            let (tx, _rx) = unbounded_channel();
            let mut view = ListSelectionView::new(params, AppEventSender::new(tx), runtime.list);
            let mut geometry = Vec::new();

            for (tab_id, tab_text, hint) in [
                (KEYMAP_ALL_TAB_ID, "Common", "left/right"),
                (KEYMAP_CUSTOM_TAB_ID, "Customized (0)", "left/right"),
                (KEYMAP_DEBUG_TAB_ID, "Debug", "enter"),
            ] {
                for _ in 0..tab_count {
                    if BottomPaneView::active_tab_id(&view) == Some(tab_id) {
                        break;
                    }
                    view.handle_key_event(KeyEvent::from(KeyCode::Right));
                }
                assert_eq!(BottomPaneView::active_tab_id(&view), Some(tab_id));

                for query in ["", "zzzz-no-matching-shortcut"] {
                    view.set_search_query(query.to_owned());
                    let desired_height = view.desired_height(width);
                    let height = desired_height.min(height_limit);
                    let area = Rect::new(/*x*/ 0, /*y*/ 0, width, height);
                    let mut buffer = Buffer::empty(area);
                    view.render(area, &mut buffer);
                    let lines = (0..height)
                        .map(|y| {
                            (0..width)
                                .map(|x| buffer[(x, y)].symbol())
                                .collect::<String>()
                        })
                        .collect::<Vec<_>>();
                    let search_text = if query.is_empty() {
                        "Type to search shortcuts"
                    } else {
                        query
                    };
                    let header_y = lines
                        .iter()
                        .position(|line| line.trim() == "Keymap")
                        .expect("picker title remains visible");
                    let tabs_y = lines
                        .iter()
                        .position(|line| line.contains(tab_text))
                        .expect("active group remains visible");
                    let search_y = lines
                        .iter()
                        .position(|line| line.trim() == search_text)
                        .expect("search remains visible");
                    let hint_y = lines
                        .iter()
                        .position(|line| line.trim_start().starts_with(hint))
                        .expect("group-specific footer remains visible");
                    geometry.push((desired_height, height, header_y, tabs_y, search_y, hint_y));
                }
            }

            assert_eq!(
                geometry,
                vec![geometry[0]; geometry.len()],
                "group and search changes must keep controls anchored at width {width} and height limit {height_limit}"
            );
        }
    }
}
