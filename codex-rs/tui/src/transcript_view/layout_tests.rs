//! Cache reuse, bounded lifetime, and invalidation across frames.

use super::*;
use pretty_assertions::assert_eq;
use ratatui::text::Line;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

#[derive(Debug, Default)]
struct Cell {
    renders: AtomicUsize,
    tick: AtomicUsize,
    mutable: bool,
}

impl HistoryCell for Cell {
    fn display_lines(&self, _width: u16) -> Vec<Line<'static>> {
        self.renders.fetch_add(/*val*/ 1, Ordering::Relaxed);
        vec![format!("tick {}", self.tick.load(Ordering::Relaxed)).into()]
    }

    fn raw_lines(&self) -> Vec<Line<'static>> {
        vec![]
    }

    fn has_stable_transcript_height(&self) -> bool {
        !self.mutable
    }

    fn transcript_animation_tick(&self) -> Option<u64> {
        Some(self.tick.load(Ordering::Relaxed) as u64)
    }
}

#[test]
fn layouts_refresh_for_width_animation_and_mutable_frames() {
    for mutable in [false, true] {
        let cell = Arc::new(Cell {
            mutable,
            ..Cell::default()
        });
        let history = cell.clone() as Arc<dyn HistoryCell>;
        let mut cache = LayoutCache::default();
        let get = |cache: &mut LayoutCache, width| {
            cache.get(
                &history,
                width,
                CellPresentation {
                    bidi_layout: false,
                    summarized: false,
                    separated: false,
                    shares_prompt_background: false,
                    turn_tip_space: false,
                    expanded: false,
                    disclosure: false,
                    detailed: false,
                    image_protocol: None,
                },
                || TextLayout::new(history.transcript_hyperlink_lines(width), width),
            )
        };
        cache.begin_frame();
        get(&mut cache, /*width*/ 20);
        get(&mut cache, /*width*/ 20);
        assert_eq!(cell.renders.load(Ordering::Relaxed), 1);
        cache.begin_frame();
        get(&mut cache, /*width*/ 20);
        assert_eq!(
            cell.renders.load(Ordering::Relaxed),
            1 + usize::from(mutable)
        );
        cell.tick.store(/*val*/ 2, Ordering::Relaxed);
        cache.begin_frame();
        let next = get(&mut cache, /*width*/ 20);
        assert_eq!(next.text(), "tick 2");
        get(&mut cache, /*width*/ 10);
        assert_eq!(
            cell.renders.load(Ordering::Relaxed),
            3 + usize::from(mutable)
        );
        // A document drag wraps cached text across widths. On release, restore
        // width-specific rendering even when the last preview had the same width.
        let before_drag = cell.renders.load(Ordering::Relaxed);
        cache.set_document_resize_preview(/*active*/ true);
        let narrow = get(&mut cache, /*width*/ 4);
        assert!(narrow.row_count() > 1);
        let wide = get(&mut cache, /*width*/ 10);
        assert_eq!(wide.row_count(), 1);
        assert_eq!(cell.renders.load(Ordering::Relaxed), before_drag);
        cache.set_document_resize_preview(/*active*/ false);
        get(&mut cache, /*width*/ 10);
        assert_eq!(cell.renders.load(Ordering::Relaxed), before_drag + 1);
        crate::terminal_palette::with_test_default_colors(
            crate::terminal_probe::DefaultColors {
                fg: (12, 34, 56),
                bg: (65, 43, 21),
            },
            || {
                cache.begin_frame();
                get(&mut cache, /*width*/ 10);
                assert_eq!(
                    cell.renders.load(Ordering::Relaxed),
                    5 + usize::from(mutable)
                );
            },
        );
    }
}

