//! Layout and pointer interaction state for application-owned conversation panes.

use ratatui::layout::Position;
use ratatui::layout::Rect;

use crate::app_event::PaneSlot;
use crate::tui::MousePrimaryEvent;
use crate::tui::MousePrimaryEventKind;

const MIN_SPLIT_PANE_WIDTH: u16 = 41;
const SPLIT_DIVIDER_WIDTH: u16 = 1;
const MIN_SPLIT_WIDTH: u16 = MIN_SPLIT_PANE_WIDTH * 2 + SPLIT_DIVIDER_WIDTH;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct PaneSplitPreference {
    parent_width: u16,
    available_width: u16,
}

impl Default for PaneSplitPreference {
    fn default() -> Self {
        Self {
            parent_width: 1,
            available_width: 2,
        }
    }
}

impl PaneSplitPreference {
    fn parent_width(self, available_width: u16) -> u16 {
        let denominator = u32::from(self.available_width.max(/*other*/ 1));
        let numerator = u32::from(self.parent_width) * u32::from(available_width);
        ((numerator + denominator / 2) / denominator) as u16
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum OwnedScreenLayout {
    Single {
        slot: PaneSlot,
        area: Rect,
        show_header: bool,
    },
    Split {
        area: Rect,
        parent: Rect,
        divider: Rect,
        side: Rect,
    },
}

impl OwnedScreenLayout {
    pub(super) fn minimum_width(has_side: bool) -> u16 {
        if has_side {
            MIN_SPLIT_WIDTH
        } else {
            MIN_SPLIT_PANE_WIDTH
        }
    }

    pub(super) fn new(
        area: Rect,
        has_side: bool,
        focused: PaneSlot,
        preference: PaneSplitPreference,
    ) -> Self {
        if !has_side {
            return Self::Single {
                slot: PaneSlot::Parent,
                area,
                show_header: false,
            };
        }
        if area.width < MIN_SPLIT_WIDTH {
            return Self::Single {
                slot: focused,
                area,
                show_header: true,
            };
        }

        let available_width = area.width.saturating_sub(SPLIT_DIVIDER_WIDTH);
        let max_parent_width = available_width.saturating_sub(MIN_SPLIT_PANE_WIDTH);
        let parent_width = preference
            .parent_width(available_width)
            .clamp(MIN_SPLIT_PANE_WIDTH, max_parent_width);
        let side_width = available_width.saturating_sub(parent_width);
        let parent = Rect::new(area.x, area.y, parent_width, area.height);
        let divider = Rect::new(parent.right(), area.y, SPLIT_DIVIDER_WIDTH, area.height);
        let side = Rect::new(divider.right(), area.y, side_width, area.height);
        Self::Split {
            area,
            parent,
            divider,
            side,
        }
    }

    pub(super) fn area(self) -> Rect {
        match self {
            Self::Single { area, .. } | Self::Split { area, .. } => area,
        }
    }

    fn split_geometry(self) -> Option<SplitGeometry> {
        match self {
            Self::Single { .. } => None,
            Self::Split { area, divider, .. } => Some(SplitGeometry { area, divider }),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SplitGeometry {
    area: Rect,
    divider: Rect,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum SplitInteraction {
    #[default]
    Idle,
    Dragging {
        origin_column: u16,
        initial_preference: PaneSplitPreference,
    },
}

#[derive(Debug, Default)]
pub(super) struct OwnedScreenSplitState {
    preference: PaneSplitPreference,
    interaction: SplitInteraction,
    geometry: Option<SplitGeometry>,
}

impl OwnedScreenSplitState {
    pub(super) fn preference(&self) -> PaneSplitPreference {
        self.preference
    }

    pub(super) fn record_layout(&mut self, layout: OwnedScreenLayout) {
        let next_geometry = layout.split_geometry();
        let area_changed = self
            .geometry
            .zip(next_geometry)
            .is_some_and(|(current, next)| current.area != next.area);
        if next_geometry.is_none() || area_changed {
            self.interaction = SplitInteraction::Idle;
        }
        self.geometry = next_geometry;
    }

    pub(super) fn clear_rendered_layout(&mut self) {
        self.interaction = SplitInteraction::Idle;
        self.geometry = None;
    }

    pub(super) fn is_dragging(&self) -> bool {
        matches!(self.interaction, SplitInteraction::Dragging { .. })
    }

    pub(super) fn cancel_drag(&mut self) -> bool {
        let was_dragging = self.is_dragging();
        self.interaction = SplitInteraction::Idle;
        was_dragging
    }

    pub(super) fn handle_mouse(&mut self, event: MousePrimaryEvent) -> bool {
        match event.kind {
            MousePrimaryEventKind::Press => {
                self.interaction = SplitInteraction::Idle;
                let position = Position::new(event.column, event.row);
                if self.geometry.is_some_and(|geometry| {
                    geometry.area.width > MIN_SPLIT_WIDTH && geometry.divider.contains(position)
                }) {
                    self.interaction = SplitInteraction::Dragging {
                        origin_column: event.column,
                        initial_preference: self.preference,
                    };
                    true
                } else {
                    false
                }
            }
            MousePrimaryEventKind::Drag if self.is_dragging() => {
                self.update_preference(event.column);
                true
            }
            MousePrimaryEventKind::Release if self.is_dragging() => {
                let SplitInteraction::Dragging {
                    origin_column,
                    initial_preference,
                } = self.interaction
                else {
                    return false;
                };
                if event.column == origin_column {
                    self.preference = initial_preference;
                } else {
                    self.update_preference(event.column);
                }
                self.interaction = SplitInteraction::Idle;
                true
            }
            MousePrimaryEventKind::Drag | MousePrimaryEventKind::Release => false,
        }
    }

    fn update_preference(&mut self, column: u16) {
        let Some(geometry) = self.geometry else {
            return;
        };
        let available_width = geometry.area.width.saturating_sub(SPLIT_DIVIDER_WIDTH);
        let max_parent_width = available_width.saturating_sub(MIN_SPLIT_PANE_WIDTH);
        let parent_width = column
            .saturating_sub(geometry.area.x)
            .clamp(MIN_SPLIT_PANE_WIDTH, max_parent_width);
        self.preference = PaneSplitPreference {
            parent_width,
            available_width,
        };
    }
}

#[cfg(test)]
#[path = "owned_screen_resize_tests.rs"]
mod tests;
