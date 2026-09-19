//! Reading and prompt backtracking follow current entries through live updates and completion.

use super::*;
use crate::history_cell::PlainHistoryCell;
use crate::history_cell::UserHistoryCell;
use crate::keymap::RuntimeKeymap;
use pretty_assertions::assert_eq;
use ratatui::style::Modifier;

fn user(message: &str) -> Arc<dyn HistoryCell> {
    Arc::new(UserHistoryCell {
        message: message.to_owned(),
        text_elements: Vec::new(),
        local_image_paths: Vec::new(),
        remote_image_urls: Vec::new(),
        spoken: false,
    })
}

fn render(overlay: &mut TranscriptOverlay, area: Rect) -> Buffer {
    let mut buffer = Buffer::empty(area);
    overlay.render(area, &mut buffer);
    buffer
}

fn content_rows(overlay: &TranscriptOverlay, buffer: &Buffer) -> Vec<String> {
    let area = overlay.content_area;
    (area.top()..area.bottom())
        .map(|y| {
            (area.left()..area.right())
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
                .trim_end()
                .to_owned()
        })
        .collect()
}

#[test]
fn reading_live_output_updates_in_place_and_stays_on_the_committed_entry() {
    let mut overlay =
        TranscriptOverlay::new(vec![user("first prompt")], RuntimeKeymap::defaults().pager);
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 40, /*height*/ 9,
    );
    for revision in ["streaming", "updated"] {
        let lines = (0..8)
            .map(|row| HyperlinkLine::from(format!("{revision} row {row}")))
            .collect();
        overlay.sync_live_tail(area.width, /*key*/ None, |_| Some(lines));
        if revision == "streaming" {
            render(&mut overlay, area);
            overlay.scroll(/*rows*/ -2);
        }
        let buffer = render(&mut overlay, area);
        assert_eq!(
            content_rows(&overlay, &buffer),
            (2..6)
                .map(|row| format!("{revision} row {row}"))
                .collect::<Vec<_>>()
        );
        assert!(!overlay.view.is_following());
    }
    overlay.insert_cell(Arc::new(PlainHistoryCell::new(
        (0..8)
            .map(|row| Line::from(format!("completed row {row}")))
            .collect(),
    )));
    overlay.sync_live_tail(area.width, /*key*/ None, |_| None);
    overlay.insert_cell(user("queued prompt"));
    let completed = render(&mut overlay, area);
    assert_eq!(
        content_rows(&overlay, &completed),
        [
            "completed row 2",
            "completed row 3",
            "completed row 4",
            "completed row 5"
        ]
    );
    overlay.set_highlight_cell(Some(2));
    let highlighted = render(&mut overlay, area);
    assert!(
        content_rows(&overlay, &highlighted)
            .iter()
            .any(|row| row == "› queued prompt")
    );
    assert!(!overlay.view.is_following());
    overlay.view.jump_to_latest();
    let latest = render(&mut overlay, area);
    let latest = content_rows(&overlay, &latest).join("\n");
    assert!(latest.contains("queued prompt"));
    assert!(!latest.contains("updated row"));
}

#[test]
fn backtracking_reveals_a_prompt_when_only_its_final_padding_is_visible() {
    let mut overlay = TranscriptOverlay::new(
        vec![
            user("first prompt"),
            Arc::new(PlainHistoryCell::new(vec![
                "tail zero".into(),
                "tail one".into(),
                "tail two".into(),
            ])),
        ],
        RuntimeKeymap::defaults().pager,
    );
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 40, /*height*/ 10,
    );
    let before = render(&mut overlay, area);
    assert_eq!(
        content_rows(&overlay, &before),
        ["", "", "tail zero", "tail one", "tail two"]
    );

    let first_prompt_index = 0;
    overlay.set_highlight_cell(Some(first_prompt_index));
    let highlighted = render(&mut overlay, area);
    let rows = content_rows(&overlay, &highlighted);
    let prompt_row = rows
        .iter()
        .position(|row| row == "› first prompt")
        .expect("backtracking reveals the prompt body, not just its padding");
    let y = overlay.content_area.y + prompt_row as u16;
    assert!(
        (2..2 + "first prompt".len() as u16)
            .all(|x| highlighted[(x, y)].modifier.contains(Modifier::REVERSED))
    );
}

#[test]
fn prompt_highlight_leaves_gutters_and_padding_unreversed() {
    crate::terminal_palette::with_test_default_colors(
        crate::terminal_probe::DefaultColors {
            fg: (32, 32, 32),
            bg: (255, 255, 255),
        },
        || {
            let mut snapshots = Vec::new();
            for (width, height) in [(40, 10), (12, 12)] {
                let mut overlay = TranscriptOverlay::new(
                    vec![
                        Arc::new(PlainHistoryCell::new(vec!["prior".into()])),
                        user("alpha beta gamma"),
                    ],
                    RuntimeKeymap::defaults().pager,
                );
                let first_prompt_index = 1;
                overlay.set_highlight_cell(Some(first_prompt_index));
                let area = Rect::new(/*x*/ 0, /*y*/ 0, width, height);
                let buffer = render(&mut overlay, area);
                let content = Buffer {
                    area: Rect::new(
                        /*x*/ 0,
                        /*y*/ 0,
                        width,
                        overlay.content_area.height,
                    ),
                    content: buffer
                        .content
                        .chunks(usize::from(width))
                        .skip(usize::from(overlay.content_area.y))
                        .take(usize::from(overlay.content_area.height))
                        .flatten()
                        .cloned()
                        .collect(),
                };
                snapshots.push(format!("{width} columns\n{content:?}"));
            }
            insta::assert_snapshot!(snapshots.join("\n\n"));
        },
    );
}
