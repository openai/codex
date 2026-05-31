use std::fmt;
use std::sync::Arc;
use std::thread;

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::history_cell::HistoryCell;
use crate::history_cell::HistoryRenderMode;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TranscriptLayoutKey {
    pub(crate) generation: u64,
    pub(crate) width: u16,
    pub(crate) render_mode: HistoryRenderMode,
    pub(crate) cell_count: usize,
}

pub(crate) struct TranscriptLayoutResult {
    pub(crate) key: TranscriptLayoutKey,
    pub(crate) heights: Vec<usize>,
    pub(crate) total_height: usize,
}

impl fmt::Debug for TranscriptLayoutResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TranscriptLayoutResult")
            .field("key", &self.key)
            .field("height_count", &self.heights.len())
            .field("total_height", &self.total_height)
            .finish()
    }
}

pub(crate) fn spawn_transcript_layout_worker(
    key: TranscriptLayoutKey,
    cells: Vec<Arc<dyn HistoryCell>>,
    app_event_tx: AppEventSender,
) {
    thread::spawn(move || {
        let heights = measure_transcript_heights(&cells, key.width, key.render_mode);
        let total_height = heights.iter().copied().sum();
        app_event_tx.send(AppEvent::TranscriptLayoutReady(TranscriptLayoutResult {
            key,
            heights,
            total_height,
        }));
    });
}

fn measure_transcript_heights(
    cells: &[Arc<dyn HistoryCell>],
    width: u16,
    render_mode: HistoryRenderMode,
) -> Vec<usize> {
    let worker_count = layout_worker_count(cells.len());
    if worker_count <= 1 {
        return measure_transcript_height_range(cells, width, render_mode, /*start*/ 0);
    }

    let chunk_size = cells.len().div_ceil(worker_count);
    let mut chunks = thread::scope(|scope| {
        let mut handles = Vec::with_capacity(worker_count);
        for (chunk_idx, chunk) in cells.chunks(chunk_size).enumerate() {
            let start = chunk_idx.saturating_mul(chunk_size);
            handles.push(scope.spawn(move || {
                (
                    start,
                    measure_transcript_height_range(chunk, width, render_mode, start),
                )
            }));
        }
        handles
            .into_iter()
            .map(|handle| handle.join().unwrap_or_else(|_| (0, Vec::new())))
            .collect::<Vec<_>>()
    });
    chunks.sort_by_key(|(start, _)| *start);

    let mut heights = Vec::with_capacity(cells.len());
    for (_, chunk_heights) in chunks {
        heights.extend(chunk_heights);
    }
    heights
}

fn measure_transcript_height_range(
    cells: &[Arc<dyn HistoryCell>],
    width: u16,
    render_mode: HistoryRenderMode,
    start: usize,
) -> Vec<usize> {
    cells
        .iter()
        .enumerate()
        .map(|(offset, cell)| {
            let idx = start.saturating_add(offset);
            let spacing = usize::from(idx > 0 && !cell.is_stream_continuation());
            spacing.saturating_add(cell.desired_transcript_height_for_mode(width, render_mode) as usize)
        })
        .collect()
}

fn layout_worker_count(cell_count: usize) -> usize {
    if cell_count == 0 {
        return 1;
    }
    let parallelism = thread::available_parallelism().map_or(/*default*/ 1, usize::from);
    parallelism.min(/*other*/ 4).min(cell_count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::text::Line;

    #[derive(Debug)]
    struct TestCell {
        lines: Vec<Line<'static>>,
        stream_continuation: bool,
    }

    impl HistoryCell for TestCell {
        fn display_lines(&self, _width: u16) -> Vec<Line<'static>> {
            self.lines.clone()
        }

        fn raw_lines(&self) -> Vec<Line<'static>> {
            self.lines.clone()
        }

        fn is_stream_continuation(&self) -> bool {
            self.stream_continuation
        }
    }

    #[test]
    fn measures_transcript_heights_with_inter_cell_spacing() {
        let cells: Vec<Arc<dyn HistoryCell>> = vec![
            Arc::new(TestCell {
                lines: vec![Line::from("first")],
                stream_continuation: false,
            }),
            Arc::new(TestCell {
                lines: vec![Line::from("second")],
                stream_continuation: false,
            }),
            Arc::new(TestCell {
                lines: vec![Line::from("continuation")],
                stream_continuation: true,
            }),
        ];

        let heights =
            measure_transcript_heights(&cells, /*width*/ 80, HistoryRenderMode::Rich);

        assert_eq!(heights, vec![1, 2, 1]);
    }
}