#[test]
fn recent_entries_are_reused_and_old_entries_are_evicted() {
    let cells: Vec<_> = (0..100).map(|_| Arc::new(Cell::default())).collect();
    let mut cache = LayoutCache::default();
    cache.begin_frame();
    for cell in &cells {
        let history = cell.clone() as Arc<dyn HistoryCell>;
        cache.get(
            &history,
            /*width*/ 20,
            CellPresentation {
                bidi_layout: false,
                summarized: false,
                separated: false,
                shares_prompt_background: false,
                turn_tip_space: false,
                expanded: false,
                disclosure: false,
                detailed: false,
                image_protocol: None,
            },
            || {
                TextLayout::new(
                    history.transcript_hyperlink_lines(/*width*/ 20),
                    /*width*/ 20,
                )
            },
        );
    }
    for index in [99, 98, 0] {
        let history = cells[index].clone() as Arc<dyn HistoryCell>;
        cache.get(
            &history,
            /*width*/ 20,
            CellPresentation {
                bidi_layout: false,
                summarized: false,
                separated: false,
                shares_prompt_background: false,
                turn_tip_space: false,
                expanded: false,
                disclosure: false,
                detailed: false,
                image_protocol: None,
            },
            || {
                TextLayout::new(
                    history.transcript_hyperlink_lines(/*width*/ 20),
                    /*width*/ 20,
                )
            },
        );
    }
    assert_eq!(
        [99, 98, 0].map(|index| cells[index].renders.load(Ordering::Relaxed)),
        [1, 1, 2]
    );
}

#[tokio::test]
async fn local_markdown_images_appear_at_their_labels_in_compact_replies() {
    use crate::history_cell::AgentMarkdownCell;
    use crate::pets::ImageProtocol;
    use crate::transcript_view::tests::text;
    use crate::tui::FrameRequester;
    use crossterm::event::KeyModifiers;
    use crossterm::event::MouseButton;
    use crossterm::event::MouseEvent;
    use crossterm::event::MouseEventKind;

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("toolbar final.png");
    image::RgbaImage::new(/*width*/ 1, /*height*/ 1)
        .save(&path)
        .unwrap();
    std::fs::copy(&path, dir.path().join("trailing.png")).unwrap();
    let markdown = format!(
        "![Installed toolbar](<{}>)![](trailing.png)\n\n\
         Before ![two\nlines][toolbar] after.\n\n\
         - ![](toolbar%20final.png) caption\n\n\
         ![remote](https://example.com/t.png) and ![missing](missing.png).\n\n\
         Final ![trailing label](trailing.png)\n\n\
         [toolbar]: toolbar%20final.png",
        url::Url::from_file_path(&path).unwrap()
    );
    let cells: Vec<Arc<dyn HistoryCell>> =
        vec![Arc::new(AgentMarkdownCell::new(markdown, dir.path()))];
    let area = Rect::new(
        /*x*/ 3, /*y*/ 4, /*width*/ 48, /*height*/ 32,
    );
    let mut buf = Buffer::empty(area);
    let mut view = TranscriptView {
        copy_on_select: true,
        ..Default::default()
    };
    let (tx, mut rx) = tokio::sync::broadcast::channel(/*capacity*/ 1);
    let frames = FrameRequester::new(tx);
    view.render(area, &mut buf, &cells);
    let original = view.layout(&cells, /*index*/ 0).unwrap();
    assert!(text(&buf).contains("Installed toolbar"));

    view.image_protocol = Some(ImageProtocol::Kitty);
    view.render(area, &mut buf, &cells);
    tokio::time::timeout(std::time::Duration::from_secs(/*secs*/ 10), async {
        while view.image_placements().len() < 5 {
            view.request_visible_images(&frames, async { true }, /*streaming*/ None);
            rx.recv().await.unwrap();
            view.render(area, &mut buf, &cells);
        }
    })
    .await
    .unwrap();
    for placement in view.image_placements() {
        buf[(placement.area.x, placement.area.y)].set_symbol("▣");
    }
    let displayed = crate::terminal_hyperlinks::strip_osc8(&text(&buf));
    let layout = view.layout(&cells, /*index*/ 0).unwrap();
    assert_eq!(layout.text(), original.text());
    let search = Arc::clone(&layout).for_search();
    assert_eq!(search.row_count(), original.row_count());
    assert_eq!(images::regions(&search).count(), 0);
    assert!(search.has_markdown_images);
    assert_eq!(images::regions(&layout.rewrap(/*width*/ 1)).count(), 0);

    let preview = view.image_placements().last().unwrap().area;
    assert_eq!(preview.height, 1);
    let before = (preview.x, preview.y - 1);
    for (column, expected) in [
        (preview.right() - 1, "Final "),
        (preview.right(), "Final trailing label"),
        (preview.right() + 2, "Final trailing label"),
    ] {
        let after = (column, preview.y);
        for (from, to) in [(before, after), (after, before)] {
            for (kind, (column, row)) in [
                (MouseEventKind::Down(MouseButton::Left), from),
                (MouseEventKind::Drag(MouseButton::Left), to),
            ] {
                view.handle_mouse(
                    MouseEvent {
                        kind,
                        column,
                        row,
                        modifiers: KeyModifiers::NONE,
                    },
                    &cells,
                );
            }
            let Some(ViewAction::CopyOnSelect(copied)) = view.handle_mouse(
                MouseEvent {
                    kind: MouseEventKind::Up(MouseButton::Left),
                    column: to.0,
                    row: to.1,
                    modifiers: KeyModifiers::NONE,
                },
                &cells,
            ) else {
                panic!("expected a copy on release for {from:?} → {to:?}");
            };
            assert_eq!(copied, expected, "{from:?} → {to:?}");
        }
    }
    insta::assert_snapshot!("inline_markdown_images", displayed.trim_end());
}

