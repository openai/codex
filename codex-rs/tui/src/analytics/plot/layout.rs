//! Calendar viewport and shared coordinates for bars, axes, and annotations.

use super::painting::Band;

#[derive(Clone, Copy)]
pub(super) struct Layout {
    pub(super) width: usize,
    pub(super) height: usize,
    pub(super) axis_width: usize,
    pub(super) plot_width: usize,
    pub(super) count: usize,
    pub(super) start: usize,
    pub(super) baseline: usize,
    pub(super) marker_row: usize,
}

impl Layout {
    pub(super) fn new(
        day_count: usize,
        cursor: usize,
        width: usize,
        height: usize,
        label_width: usize,
        bands: &[Band],
    ) -> Self {
        // Hide the axis when a complete label would crowd out the bars.
        let axis_width = if width >= 30 && label_width <= width / 3 {
            label_width
        } else {
            0
        };
        let plot_width = width - axis_width;
        let count = day_count.min(plot_width);
        let start = cursor.saturating_sub(count - 1).min(day_count - count);
        let baseline = height + 1;
        let marker_row = baseline + if bands.len() == 2 { height + 2 } else { 1 };
        Self {
            width,
            height,
            axis_width,
            plot_width,
            count,
            start,
            baseline,
            marker_row,
        }
    }

    pub(super) fn center(self, index: usize) -> usize {
        self.axis_width
            + (index * self.plot_width / self.count + (index + 1) * self.plot_width / self.count
                - 1)
                / 2
    }
}
