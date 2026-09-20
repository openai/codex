//! Status filters and viewport state survive refreshes of the shared task projection.

mod hints;
mod input;
mod navigation;
mod render;
mod rows;

use super::*;

// Counts and filtering use the same status groups.
pub(super) const TASK_FILTERS: &[(&str, Option<AgentsOverviewGroup>)] = &[
    ("All", None),
    ("Needs you", Some(AgentsOverviewGroup::NeedsYou)),
    ("Working", Some(AgentsOverviewGroup::Working)),
    ("Ready", Some(AgentsOverviewGroup::Ready)),
    ("Inactive", Some(AgentsOverviewGroup::Finished)),
];

impl AgentsOverviewView {
    pub(in crate::app::agents_overview_view) fn reconcile_command_center_selection(&mut self) {
        let visible = self.visible_indices();
        if !visible.contains(&self.selected) {
            self.selected = visible.first().copied().unwrap_or(usize::MAX);
        }
    }
}
