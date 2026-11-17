#![cfg(test)]

use super::ShortcutGuideKeys;
use super::ShortcutView;
use super::lines;
use crate::bottom_pane::footer::FooterKeyHints;
use crate::bottom_pane::footer::FooterMode;
use crate::bottom_pane::footer::FooterProps;
use crate::bottom_pane::footer::footer_height;
use crate::bottom_pane::footer::render_footer_from_props;
use crate::key_hint;
use crate::key_hint::ShortcutHint;
use crate::ui_consts::FOOTER_INDENT_COLS;
use crossterm::event::KeyCode;
use pretty_assertions::assert_eq;
use ratatui::Terminal;
use ratatui::backend::TestBackend;

fn props() -> FooterProps {
    FooterProps {
        mode: FooterMode::ShortcutOverlay(Default::default()),
        esc_backtrack_hint: false,
        is_task_running: true,
        queue_submissions: false,
        collaboration_modes_enabled: true,
        is_wsl: false,
        quit_shortcut_key: key_hint::ctrl(KeyCode::Char('c')),
        status_line_value: None,
        status_line_enabled: false,
        key_hints: FooterKeyHints {
            agents: Some(key_hint::plain(KeyCode::Left).into()),
            ..FooterKeyHints::default_bindings()
        },
        active_agent_label: None,
    }
}

fn snapshot(name: &str, width: u16, height: Option<u16>, props: &FooterProps) {
    let inner_width = width - FOOTER_INDENT_COLS as u16;
    let measured_lines = lines(props, inner_width);
    assert!(
        measured_lines
            .iter()
            .all(|line| line.width() <= usize::from(inner_width))
    );
    let measured_height = footer_height(props, width);
    assert_eq!(usize::from(measured_height), measured_lines.len());
    let height = height.unwrap_or(measured_height);
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal
        .draw(|frame| {
            render_footer_from_props(
                frame.area(),
                frame.buffer_mut(),
                props,
                /*collaboration_mode_indicator*/ None,
                /*show_cycle_hint*/ false,
                /*show_shortcuts_hint*/ false,
                /*show_queue_hint*/ false,
            );
        })
        .unwrap();
    let painted_height = terminal
        .backend()
        .buffer()
        .content
        .chunks(usize::from(width))
        .rposition(|row| row.iter().any(|cell| cell.symbol() != " "))
        .map_or(/*default*/ 0, |row| row + 1);
    assert_eq!(painted_height, usize::from(height));
    insta::assert_snapshot!(name, terminal.backend());
}

#[test]
fn shortcut_overlay_uses_runtime_bindings_and_wsl_paste() {
    let mut props = props();
    props.is_task_running = false;
    props.is_wsl = true;
    props.esc_backtrack_hint = true;
    props.key_hints.external_editor = Some(ShortcutHint::Chord {
        prefix: key_hint::ctrl(KeyCode::Char('x')),
        completion: key_hint::ctrl(KeyCode::Char('e')),
    });
    props.key_hints.toggle_shortcuts = Some(ShortcutHint::Chord {
        prefix: key_hint::ctrl(KeyCode::Char('x')),
        completion: key_hint::plain(KeyCode::Char('h')),
    });
    props.key_hints.insert_newline = Some(key_hint::shift(KeyCode::Enter).into());
    props.key_hints.history_search = None;
    props.key_hints.reasoning_down = None;
    snapshot(
        "shortcut_overlay_customized_wsl",
        /*width*/ 80,
        /*height*/ None,
        &props,
    );
}

#[test]
fn shortcut_overlay_leaves_remapped_tab_and_function_keys_available() {
    let config = toml::from_str(
        r#"
        global.open_transcript = "tab"
        vim_search.forward = "f1"
        composer.queue = []
        "#,
    )
    .unwrap();
    let keymap = crate::keymap::RuntimeKeymap::from_config(&config).unwrap();
    let mut props = props();
    props.key_hints.shortcut_guide = ShortcutGuideKeys::from_keymap(&keymap);
    let mut view = ShortcutView::List;

    let keys = props.key_hints.shortcut_guide;
    assert_eq!(keys.view, Some(KeyCode::F(5)));
    assert!(!view.handle_key(KeyCode::Tab.into(), &props));
    assert!(!view.handle_key(KeyCode::F(1).into(), &props));
    assert!(view.handle_key(KeyCode::F(5).into(), &props));
    assert_eq!(view, ShortcutView::Keyboard);
    assert!(view.handle_key(KeyCode::F(5).into(), &props));
    assert_eq!(view, ShortcutView::List);
}

