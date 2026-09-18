//! Exercises fixed model theme files through selection, preview, and cancellation.

use super::*;
use crate::app_event_sender::AppEventSender;
use crate::terminal_palette::with_test_default_colors;
use pretty_assertions::assert_eq;

#[test]
fn model_theme_files_preview_select_and_restore() {
    if std::env::var_os("CODEX_MODEL_THEME_TEST_CHILD").is_none() {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "theme_picker::model_tests::model_theme_files_preview_select_and_restore",
                "--nocapture",
            ])
            .env("CODEX_MODEL_THEME_TEST_CHILD", "1")
            .env("FORCE_COLOR", "3")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    let original = highlight::current_syntax_theme();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let tx = AppEventSender::new(tx);
    let mut snapshot = String::new();
    for name in ["ada", "babbage", "curie", "cushman", "dali", "davinci"] {
        let light = matches!(name, "babbage" | "cushman" | "davinci");
        let colors = crate::terminal_probe::DefaultColors {
            fg: if light { (32, 32, 32) } else { (224, 224, 224) },
            bg: if light { (255, 255, 255) } else { (24, 24, 24) },
        };
        with_test_default_colors(colors, || {
            let params = build_theme_picker_params(Some(name), /*codex_home*/ None, Some(120));
            let idx = params.initial_selected_idx.unwrap();
            assert_eq!(params.items[idx].search_value.as_deref(), Some(name));
            params.on_selection_changed.as_ref().unwrap()(idx, &tx);
            assert!(matches!(
                rx.try_recv().unwrap(),
                AppEvent::SyntaxThemePreviewed
            ));
            let code = "let answer = 42;\n";
            let lines = highlight::highlight_code_to_lines(code, "rust");
            assert!(lines.iter().all(|line| line.style.bg.is_none()
                && line.spans.iter().all(|span| span.style.bg.is_none())));
            let area = Rect::new(0, 0, 54, 4);
            let mut buffer = Buffer::empty(area);
            ThemePreviewNarrowRenderable.render(area, &mut buffer);
            if matches!(name, "dali" | "davinci") {
                assert!(
                    buffer
                        .content
                        .iter()
                        .all(|cell| cell.bg == ratatui::style::Color::Reset)
                );
            }
            snapshot.push_str(&format!(
                "{name}: accent={:?}\n",
                crate::style::accent_style()
            ));
            for y in 0..area.height {
                let row: String = (0..area.width).map(|x| buffer[(x, y)].symbol()).collect();
                snapshot.push_str(row.trim_end());
                snapshot.push('\n');
            }
            let styles = buffer
                .content
                .iter()
                .map(|cell| format!("{:?}", cell.style()))
                .collect::<std::collections::BTreeSet<_>>();
            snapshot.push_str(&format!("preview styles: {styles:?}\nsyntax: {lines:?}\n"));
            params.items[idx].actions[0](&tx);
            assert!(
                matches!(rx.try_recv().unwrap(), AppEvent::SyntaxThemeSelected { name: selected } if selected == name)
            );
            params.on_cancel.as_ref().unwrap()(&tx);
            assert!(matches!(
                rx.try_recv().unwrap(),
                AppEvent::SyntaxThemePreviewed
            ));
            assert_eq!(highlight::current_syntax_theme(), original);
        });
    }
    insta::assert_snapshot!(snapshot);
}
