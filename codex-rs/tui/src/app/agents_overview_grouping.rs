//! Command Center grouping, including restoration and client-local preference persistence.
//! Directory is view-local so selecting it never writes a value that older clients reject.
//! Saved paths are sufficient for navigation while local workspace identities resolve.

use super::super::App;
use super::super::agents_overview::AgentsOverviewState;
use super::super::agents_overview_discovery::matches_query;
use super::AgentsOverviewProjectGroup;
use super::AgentsOverviewRow;
use super::AgentsOverviewView;
use super::AgentsOverviewViewState;
use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::legacy_core::config::edit::ConfigEdit;
use crate::legacy_core::config::edit::ConfigEditsBuilder;
use crate::thread_label::ThreadLabelStyle;
use codex_app_server_protocol::Thread;
use std::collections::HashMap;
use std::collections::VecDeque;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::PoisonError;

type WorkspaceGroup = (AgentsOverviewProjectGroup, PathBuf);

fn workspace_group(cwd: &Path, worktrees_enabled: bool) -> WorkspaceGroup {
    (
        AgentsOverviewProjectGroup::for_cwd(cwd, worktrees_enabled),
        super::directory_group_key(cwd, worktrees_enabled),
    )
}

#[derive(Default)]
pub(super) struct WorkspaceGroups {
    /// None while queued or in flight. Each saved path is probed once per opening.
    by_cwd: HashMap<PathBuf, Option<WorkspaceGroup>>,
    queue: VecDeque<PathBuf>,
    workers: usize,
    repaint_pending: bool,
}

