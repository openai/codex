use std::any::Any;
use std::path::PathBuf;

use crate::app_event_sender::AppEventSender;

use super::BottomPane;
use super::SearchableTablePickerView;
use super::TablePickerItem;
use super::TablePickerOnSelected;

#[derive(Clone)]
pub(crate) struct CommitSelection {
    pub full_sha: String,
    pub short_sha: String,
    pub summary: String,
}

pub(crate) type CommitSelected = TablePickerOnSelected<CommitSelection>;

pub(crate) struct CommitPickerView {
    inner: SearchableTablePickerView<CommitSelection>,
}

impl CommitPickerView {
    pub(crate) fn new(
        cwd: PathBuf,
        app_event_tx: AppEventSender,
        on_selected: CommitSelected,
    ) -> Self {
        let commits = load_recent_commits(&cwd);
        let inner = SearchableTablePickerView::new(
            "Select a commit".to_string(),
            "Type to search commits".to_string(),
            "no commits found".to_string(),
            commits,
            10,
            app_event_tx,
            on_selected,
        );
        Self { inner }
    }
}

impl super::bottom_pane_view::BottomPaneView for CommitPickerView {
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

fn load_recent_commits(cwd: &PathBuf) -> Vec<TablePickerItem<CommitSelection>> {
    const FIELDS_SEPARATOR: char = '\u{1f}';
    let output = std::process::Command::new("git")
        .args([
            "log",
            "-n",
            "100",
            "--pretty=format:%H%x1f%h%x1f%an%x1f%ar%x1f%s",
        ])
        .current_dir(cwd)
        .output();

    let stdout = match output {
        Ok(out) if out.status.success() => out.stdout,
        _ => Vec::new(),
    };

    String::from_utf8_lossy(&stdout)
        .lines()
        .filter_map(|line| parse_log_line(line, FIELDS_SEPARATOR))
        .collect()
}

fn parse_log_line(line: &str, separator: char) -> Option<TablePickerItem<CommitSelection>> {
    let mut parts = line.split(separator);
    let full_sha = parts.next()?.trim().to_string();
    let short_sha = parts.next()?.trim().to_string();
    let author = parts.next()?.trim().to_string();
    let relative_time = parts.next()?.trim().to_string();
    let summary = parts.next().unwrap_or_default().trim().to_string();

    if full_sha.is_empty() || short_sha.is_empty() {
        return None;
    }

    let label = if summary.is_empty() {
        short_sha.clone()
    } else {
        format!("{short_sha} {summary}")
    };

    let description = if author.is_empty() && relative_time.is_empty() {
        None
    } else if author.is_empty() {
        Some(relative_time.clone())
    } else if relative_time.is_empty() {
        Some(author.clone())
    } else {
        Some(format!("{author} · {relative_time}"))
    };

    let search_value = format!("{full_sha} {short_sha} {summary} {author} {relative_time}");

    Some(TablePickerItem {
        value: CommitSelection {
            full_sha,
            short_sha,
            summary,
        },
        label,
        description,
        search_value,
        detail_builder: None,
    })
}
