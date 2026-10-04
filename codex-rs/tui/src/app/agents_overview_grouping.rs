//! Command Center grouping, including restoration and client-local preference persistence.
//! The view owns the live choice; ordered app events save it through the existing config writer.

use super::super::App;
use super::super::agents_overview::AgentsOverviewState;
use super::AgentsOverviewGrouping;
use super::AgentsOverviewView;
use crate::legacy_core::config::edit::ConfigEdit;
use crate::legacy_core::config::edit::ConfigEditsBuilder;
use codex_app_server_protocol::Thread;

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

impl AgentsOverviewState {
    pub(in super::super) fn new(grouping: codex_config::types::AgentsOverviewGrouping) -> Self {
        let state = Self::default();
        state
            .view_state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .grouping = grouping;
        state
    }
}

impl App {
    pub(in super::super) async fn persist_agents_overview_grouping(
        &mut self,
        grouping: AgentsOverviewGrouping,
    ) {
        self.local_settings.tui.agents_overview_grouping = grouping;
        let edit = ConfigEdit::SetPath {
            segments: vec!["tui".to_string(), "agents_overview_grouping".to_string()],
            value: grouping.as_str().into(),
        };
        if let Err(err) =
            ConfigEditsBuilder::for_config_path(self.local_settings.user_config_path.as_path())
                .with_edits([edit])
                .apply()
                .await
        {
            self.add_agents_overview_error(format!(
                "Failed to save Command Center grouping: {err}"
            ));
        }
    }
}