#[test]
fn fullscreen_image_previews_follow_layout_without_entering_text_surfaces() {
    use crate::history_cell::new_image_generation_call;
    use crate::history_cell::new_view_image_tool_call;
    use crate::pets::ImageProtocol;
    use codex_utils_absolute_path::AbsolutePathBuf;
    use codex_utils_path_uri::LegacyAppPathString;

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("artifact.png");
    image::RgbaImage::from_pixel(480, 160, image::Rgba([255, 0, 0, 255]))
        .save(&path)
        .unwrap();
    let square_path = dir.path().join("generated.png");
    image::RgbImage::new(768, 768).save(&square_path).unwrap();
    let cells: Vec<Arc<dyn HistoryCell>> = vec![
        Arc::new(new_view_image_tool_call(
            "view-call".to_string(),
            LegacyAppPathString::from_string(path.to_string_lossy().into_owned()),
        )),
        Arc::new(
            crate::history_cell::AgentMarkdownCell::new(
                "Here is the image.".to_string(),
                dir.path(),
            )
            .with_phase(Some(codex_protocol::models::MessagePhase::FinalAnswer)),
        ),
        Arc::new(new_image_generation_call(
            "image-call".to_string(),
            "completed",
            Some("A square".to_string()),
            Some(AbsolutePathBuf::try_from(square_path.clone()).unwrap()),
        )),
        Arc::new(crate::history_cell::FinalMessageSeparator::new(
            Some(2),
            /*runtime_metrics*/ None,
        )),
    ];
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 140, /*height*/ 40,
    );
    let frame = |buf: &Buffer| {
        let uri = url::Url::from_file_path(&path).unwrap().to_string();
        crate::transcript_view::tests::text(buf)
            .replace(&uri, "file:///workspace/artifact.png")
            .replace(path.to_str().unwrap(), "/workspace/artifact.png")
            .trim_end()
            .to_string()
    };
    let mut view = TranscriptView::default();
    let mut buf = Buffer::empty(area);
    view.render(area, &mut buf, &cells);
    let text_only = buf.clone();
    view.image_protocol = Some(ImageProtocol::Kitty);
    view.render(area, &mut buf, &cells);
    view.sync_turn_details(&cells, /*enabled*/ true, /*active*/ None);
    view.render(area, &mut buf, &cells);
    let collapsed = frame(&buf);
    let collapsed_areas: Vec<_> = view.image_placements().iter().map(|p| p.area).collect();
    assert_eq!(collapsed_areas.len(), 1);
    assert!(
        collapsed.find("Here is the image.").unwrap() < collapsed.find("Generated image").unwrap()
    );
    assert_eq!(collapsed_areas[0].x, area.x + 2);
    assert!(collapsed_areas[0].height > 6);
    view.sync_turn_details(&cells, /*enabled*/ false, /*active*/ None);
    view.render(area, &mut buf, &cells);
    // Find may request full text, but only manual expansion may reveal inspected pixels.
    assert!(
        view.entry_layout(&cells, /*index*/ 0, /*full_content*/ true)
            .unwrap()
            .image
            .is_none()
    );

    // An inspected image only appears after its existing activity control is expanded.
    assert!(view.toggle_disclosure_at(&cells, /*column*/ 4, /*row*/ 1));
    view.image_protocol = None;
    view.render(area, &mut buf, &cells);
    let text: Vec<_> = (0..cells.len())
        .map(|index| view.layout(&cells, index).unwrap().text().to_string())
        .collect();
    view.image_protocol = Some(ImageProtocol::Kitty);
    view.render(area, &mut buf, &cells);
    let expanded_areas: Vec<_> = view.image_placements().iter().map(|p| p.area).collect();
    insta::assert_snapshot!(
        "fullscreen_image_artifact_rows",
        format!(
            "Collapsed inspection / generated output\n{collapsed}\nGraphics: {collapsed_areas:?}\n\nExpanded inspection / generated output\n{}\nGraphics: {expanded_areas:?}",
            frame(&buf)
        )
    );
    for index in [0, 2] {
        let expected = &text[index];
        let layout = view.layout(&cells, index).unwrap();
        assert_eq!(layout.text(), expected);
        let pinned_narrow = layout.rewrap(/*width*/ 8);
        assert!(pinned_narrow.visible_image().is_none());
        let pinned_restored = pinned_narrow.rewrap(area.width);
        assert_eq!(pinned_restored.row_count(), layout.row_count());
        assert!(pinned_restored.visible_image().is_some());
    }
    // Short, inset viewports may begin in an image. Its placement stays inside the viewport.
    let short = Rect::new(
        /*x*/ 7, /*y*/ 5, /*width*/ 16, /*height*/ 5,
    );
    view.jump_to_latest();
    view.render(short, &mut Buffer::empty(short), &cells);
    let placements = view.image_placements();
    assert!(!placements.is_empty());
    assert!(
        placements
            .iter()
            .all(|placement| short.contains(placement.area.as_position())
                && placement.area.right() <= short.right()
                && placement.area.bottom() <= short.bottom())
    );

    view.render(area, &mut buf, &cells);
    let control_row = view
        .visible
        .iter()
        .position(|visible| visible.layout.disclosure_row() == Some(visible.row))
        .unwrap() as u16;
    assert!(view.toggle_disclosure_at(&cells, /*column*/ 4, control_row));
    view.render(area, &mut buf, &cells);

    // Raw mode and unsupported terminals reserve no rows, even after rich previews were cached.
    view.jump_to_latest();
    view.set_presentation(/*detailed*/ false, HistoryRenderMode::Raw);
    view.render(area, &mut buf, &cells);
    assert!((0..cells.len()).all(|index| view.layout(&cells, index).unwrap().image.is_none()));
    view.set_presentation(/*detailed*/ false, HistoryRenderMode::Rich);
    view.image_protocol = None;
    view.render(area, &mut buf, &cells);
    assert_eq!(buf, text_only);

    // Resumed sessions with a deleted file fall back to the original text entry.
    std::fs::remove_file(&path).unwrap();
    std::fs::remove_file(&square_path).unwrap();
    view.image_protocol = Some(ImageProtocol::Kitty);
    view.render(area, &mut buf, &cells);
    assert_eq!(buf, text_only);
    assert!(view.image_placements().is_empty());

    // A tiny compressed file with a large decoded raster must also fall back to text.
    image::RgbImage::new(4000, 3000).save(&path).unwrap();
    view.image_protocol = None;
    view.render(area, &mut buf, &cells);
    view.image_protocol = Some(ImageProtocol::Kitty);
    view.render(area, &mut buf, &cells);
    assert_eq!(buf, text_only);
    assert!(view.image_placements().is_empty());
}
