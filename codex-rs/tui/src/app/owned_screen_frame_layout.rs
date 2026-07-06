//! Responsive geometry resolution for frame-owned side panels.

use super::super::owned_screen_resize::OwnedScreenLayout;
use super::*;
use ratatui::layout::Rect;

impl OwnedScreenFrameState {
    pub(in super::super) fn layout(
        &mut self,
        area: Rect,
        has_side: bool,
    ) -> OwnedScreenFrameLayout {
        self.resolve_layout(area, has_side, /*overlays_enabled*/ true)
    }

    pub(in super::super) fn layout_without_overlay(
        &mut self,
        area: Rect,
        has_side: bool,
    ) -> OwnedScreenFrameLayout {
        self.resolve_layout(area, has_side, /*overlays_enabled*/ false)
    }

    fn resolve_layout(
        &mut self,
        area: Rect,
        has_side: bool,
        overlays_enabled: bool,
    ) -> OwnedScreenFrameLayout {
        if self.layout.is_some_and(|layout| layout.area != area) {
            self.interaction = FrameInteraction::Idle;
        }
        let hard_center_width = OwnedScreenLayout::minimum_width(has_side);
        let preferred_center_width = PREFERRED_CENTER_WIDTH.max(hard_center_width);
        let mut remaining_width = area.width;
        let mut sidebar_docked = false;
        let mut summary_docked = false;
        let first_shown = self.last_activated_panel;
        let shown_order = match first_shown {
            Some(OwnedScreenPanel::Summary) => {
                [OwnedScreenPanel::Summary, OwnedScreenPanel::Sidebar]
            }
            Some(OwnedScreenPanel::Sidebar) | None => {
                [OwnedScreenPanel::Sidebar, OwnedScreenPanel::Summary]
            }
        };
        for panel in shown_order {
            if self.preference(panel) != OwnedScreenPanelPreference::Shown {
                continue;
            }
            let panel_width = self.panel_state(panel).width + PANEL_DIVIDER_WIDTH;
            if remaining_width >= hard_center_width.saturating_add(panel_width) {
                match panel {
                    OwnedScreenPanel::Sidebar => sidebar_docked = true,
                    OwnedScreenPanel::Summary => summary_docked = true,
                }
                remaining_width = remaining_width.saturating_sub(panel_width);
            }
        }
        for panel in [OwnedScreenPanel::Sidebar, OwnedScreenPanel::Summary] {
            if self.preference(panel) != OwnedScreenPanelPreference::Auto {
                continue;
            }
            let panel_width = self.panel_state(panel).width + PANEL_DIVIDER_WIDTH;
            if remaining_width >= preferred_center_width.saturating_add(panel_width) {
                match panel {
                    OwnedScreenPanel::Sidebar => sidebar_docked = true,
                    OwnedScreenPanel::Summary => summary_docked = true,
                }
                remaining_width = remaining_width.saturating_sub(panel_width);
            }
        }
        let mut center = area;
        let (sidebar, sidebar_divider) = if sidebar_docked {
            let panel_area = Rect::new(center.x, center.y, self.sidebar.width, center.height);
            let divider = Rect::new(
                panel_area.right(),
                center.y,
                PANEL_DIVIDER_WIDTH,
                center.height,
            );
            center.x = divider.right();
            center.width = center
                .width
                .saturating_sub(self.sidebar.width + PANEL_DIVIDER_WIDTH);
            (
                Some(OwnedScreenPanelLayout {
                    area: panel_area,
                    presentation: OwnedScreenPanelPresentation::Docked,
                }),
                Some(divider),
            )
        } else {
            (None, None)
        };

        let (summary, summary_divider) = if summary_docked {
            let divider = Rect::new(
                center
                    .right()
                    .saturating_sub(self.summary.width + PANEL_DIVIDER_WIDTH),
                center.y,
                PANEL_DIVIDER_WIDTH,
                center.height,
            );
            let panel_area =
                Rect::new(divider.right(), center.y, self.summary.width, center.height);
            center.width = center
                .width
                .saturating_sub(self.summary.width + PANEL_DIVIDER_WIDTH);
            (
                Some(OwnedScreenPanelLayout {
                    area: panel_area,
                    presentation: OwnedScreenPanelPresentation::Docked,
                }),
                Some(divider),
            )
        } else {
            (None, None)
        };

        let mut layout = OwnedScreenFrameLayout {
            area,
            center,
            sidebar,
            sidebar_divider,
            summary,
            summary_divider,
        };
        let overlay = if overlays_enabled {
            self.last_activated_panel
                .filter(|panel| self.preference(*panel) == OwnedScreenPanelPreference::Shown)
                .filter(|panel| layout.panel(*panel).is_none())
                .or_else(|| {
                    [OwnedScreenPanel::Sidebar, OwnedScreenPanel::Summary]
                        .into_iter()
                        .find(|panel| {
                            self.preference(*panel) == OwnedScreenPanelPreference::Shown
                                && layout.panel(*panel).is_none()
                        })
                })
        } else {
            None
        };
        if let Some(panel) = overlay {
            let panel_layout = OwnedScreenPanelLayout {
                area: overlay_area(area, panel, self.panel_state(panel).width),
                presentation: OwnedScreenPanelPresentation::Overlay,
            };
            match panel {
                OwnedScreenPanel::Sidebar => layout.sidebar = Some(panel_layout),
                OwnedScreenPanel::Summary => layout.summary = Some(panel_layout),
            }
            self.focus = panel.into();
        }
        if self
            .focus
            .panel()
            .is_some_and(|panel| layout.panel(panel).is_none())
        {
            self.focus = OwnedScreenFrameFocus::Conversation;
        }
        self.layout = Some(layout);
        layout
    }
}

fn overlay_area(area: Rect, panel: OwnedScreenPanel, preferred_width: u16) -> Rect {
    let horizontal_margin = u16::from(area.width > 4) * 2;
    let width = preferred_width.min(area.width.saturating_sub(horizontal_margin));
    let x = match panel {
        OwnedScreenPanel::Sidebar => area.x.saturating_add(horizontal_margin / 2),
        OwnedScreenPanel::Summary => area.right().saturating_sub(width + horizontal_margin / 2),
    };
    Rect::new(x, area.y, width, area.height)
}
