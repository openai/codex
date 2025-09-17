use std::any::Any;
use std::path::Path;
use std::path::PathBuf;

use crate::app_event_sender::AppEventSender;
use crate::git_shortstat::get_diff_shortstat_against;

use super::BottomPane;
use super::SearchableTablePickerView;
use super::TablePickerItem;
use super::TablePickerOnSelected;
use super::bottom_pane_view::BottomPaneView;
use super::popup_consts::MAX_POPUP_ROWS;
use ratatui::style::Stylize;
use ratatui::text::Span;

/// Callback invoked when a branch is selected.
pub(crate) type BranchSelected = TablePickerOnSelected<String>;

/// A searchable branch picker view. Shows the repository's branches with the
/// default/base branch pinned to the top when present.
pub(crate) struct BranchPickerView {
    inner: SearchableTablePickerView<String>,
}

impl BranchPickerView {
    pub(crate) fn new(
        cwd: PathBuf,
        app_event_tx: AppEventSender,
        on_selected: BranchSelected,
    ) -> Self {
        let branch_items = load_branch_items(&cwd);
        let inner = SearchableTablePickerView::new(
            "Select a base branch".to_string(),
            "Type to search branches".to_string(),
            "no matches".to_string(),
            branch_items,
            MAX_POPUP_ROWS,
            app_event_tx,
            on_selected,
        );
        Self { inner }
    }
}

impl BottomPaneView for BranchPickerView {
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn handle_key_event(&mut self, pane: &mut BottomPane, key_event: crossterm::event::KeyEvent) {
        self.inner.handle_key_event(pane, key_event);
    }

    fn is_complete(&self) -> bool {
        self.inner.is_complete()
    }

    fn on_ctrl_c(&mut self, pane: &mut super::BottomPane) -> super::CancellationEvent {
        self.inner.on_ctrl_c(pane)
    }

    fn desired_height(&self, width: u16) -> u16 {
        self.inner.desired_height(width)
    }

    fn render(&self, area: ratatui::layout::Rect, buf: &mut ratatui::buffer::Buffer) {
        self.inner.render(area, buf);
    }
}

fn load_branch_items(cwd: &PathBuf) -> Vec<TablePickerItem<String>> {
    let branches = gather_branches(cwd);
    let current_branch = current_branch_name(cwd).unwrap_or_else(|| "(detached HEAD)".to_string());

    branches
        .into_iter()
        .map(|branch| {
            let label = format!("{current_branch} -> {branch}");
            let search_value = format!("{current_branch} {branch}");
            let cwd = cwd.clone();
            let detail_branch = branch.clone();
            TablePickerItem {
                value: branch,
                label,
                description: None,
                search_value,
                // Wrap the async function in a blocking call for sync context
                detail_builder: Some(Box::new(move || {
                    // Use block_in_place to avoid blocking the async runtime if present
                    tokio::task::block_in_place(|| {
                        tokio::runtime::Handle::try_current()
                            .ok()
                            .and_then(|handle| {
                                handle.block_on(branch_shortstat(&cwd, &detail_branch))
                            })
                            .or_else(|| {
                                // Fallback: create a new runtime if not in one
                                let rt = tokio::runtime::Runtime::new().ok()?;
                                rt.block_on(branch_shortstat(&cwd, &detail_branch))
                            })
                    })
                })),
            }
        })
        .collect()
}

fn gather_branches(cwd: &PathBuf) -> Vec<String> {
    let out = std::process::Command::new("git")
        .args(["branch", "--format=%(refname:short)"])
        .current_dir(cwd)
        .output();

    let mut branches: Vec<String> = match out {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout)
            .lines()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect(),
        _ => Vec::new(),
    };

    branches.sort_unstable();

    if let Some(base) = default_branch(cwd, &branches)
        && let Some(pos) = branches.iter().position(|name| name == &base)
    {
        let base_branch = branches.remove(pos);
        branches.insert(0, base_branch);
    }

    branches
}

fn default_branch(cwd: &PathBuf, branches: &[String]) -> Option<String> {
    let configured_default = if let Ok(out) = std::process::Command::new("git")
        .args(["config", "--get", "init.defaultBranch"])
        .current_dir(cwd)
        .output()
    {
        if out.status.success() {
            String::from_utf8(out.stdout)
                .ok()
                .map(|s| s.trim().to_string())
        } else {
            None
        }
    } else {
        None
    };

    if let Some(default_branch) = configured_default
        && branches.iter().any(|branch| branch == &default_branch)
    {
        return Some(default_branch);
    }

    for candidate in ["main", "master"] {
        if branches.iter().any(|branch| branch == candidate) {
            return Some(candidate.to_string());
        }
    }

    None
}

fn current_branch_name(cwd: &PathBuf) -> Option<String> {
    let out = std::process::Command::new("git")
        .args(["branch", "--show-current"])
        .current_dir(cwd)
        .output()
        .ok()?;

    if !out.status.success() {
        return None;
    }

    String::from_utf8(out.stdout)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|name| !name.is_empty())
}

async fn branch_shortstat(cwd: &Path, branch: &str) -> Option<Vec<Span<'static>>> {
    let stats = get_diff_shortstat_against(cwd, branch)
        .await
        .ok()
        .flatten()?;
    if stats.files_changed == 0 && stats.insertions == 0 && stats.deletions == 0 {
        return None;
    }

    let file_label = if stats.files_changed == 1 {
        "file changed"
    } else {
        "files changed"
    };

    let mut spans: Vec<Span<'static>> = Vec::new();
    spans.push(format!("- {} {file_label} (", stats.files_changed).dim());
    spans.push(format!("+{}", stats.insertions).green());
    spans.push(" ".dim());
    spans.push(format!("-{}", stats.deletions).red());
    spans.push(")".dim());
    Some(spans)
}
