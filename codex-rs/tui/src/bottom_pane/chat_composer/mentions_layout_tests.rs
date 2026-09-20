//! Production mention overlays retain the draft, status footer and editor cursor across scopes.

use super::*;
use pretty_assertions::assert_eq;
use ratatui::layout::Size;
use tokio::sync::mpsc::unbounded_channel;

fn composer() -> ChatComposer {
    let (tx, _rx) = unbounded_channel::<AppEvent>();
    let mut composer = ChatComposer::new(
        /*has_input_focus*/ true,
        AppEventSender::new(tx),
        /*enhanced_keys_supported*/ false,
        "Ask Codex".to_string(),
        /*disable_paste_burst*/ true,
    );
    composer.set_status_line_enabled(/*enabled*/ true);
    composer.set_status_line(Some("MODEL STATUS".into()));
    composer.set_mentions_v2_enabled(/*enabled*/ true);
    composer.set_plugin_mentions(Some(
        (0..12)
            .map(|index| PluginCapabilitySummary {
                config_name: format!("plugin{index:02}@test"),
                display_name: format!("Plugin{index:02}"),
                plugin_namespace: None,
                description: Some(format!("Plugin description {index:02}")),
                has_skills: false,
                mcp_server_names: Vec::new(),
                app_connector_ids: Vec::new(),
            })
            .collect(),
    ));
    composer.set_text_content("@".to_string(), Vec::new(), Vec::new());
    composer.draft.textarea.set_cursor("@".len());
    composer
}

fn render_overlay(
    composer: &ChatComposer,
    size: Size,
    footer: &TranscriptFooter,
) -> (Buffer, Rect, Option<(u16, u16)>) {
    let options = composer.resolve_render_options(ComposerRenderOptions {
        command_popup_placement: CommandPopupPlacement::Overlay,
        footer: Some(footer),
        ..Default::default()
    });
    let mut buffer = Buffer::empty(Rect::new(
        /*x*/ 0,
        /*y*/ 0,
        size.width,
        size.height,
    ));
    let height = composer
        .desired_height_with_options(size.width, options)
        .min(size.height);
    let area = Rect::new(/*x*/ 0, size.height - height, size.width, height);
    for y in 0..area.y {
        Line::from(format!("Transcript row {y:02}: behind the menu").magenta())
            .render(Rect::new(/*x*/ 0, y, size.width, /*height*/ 1), &mut buffer);
    }
    composer.render_with_options(area, &mut buffer, /*mask_char*/ None, options);
    (
        buffer,
        area,
        composer.cursor_pos_with_options(area, options),
    )
}

