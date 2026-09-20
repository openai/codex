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
        mode: FooterMode::ShortcutOverlay,
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
fn shortcut_overlay_clipping_preserves_customization_in_small_areas() {
    snapshot(
        "shortcut_overlay_customize_only",
        /*width*/ 28,
        /*height*/ Some(1),
        &props(),
    );
}