#[test]
fn shortcut_overlay_draws_base_keyboard_and_falls_back_to_the_list() {
    let mut props = props();
    let all = ratatui::text::Text::from(lines(&props, /*width*/ 80)).to_string();
    assert!(
        all.lines()
            .any(|row| row.contains("Paste image") && row.contains("⌃v"))
    );
    let mut view = ShortcutView::List;
    assert!(view.handle_key(KeyCode::Tab.into(), &props));
    props.mode = FooterMode::ShortcutOverlay(view);
    snapshot(
        "shortcut_keyboard_base",
        /*width*/ 80,
        /*height*/ None,
        &props,
    );
    snapshot(
        "shortcut_keyboard_roomy",
        /*width*/ 96,
        /*height*/ None,
        &props,
    );
    for (width, height) in [(68, usize::MAX), (80, 12)] {
        let fallback =
            ratatui::text::Text::from(super::guide_lines(&props, width, height)).to_string();
        assert!(fallback.contains("Paste image") && fallback.contains("Change mode"));
    }
}

#[test]
fn shortcut_keyboard_adapts_to_probed_and_unknown_terminal_colors() {
    let mut props = props();
    props.mode = FooterMode::ShortcutOverlay(ShortcutView::Keyboard);
    for colors in [
        Some(crate::terminal_probe::DefaultColors {
            fg: (230, 230, 230),
            bg: (20, 20, 20),
        }),
        Some(crate::terminal_probe::DefaultColors {
            fg: (20, 20, 20),
            bg: (245, 245, 245),
        }),
        None,
    ] {
        crate::terminal_palette::with_test_default_colors(colors, || {
            let rendered = lines(&props, /*width*/ 78);
            let panel = rendered[2].style.bg;
            let cap = rendered
                .iter()
                .flat_map(|line| &line.spans)
                .find(|span| span.content.trim() == "Q")
                .unwrap()
                .style;
            let bound = rendered
                .iter()
                .flat_map(|line| &line.spans)
                .find(|span| span.content.trim() == "Esc")
                .unwrap()
                .style;
            if colors.is_some() {
                assert_ne!(panel, cap.bg);
                assert_ne!(cap.bg, bound.bg);
                assert_ne!(panel, bound.bg);
            } else {
                assert_eq!(panel, Some(ratatui::style::Color::Reset));
                assert_eq!(cap.bg, panel);
                assert_ne!(cap, bound);
            }
        });
    }
}

#[test]
fn shortcut_keyboard_keeps_diagram_for_keys_outside_us_layout() {
    let mut props = props();
    props.key_hints.agents = Some(key_hint::plain(KeyCode::F(13)).into());
    props.key_hints.show_transcript = Some(key_hint::plain(KeyCode::Char('é')).into());
    let mut view = ShortcutView::List;
    assert!(view.handle_key(KeyCode::Tab.into(), &props));
    props.mode = FooterMode::ShortcutOverlay(view);
    snapshot(
        "shortcut_keyboard_off_diagram_keys",
        /*width*/ 80,
        /*height*/ None,
        &props,
    );

    // Keep the diagram within its original height on an 80-column terminal.
    assert_eq!(
        super::guide_lines(&props, /*width*/ 78, /*height*/ 17),
        lines(&props, /*width*/ 78),
    );

    // Switching back keeps off-diagram bindings discoverable with their actions.
    assert!(view.handle_key(KeyCode::Tab.into(), &props));
    props.mode = FooterMode::ShortcutOverlay(view);
    let list = ratatui::text::Text::from(lines(&props, /*width*/ 80)).to_string();
    assert!(
        list.lines()
            .any(|row| row.contains("f13") && row.contains("Agents"))
    );
    assert!(
        list.lines()
            .any(|row| row.contains('é') && row.contains("Open transcript"))
    );
}

#[test]
fn shortcut_overlay_clipping_preserves_customization_in_small_areas() {
    snapshot(
        "shortcut_overlay_customize_only",
        /*width*/ 28,
        /*height*/ Some(1),
        &props(),
    );
}
