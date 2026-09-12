//! Task grouping modes share model labels and group membership across rendering and navigation.

use super::AgentsOverviewView;
use codex_app_server_protocol::Thread;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(in super::super) enum AgentsOverviewGrouping {
    #[default]
    Project,
    Status,
    Model,
}

pub(super) fn model_name(thread: &Thread) -> &str {
    thread
        .model
        .as_deref()
        .filter(|model| !model.is_empty())
        .unwrap_or("Unknown")
}

impl AgentsOverviewView {
    pub(super) fn same_group(
        &self,
        grouping: AgentsOverviewGrouping,
        left: usize,
        right: usize,
    ) -> bool {
        match grouping {
            AgentsOverviewGrouping::Project => {
                self.project_groups[left].key == self.project_groups[right].key
            }
            AgentsOverviewGrouping::Status => self.rows[left].group == self.rows[right].group,
            AgentsOverviewGrouping::Model => {
                model_name(&self.rows[left].thread) == model_name(&self.rows[right].thread)
            }
        }
    }
}
