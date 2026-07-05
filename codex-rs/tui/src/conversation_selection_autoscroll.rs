//! Edge-driven vertical autoscroll state for conversation text selection.
//!
//! This module deliberately owns no clock or scrollable content. The viewport supplies elapsed
//! time and applies the returned row delta, which keeps animation tests deterministic and lets the
//! viewport stop the controller when it reaches a content boundary.

use std::time::Duration;

use ratatui::layout::Position;
use ratatui::layout::Rect;

const EDGE_THRESHOLD_ROWS: i64 = 3;
const ROWS_PER_SECOND_SLOW: i128 = 6;
const ROWS_PER_SECOND_MEDIUM: i128 = 36;
const ROWS_PER_SECOND_FAST: i128 = 72;
const NANOS_PER_SECOND: i128 = 1_000_000_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AutoscrollDirection {
    Up,
    Down,
}

impl AutoscrollDirection {
    fn sign(self) -> i128 {
        match self {
            Self::Up => -1,
            Self::Down => 1,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AutoscrollStep {
    pub(crate) direction: AutoscrollDirection,
    pub(crate) rows: usize,
    pub(crate) pointer: Position,
}

/// Accumulates edge-autoscroll motion while retaining the drag's raw terminal position.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct SelectionAutoscroll {
    pointer: Option<Position>,
    direction: Option<AutoscrollDirection>,
    rows_per_second: i128,
    fractional_row_nanos: i128,
}

impl SelectionAutoscroll {
    /// Updates the unclamped pointer used both for edge detection and later selection projection.
    pub(crate) fn update_pointer(&mut self, area: Rect, pointer: Position) {
        self.pointer = Some(pointer);
        let Some(direction) = vertical_direction(area, pointer) else {
            self.stop();
            return;
        };

        if self.direction.is_none() {
            self.fractional_row_nanos = 0;
        }
        self.direction = Some(direction);
        self.rows_per_second = rows_per_second(area, pointer);
    }

    /// Advances the controller and returns only complete rows of motion.
    pub(crate) fn advance(&mut self, elapsed: Duration) -> Option<AutoscrollStep> {
        let direction = self.direction?;
        let pointer = self.pointer?;
        let elapsed_nanos = i128::try_from(elapsed.as_nanos()).unwrap_or(i128::MAX);
        let row_nanos = elapsed_nanos
            .saturating_mul(self.rows_per_second)
            .saturating_mul(direction.sign());
        self.fractional_row_nanos = self.fractional_row_nanos.saturating_add(row_nanos);

        let whole_rows = self.fractional_row_nanos / NANOS_PER_SECOND;
        if whole_rows == 0 {
            return None;
        }
        self.fractional_row_nanos = self
            .fractional_row_nanos
            .saturating_sub(whole_rows.saturating_mul(NANOS_PER_SECOND));

        let direction = if whole_rows.is_negative() {
            AutoscrollDirection::Up
        } else {
            AutoscrollDirection::Down
        };
        let rows = usize::try_from(whole_rows.unsigned_abs()).unwrap_or(usize::MAX);
        Some(AutoscrollStep {
            direction,
            rows,
            pointer,
        })
    }

    pub(crate) fn needs_frame(&self) -> bool {
        self.direction.is_some()
    }

    pub(crate) fn pointer(&self) -> Option<Position> {
        self.pointer
    }

    /// Stops motion and discards fractional progress while preserving the last raw pointer.
    pub(crate) fn stop(&mut self) {
        self.direction = None;
        self.rows_per_second = 0;
        self.fractional_row_nanos = 0;
    }

    /// Clears the gesture entirely so a later unrelated draw cannot re-arm a blocked edge.
    pub(crate) fn reset(&mut self) {
        self.stop();
        self.pointer = None;
    }
}

fn vertical_direction(area: Rect, pointer: Position) -> Option<AutoscrollDirection> {
    if area.height == 0 {
        return None;
    }

    let relative_y = i64::from(pointer.y) - i64::from(area.y);
    let distance_to_top = relative_y;
    let distance_to_bottom = i64::from(area.height) - relative_y;
    match (
        distance_to_top <= EDGE_THRESHOLD_ROWS,
        distance_to_bottom <= EDGE_THRESHOLD_ROWS,
    ) {
        (true, true) if distance_to_bottom < distance_to_top => Some(AutoscrollDirection::Down),
        (true, _) => Some(AutoscrollDirection::Up),
        (_, true) => Some(AutoscrollDirection::Down),
        (false, false) => None,
    }
}

fn rows_per_second(area: Rect, pointer: Position) -> i128 {
    let relative_x = i64::from(pointer.x) - i64::from(area.x);
    let relative_y = i64::from(pointer.y) - i64::from(area.y);
    let min_distance = relative_x
        .min(i64::from(area.width) - relative_x)
        .min(relative_y)
        .min(i64::from(area.height) - relative_y);

    if min_distance <= 1 {
        ROWS_PER_SECOND_FAST
    } else if min_distance <= 2 {
        ROWS_PER_SECOND_MEDIUM
    } else {
        ROWS_PER_SECOND_SLOW
    }
}

#[cfg(test)]
#[path = "conversation_selection_autoscroll_tests.rs"]
mod tests;
