//! Independent status effects and master-switch precedence.

use super::*;
use crate::app_event::AppEvent;
use crate::terminal_palette::with_test_default_colors;
use crate::terminal_probe::DefaultColors;

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