fn buffer_text(buffer: &Buffer) -> String {
    buffer
        .content
        .chunks(usize::from(buffer.area.width))
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
fn mention_scopes_overlay_history_without_moving_composer_or_footer() {
    let footer = TranscriptFooter {
        text: "TRANSCRIPT STATUS".into(),
        cursor_column: None,
        is_interactive: false,
    };
    for (width, height) in [(80, 32), (40, 32), (80, 12), (80, 7), (80, 6), (80, 5)] {
        let mut composer = composer();
        composer.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        let size = Size::new(width, height);
        let (closed, composer_area, cursor) = render_overlay(&composer, size, &footer);
        let composer_start = closed.index_of(composer_area.x, composer_area.y);
        composer.popups.dismissed_mention_token = None;
        composer.sync_popups();
        let mut menu_row = None;
        for scope in ["all", "filesystem", "plugins"] {
            let (open, area, open_cursor) = render_overlay(&composer, size, &footer);
            assert_eq!((area, open_cursor), (composer_area, cursor));
            assert_eq!(
                &open.content[composer_start..],
                &closed.content[composer_start..],
            );
            if (1..=2).contains(&composer_area.y) && scope != "filesystem" {
                assert!(buffer_text(&open).contains("Plugin00"));
            }
            if height == 32 {
                assert_eq!(
                    &open.content[..usize::from(width)],
                    &closed.content[..usize::from(width)]
                );
                let current_menu_row = (0..composer_area.y).find(|y| {
                    (0..width)
                        .map(|x| open[(x, *y)].symbol())
                        .collect::<String>()
                        .contains("Mentions")
                });
                assert!(current_menu_row.is_some());
                if let Some(previous) = menu_row {
                    assert_eq!(current_menu_row, Some(previous));
                }
                menu_row = current_menu_row;
                // The popup owns exactly its measured height, including both overflow rows.
                let ActivePopup::MentionV2(popup) = &composer.popups.active else {
                    panic!("expected unified mentions");
                };
                let first_popup_row = composer_area.y - popup.calculate_required_height(width);
                assert!(current_menu_row.is_some_and(|row| row >= first_popup_row));
                assert!((first_popup_row..composer_area.y).all(|y| {
                    !(0..width)
                        .map(|x| open[(x, y)].symbol())
                        .collect::<String>()
                        .contains("Transcript row")
                }));
            }
            if (width, height) == (80, 32) && scope == "all" {
                insta::assert_snapshot!("mention_scopes_overlay_history", buffer_text(&open));
            }
            composer.handle_key_event(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
        }
        composer.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert_eq!(
            render_overlay(&composer, size, &footer),
            (closed, composer_area, cursor)
        );
    }
}

#[test]
fn mention_menu_reserves_height_above_composer_and_owns_cursor_over_interactive_footer() {
    let composer = composer();
    let footer = TranscriptFooter {
        text: "Find: needle".into(),
        cursor_column: Some(6),
        is_interactive: true,
    };
    let options = composer.resolve_render_options(ComposerRenderOptions {
        footer: Some(&footer),
        ..Default::default()
    });
    assert!(options.footer.is_none());
    let width = 80;
    let area = Rect::new(
        /*x*/ 0,
        /*y*/ 0,
        width,
        composer.desired_height_with_options(width, options),
    );
    let mut buffer = Buffer::empty(area);
    composer.render_with_options(area, &mut buffer, /*mask_char*/ None, options);
    let ComposerLayout {
        composer: surface,
        textarea,
        popup,
        footer,
        ..
    } = composer.layout_with_options(area, options);
    assert_eq!((popup.bottom(), surface.bottom()), (surface.y, footer.y));
    assert_eq!(
        composer.cursor_pos_with_options(area, options),
        Some((3, textarea.y))
    );
    insta::assert_snapshot!("mention_menu_above_composer", buffer_text(&buffer));
}

#[test]
fn fallback_completions_keep_the_draft_cursor_and_footer_anchored() {
    let footer = TranscriptFooter {
        text: "TRANSCRIPT STATUS".into(),
        cursor_column: None,
        is_interactive: false,
    };
    for (width, height) in [(80, 24), (40, 16), (20, 5)] {
        for token in ["$", "@"] {
            let mut composer = composer();
            composer.popups.active = ActivePopup::None;
            composer.draft.textarea.set_text_clearing_elements(token);
            let size = Size::new(width, height);
            let (closed, composer_area, cursor) = render_overlay(&composer, size, &footer);
            composer.popups.active = if token == "$" {
                ActivePopup::Skill(SkillPopup::new(vec![MentionItem {
                    display_name: "Example skill".into(),
                    description: Some("Review this checkout".into()),
                    insert_text: "$example".into(),
                    search_terms: vec!["example".into()],
                    path: None,
                    category_tag: None,
                    sort_rank: 1,
                }]))
            } else {
                let mut popup = FileSearchPopup::new();
                popup.set_matches(
                    "",
                    vec![codex_file_search::FileMatch {
                        score: 1,
                        path: "src/example.rs".into(),
                        root: "/repo".into(),
                        match_type: codex_file_search::MatchType::File,
                        indices: None,
                    }],
                );
                ActivePopup::File(popup)
            };
            let (open, area, open_cursor) = render_overlay(&composer, size, &footer);
            assert_eq!((area, open_cursor), (composer_area, cursor));
            let start = open.index_of(area.x, area.y);
            assert_eq!(&open.content[start..], &closed.content[start..]);
            if height > 5 {
                assert!(buffer_text(&open).contains(if token == "$" {
                    "Example skill"
                } else {
                    "src/example.rs"
                }));
            }
        }
    }
}
