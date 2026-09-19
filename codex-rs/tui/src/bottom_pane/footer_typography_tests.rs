//! Legacy help keeps accent keys while ordinary footer keys clear inherited emphasis.

use super::*;
use pretty_assertions::assert_eq;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Style;

#[test]
fn help_reference_and_footer_preserve_distinct_key_styles() {
    crate::terminal_palette::with_test_default_colors(
        crate::terminal_probe::DefaultColors {
            fg: (32, 32, 32),
            bg: (255, 255, 255),
        },
        || {
            let help = shortcut_overlay_lines(ShortcutsState {
                use_shift_enter_hint: false,
                esc_backtrack_hint: false,
                is_task_running: false,
                queue_submissions: false,
                is_wsl: false,
                collaboration_modes_enabled: false,
                key_hints: FooterKeyHints::default_bindings(),
            });
            let help_height = help.len() as u16;
            let footer = footer_hint_items_line(&[
                ("ctrl+o".into(), "then".into()),
                ("o".into(), "copy".into()),
            ]);
            let mut terminal = Terminal::new(TestBackend::new(
                /*width*/ 80,
                /*height*/ help_height + 1,
            ))
            .unwrap();
            terminal
                .draw(|frame| {
                    let inherited = Style::default().fg(Color::Green).bold().dim();
                    frame.render_widget(
                        Paragraph::new(help).style(inherited),
                        Rect::new(/*x*/ 0, /*y*/ 0, /*width*/ 80, help_height),
                    );
                    frame.render_widget(
                        Paragraph::new(footer).style(inherited),
                        Rect::new(
                            /*x*/ 0,
                            help_height,
                            /*width*/ 80,
                            /*height*/ 1,
                        ),
                    );
                })
                .unwrap();
            let buffer = terminal.backend().buffer();
            let styles = [
                (0, 0),
                (2, 0),
                (0, help_height),
                (4, help_height),
                (7, help_height),
            ]
            .map(|position| {
                let cell = &buffer[position];
                (cell.fg, cell.modifier)
            });
            let key_color = key_hint::ctrl(KeyCode::Char('o')).spans()[0]
                .style
                .fg
                .unwrap();
            let secondary_color = secondary_text_style().fg.unwrap();
            assert_eq!(
                styles,
                [
                    (
                        crate::style::accent_color_on(/*background*/ None),
                        Modifier::empty()
                    ),
                    (secondary_color, Modifier::empty()),
                    (key_color, Modifier::BOLD),
                    (key_color, Modifier::BOLD),
                    (secondary_color, Modifier::empty()),
                ]
            );
        },
    );
}
