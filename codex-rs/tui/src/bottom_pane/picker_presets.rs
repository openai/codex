//! Shared presentation defaults for production selection menus.
//!
//! Callers retain their actions, filtering, previews, and cancellation policies.

use super::ColumnWidthMode;
use super::PickerSurface;
use super::SelectionDescriptionLayout;
use super::SelectionViewParams;

impl SelectionViewParams {
    /// Start a panel picker with stable columns, hiding descriptions when they become too narrow.
    /// Titles and subtitles wrap; standard hints follow the active list keymap.
    pub(crate) fn picker() -> Self {
        Self {
            picker_surface: PickerSurface::Panel,
            col_width_mode: ColumnWidthMode::AutoAllRows,
            description_layout: SelectionDescriptionLayout::HideWhenNarrow {
                min_description_width: 24,
            },
            ..Self::default()
        }
    }
}
