//! Layout for the agent list and its optional search or rename field.

use super::*;

impl AgentsOverviewView {
    pub(super) fn layout_areas(&self, area: Rect) -> [Rect; 6] {
        let metadata_height = u16::from(self.state().editing_metadata());
        let footer_height = (self.footer_lines(area.width.saturating_sub(4)).len() as u16)
            .min(area.height.saturating_sub(6 + metadata_height));
        let header_height = self
            .state()
            .server_version_notice
            .as_deref()
            .map(|notice| {
                textwrap::wrap(notice, usize::from(area.width.saturating_sub(4).max(1))).len()
                    as u16
            })
            .unwrap_or(1)
            .min(
                area.height
                    .saturating_sub(footer_height + metadata_height + 3)
                    .max(1),
            );
        Layout::vertical([
            Constraint::Length(header_height),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(metadata_height),
            Constraint::Length(footer_height),
        ])
        .areas(area)
    }
}
