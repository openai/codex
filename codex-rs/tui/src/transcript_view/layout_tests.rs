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
        cache.begin_frame();
        cache.get(&history, /*width*/ 20, /*separated*/ false);
        cache.get(&history, /*width*/ 20, /*separated*/ false);
        assert_eq!(cell.renders.load(Ordering::Relaxed), 1);
        cache.begin_frame();
        cache.get(&history, /*width*/ 20, /*separated*/ false);
        assert_eq!(
            cell.renders.load(Ordering::Relaxed),
            1 + usize::from(mutable)
        );
        cell.tick.store(/*val*/ 2, Ordering::Relaxed);
        cache.begin_frame();
        let next = cache.get(&history, /*width*/ 20, /*separated*/ false);
        assert_eq!(next.rows[0].line.line.to_string(), "tick 2");
        cache.get(&history, /*width*/ 10, /*separated*/ false);
        assert_eq!(
            cell.renders.load(Ordering::Relaxed),
            3 + usize::from(mutable)
        );
        crate::terminal_palette::with_test_default_colors(
            crate::terminal_probe::DefaultColors {
                fg: (12, 34, 56),
                bg: (65, 43, 21),
            },
            || {
                cache.begin_frame();
                cache.get(&history, /*width*/ 10, /*separated*/ false);
                assert_eq!(
                    cell.renders.load(Ordering::Relaxed),
                    4 + usize::from(mutable)
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
        cache.get(
            &(cell.clone() as Arc<dyn HistoryCell>),
            /*width*/ 20,
            /*separated*/ false,
        );
    }
    for index in [99, 98, 0] {
        cache.get(
            &(cells[index].clone() as Arc<dyn HistoryCell>),
            /*width*/ 20,
            /*separated*/ false,
        );
    }
    assert_eq!(
        [99, 98, 0].map(|index| cells[index].renders.load(Ordering::Relaxed)),
        [1, 1, 2]
    );
}
