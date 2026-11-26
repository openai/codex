//! Independent status effects and master-switch precedence.

use super::*;
use crate::app_event::AppEvent;
use crate::terminal_palette::with_test_default_colors;
use crate::terminal_probe::DefaultColors;

impl StatusIndicatorWidget {
    pub(crate) fn render_heading_at(&mut self, now: Instant) -> String {
        with_test_default_colors(
            DefaultColors {
                fg: (240, 240, 240),
                bg: (16, 16, 16),
            },
            || {
                self.prepare_frame(now);
                Line::from(self.header_spans(now, /*columns*/ 80)).to_string()
            },
        )
    }
}

#[test]
fn hook_status_reflows_at_the_prepared_frame() {
    with_test_default_colors(
        DefaultColors {
            fg: (240, 240, 240),
            bg: (16, 16, 16),
        },
        || {
            let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
            let mut row = StatusIndicatorWidget::new(
                AppEventSender::new(tx),
                FrameRequester::test_dummy(),
                /*animations_enabled*/ true,
                codex_config::types::TuiEffects {
                    progress: false,
                    ..Default::default()
                },
            );
            row.set_interrupt_hint_visible(/*visible*/ false);
            row.update_hook_status_message(Some("running hook".into()));

            // The wall clock has crossed the fade boundary before either draw.
            let start = Instant::now() - Duration::from_secs(/*secs*/ 1);
            let timer = StatusTimer {
                last_resume_at: start,
                ..Default::default()
            };
            row.prepare_frame(start);
            row.update_header(
                StatusHeader::from_summary(
                    /*item_id*/ None,
                    /*part*/ 0,
                    "Checking dependencies",
                )
                .unwrap(),
            );
            row.prepare_frame(start);
            let width = 30;
            let frames = [(99, 1), (100, 2), (1000, 2)].map(|(ms, expected_height)| {
                row.prepare_frame(start + Duration::from_millis(ms));
                let height = row.with_timer(&timer).desired_height(width);
                pretty_assertions::assert_eq!(height, expected_height, "at {ms} ms");
                let mut terminal =
                    ratatui::Terminal::new(ratatui::backend::TestBackend::new(width, height))
                        .unwrap();
                terminal
                    .draw(|f| row.with_timer(&timer).render(f.area(), f.buffer_mut()))
                    .unwrap();
                format!("{ms} ms\n{}", terminal.backend())
            });
            insta::assert_snapshot!(frames.join("\n"));
        },
    );
}

#[test]
fn shimmer_and_progress_are_independent_and_obey_master_switch() {
    with_test_default_colors(
        DefaultColors {
            fg: (240, 240, 240),
            bg: (16, 16, 16),
        },
        || {
            let mut snapshots = Vec::new();
            for (animations, shimmer, progress) in [
                (true, false, false),
                (true, false, true),
                (true, true, false),
                (false, true, true),
            ] {
                let (tx, _rx) = tokio::sync::mpsc::unbounded_channel::<AppEvent>();
                let mut row = StatusIndicatorWidget::new(
                    AppEventSender::new(tx),
                    FrameRequester::test_dummy(),
                    animations,
                    codex_config::types::TuiEffects {
                        shimmer,
                        progress,
                        ..Default::default()
                    },
                );
                // Pin the label timing and fallback bullet glyph to the first frame.
                let start = Instant::now() + Duration::from_secs(/*secs*/ 3600);
                row.header_started_at = start;
                let timer = StatusTimer {
                    last_resume_at: start,
                    ..Default::default()
                };
                let lines = StatusIndicator {
                    row: &row,
                    timer: &timer,
                }
                .lines(/*width*/ 80);
                let header = &lines[0];
                // Omit the bullet's process-clock-driven styling from the snapshot.
                let label_start = if animations && progress { 2 } else { 0 };
                let label = &header.spans[label_start..];
                snapshots.push(format!(
                    "animations={animations}, shimmer={shimmer}, progress={progress}\n{header}\n{label:?}"
                ));
            }
            insta::assert_snapshot!(snapshots.join("\n\n"));
        },
    );
}