impl WorkspaceGroups {
    pub(super) fn for_rows(
        state: &Arc<Mutex<AgentsOverviewViewState>>,
        rows: &[AgentsOverviewRow],
        worktrees_enabled: bool,
        events: &AppEventSender,
    ) -> (Vec<AgentsOverviewProjectGroup>, Vec<PathBuf>) {
        let mut guard = state.lock().unwrap_or_else(PoisonError::into_inner);
        let groups = &mut guard.workspace_groups;
        groups.repaint_pending = false;
        let snapshot = rows
            .iter()
            .map(|row| {
                let cwd = row.thread.cwd.as_path();
                if worktrees_enabled && row.label_style != ThreadLabelStyle::Cloud {
                    let resolved = groups.by_cwd.entry(cwd.to_owned()).or_insert_with(|| {
                        groups.queue.push_back(cwd.to_owned());
                        None
                    });
                    if let Some(group) = resolved {
                        return group.clone();
                    }
                }
                workspace_group(cwd, /*worktrees_enabled*/ false)
            })
            .unzip();

        // A mount can block indefinitely. Bound the number of detached workers and
        // keep these calls off Tokio's blocking pool, which waits for them at shutdown.
        const MAX_WORKERS: usize = 4;
        for _ in 0..groups.queue.len().min(MAX_WORKERS - groups.workers) {
            let state = Arc::downgrade(state);
            let events = events.clone();
            let worker = std::thread::Builder::new().spawn(move || {
                while let Some(state) = state.upgrade() {
                    let cwd = {
                        let mut guard = state.lock().unwrap_or_else(PoisonError::into_inner);
                        let groups = &mut guard.workspace_groups;
                        let Some(cwd) = groups.queue.pop_front() else {
                            groups.workers -= 1;
                            return;
                        };
                        cwd
                    };
                    let group = workspace_group(&cwd, /*worktrees_enabled*/ true);
                    let changed = group != workspace_group(&cwd, /*worktrees_enabled*/ false);
                    let mut guard = state.lock().unwrap_or_else(PoisonError::into_inner);
                    let groups = &mut guard.workspace_groups;
                    groups.by_cwd.insert(cwd, Some(group));
                    if changed && !groups.repaint_pending {
                        groups.repaint_pending = true;
                        drop(guard);
                        events.send(AppEvent::AgentsOverviewGroupsUpdated);
                    }
                }
            });
            if worker.is_err() {
                break;
            }
            groups.workers += 1;
        }
        snapshot
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub(in crate::app) enum AgentsOverviewGrouping {
    #[default]
    Project,
    Directory,
    Status,
    Model,
}

impl From<codex_config::types::AgentsOverviewGrouping> for AgentsOverviewGrouping {
    fn from(grouping: codex_config::types::AgentsOverviewGrouping) -> Self {
        match grouping {
            codex_config::types::AgentsOverviewGrouping::Project => Self::Project,
            codex_config::types::AgentsOverviewGrouping::Status => Self::Status,
            codex_config::types::AgentsOverviewGrouping::Model => Self::Model,
        }
    }
}

impl AgentsOverviewGrouping {
    pub(super) fn persisted(self) -> Option<codex_config::types::AgentsOverviewGrouping> {
        match self {
            Self::Project => Some(codex_config::types::AgentsOverviewGrouping::Project),
            Self::Directory => None,
            Self::Status => Some(codex_config::types::AgentsOverviewGrouping::Status),
            Self::Model => Some(codex_config::types::AgentsOverviewGrouping::Model),
        }
    }
}

pub(super) fn model_name(thread: &Thread) -> &str {
    thread
        .model
        .as_deref()
        .filter(|model| !model.is_empty())
        .unwrap_or("Unknown")
}

impl AgentsOverviewView {
    pub(super) fn is_pinned(&self, index: usize) -> bool {
        self.pinned_thread_ranks
            .as_ref()
            .is_some_and(|ranks| ranks.contains_key(&self.rows[index].thread_id))
    }

    pub(in crate::app) fn visible_indices(&self) -> Vec<usize> {
        let state = self.state();
        let search = state.search.to_lowercase();
        let (_, status_group) = super::command_center::TASK_FILTERS[state.status_filter];
        let (mut pinned, mut visible): (Vec<_>, Vec<_>) = self
            .rows
            .iter()
            .enumerate()
            .filter_map(|(index, row)| {
                (state.rename_target == Some(row.thread_id)
                    || (state.includes_row(row)
                        && (search.is_empty() || matches_query(&row.thread, &search))
                        && status_group.is_none_or(|group| group == row.group)))
                .then_some(index)
            })
            .partition(|index| self.is_pinned(*index));
        if let Some(ranks) = &self.pinned_thread_ranks {
            pinned.sort_by_key(|index| ranks.get(&self.rows[*index].thread_id).copied());
        }
        let date_key = |index: usize| {
            let thread = &self.rows[index].thread;
            std::cmp::Reverse(if state.searching {
                state.search_sort.timestamp(thread)
            } else {
                thread.updated_at
            })
        };
        match state.grouping {
            AgentsOverviewGrouping::Project => {
                visible.sort_by_key(|index| (&self.project_groups[*index].key, date_key(*index)))
            }
            AgentsOverviewGrouping::Directory => {
                visible.sort_by_key(|index| (&self.directory_group_keys[*index], date_key(*index)))
            }
            AgentsOverviewGrouping::Status => {
                if state.searching {
                    visible.sort_by_key(|index| date_key(*index));
                }
                pinned.extend(visible);
                // Stable grouping preserves pin order, then the supplied newest-first order.
                pinned.sort_by_key(|index| self.rows[*index].group);
                return pinned;
            }
            AgentsOverviewGrouping::Model => visible
                .sort_by_key(|index| (model_name(&self.rows[*index].thread), date_key(*index))),
        }
        pinned.extend(visible);
        pinned
    }

    pub(super) fn same_group(
        &self,
        grouping: AgentsOverviewGrouping,
        left: usize,
        right: usize,
    ) -> bool {
        match grouping {
            AgentsOverviewGrouping::Status => self.rows[left].group == self.rows[right].group,
            _ if self.is_pinned(left) || self.is_pinned(right) => {
                self.is_pinned(left) && self.is_pinned(right)
            }
            AgentsOverviewGrouping::Project => {
                self.project_groups[left].key == self.project_groups[right].key
            }
            AgentsOverviewGrouping::Directory => {
                self.directory_group_keys[left] == self.directory_group_keys[right]
            }
            AgentsOverviewGrouping::Model => {
                model_name(&self.rows[left].thread) == model_name(&self.rows[right].thread)
            }
        }
    }
}

impl AgentsOverviewState {
    /// Retry unavailable paths on reopen without duplicating blocked lookups.
    pub(in crate::app) fn refresh_workspace_groups(&self) {
        self.view_state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .workspace_groups
            .by_cwd
            .retain(|_, group| group.is_none());
    }

    pub(in super::super) fn new(grouping: codex_config::types::AgentsOverviewGrouping) -> Self {
        let state = Self::default();
        state
            .view_state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .grouping = grouping.into();
        state
    }
}

impl App {
    pub(in super::super) async fn persist_agents_overview_grouping(
        &mut self,
        grouping: codex_config::types::AgentsOverviewGrouping,
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
