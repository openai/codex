use std::time::Duration;
use std::time::Instant;

// Environment filter data models for the TUI
#[derive(Clone, Debug, Default)]
pub struct EnvironmentRow {
    pub id: String,
    pub label: Option<String>,
    pub is_pinned: bool,
    pub repo_hints: Option<String>, // e.g., "openai/codex"
}

#[derive(Clone, Debug, Default)]
pub struct EnvModalState {
    pub query: String,
    pub selected: usize,
}

#[derive(Clone, Debug, Default)]
pub struct BestOfModalState {
    pub selected: usize,
}

#[derive(Clone, Debug, Copy, PartialEq, Eq)]
pub enum ApplyResultLevel {
    Success,
    Partial,
    Error,
}

#[derive(Clone, Debug)]
pub struct ApplyModalState {
    pub task_id: TaskId,
    pub title: String,
    pub result_message: Option<String>,
    pub result_level: Option<ApplyResultLevel>,
    pub skipped_paths: Vec<String>,
    pub conflict_paths: Vec<String>,
    pub diff_override: Option<String>,
}

use crate::scrollable_diff::ScrollableDiff;
use codex_cloud_tasks_client::CloudBackend;
use codex_cloud_tasks_client::TaskId;
use codex_cloud_tasks_client::TaskSummary;
#[derive(Default)]
pub struct App {
    pub tasks: Vec<TaskSummary>,
    pub selected: usize,
    pub status: String,
    pub diff_overlay: Option<DiffOverlay>,
    pub spinner_start: Option<Instant>,
    pub refresh_inflight: bool,
    pub details_inflight: bool,
    // Environment filter state
    pub env_filter: Option<String>,
    pub env_modal: Option<EnvModalState>,
    pub apply_modal: Option<ApplyModalState>,
    pub best_of_modal: Option<BestOfModalState>,
    pub environments: Vec<EnvironmentRow>,
    pub env_last_loaded: Option<std::time::Instant>,
    pub env_loading: bool,
    pub env_error: Option<String>,
    // New Task page
    pub new_task: Option<crate::new_task::NewTaskPage>,
    pub best_of_n: usize,
    // Apply preflight spinner state
    pub apply_preflight_inflight: bool,
    // Apply action spinner state
    pub apply_inflight: bool,
    // Background enrichment coordination
    pub list_generation: u64,
    pub in_flight: std::collections::HashSet<String>,
    // Background enrichment caches were planned; currently unused.
}

impl App {
    pub fn new() -> Self {
        Self {
            tasks: Vec::new(),
            selected: 0,
            status: "Press r to refresh".to_string(),
            diff_overlay: None,
            spinner_start: None,
            refresh_inflight: false,
            details_inflight: false,
            env_filter: None,
            env_modal: None,
            apply_modal: None,
            best_of_modal: None,
            environments: Vec::new(),
            env_last_loaded: None,
            env_loading: false,
            env_error: None,
            new_task: None,
            best_of_n: 1,
            apply_preflight_inflight: false,
            apply_inflight: false,
            list_generation: 0,
            in_flight: std::collections::HashSet::new(),
        }
    }

    pub fn next(&mut self) {
        if self.tasks.is_empty() {
            return;
        }
        self.selected = (self.selected + 1).min(self.tasks.len().saturating_sub(1));
    }

    pub fn prev(&mut self) {
        if self.tasks.is_empty() {
            return;
        }
        if self.selected > 0 {
            self.selected -= 1;
        }
    }
}

pub async fn load_tasks(
    backend: &dyn CloudBackend,
    env: Option<&str>,
) -> anyhow::Result<Vec<TaskSummary>> {
    // In later milestones, add a small debounce, spinner, and error display.
    let tasks = tokio::time::timeout(Duration::from_secs(5), backend.list_tasks(env)).await??;
    // Hide review-only tasks from the main list.
    let filtered: Vec<TaskSummary> = tasks.into_iter().filter(|t| !t.is_review).collect();
    Ok(filtered)
}

pub struct DiffOverlay {
    pub title: String,
    pub task_id: TaskId,
    pub sd: ScrollableDiff,
    pub base_can_apply: bool,
    pub diff_lines: Vec<String>,
    pub text_lines: Vec<String>,
    pub prompt: Option<String>,
    pub attempts: Vec<AttemptView>,
    pub selected_attempt: usize,
    pub current_view: DetailView,
    pub base_turn_id: Option<String>,
    pub sibling_turn_ids: Vec<String>,
    pub attempt_total_hint: Option<usize>,
}

#[derive(Clone, Debug, Default)]
pub struct AttemptView {
    pub turn_id: Option<String>,
    pub status: codex_cloud_tasks_client::AttemptStatus,
    pub attempt_placement: Option<i64>,
    pub diff_lines: Vec<String>,
    pub text_lines: Vec<String>,
    pub prompt: Option<String>,
    pub diff_raw: Option<String>,
}

impl AttemptView {
    pub fn has_diff(&self) -> bool {
        !self.diff_lines.is_empty()
    }

    pub fn has_text(&self) -> bool {
        !self.text_lines.is_empty() || self.prompt.is_some()
    }
}

impl DiffOverlay {
    pub fn new(task_id: TaskId, title: String, attempt_total_hint: Option<usize>) -> Self {
        let mut sd = ScrollableDiff::new();
        sd.set_content(Vec::new());
        Self {
            title,
            task_id,
            sd,
            base_can_apply: false,
            diff_lines: Vec::new(),
            text_lines: Vec::new(),
            prompt: None,
            attempts: vec![AttemptView::default()],
            selected_attempt: 0,
            current_view: DetailView::Prompt,
            base_turn_id: None,
            sibling_turn_ids: Vec::new(),
            attempt_total_hint,
        }
    }

    pub fn current_attempt(&self) -> Option<&AttemptView> {
        self.attempts.get(self.selected_attempt)
    }

    pub fn base_attempt_mut(&mut self) -> &mut AttemptView {
        if self.attempts.is_empty() {
            self.attempts.push(AttemptView::default());
        }
        &mut self.attempts[0]
    }

    pub fn set_view(&mut self, view: DetailView) {
        self.current_view = view;
        self.apply_selection_to_fields();
    }

    pub fn expected_attempts(&self) -> Option<usize> {
        self.attempt_total_hint.or({
            if self.attempts.is_empty() {
                None
            } else {
                Some(self.attempts.len())
            }
        })
    }

    pub fn attempt_count(&self) -> usize {
        self.attempts.len()
    }

    pub fn attempt_display_total(&self) -> usize {
        self.expected_attempts()
            .unwrap_or_else(|| self.attempts.len().max(1))
    }

    pub fn step_attempt(&mut self, delta: isize) -> bool {
        let total = self.attempts.len();
        if total <= 1 {
            return false;
        }
        let total_isize = total as isize;
        let current = self.selected_attempt as isize;
        let mut next = current + delta;
        next = ((next % total_isize) + total_isize) % total_isize;
        let next = next as usize;
        self.selected_attempt = next;
        self.apply_selection_to_fields();
        true
    }

    pub fn current_can_apply(&self) -> bool {
        matches!(self.current_view, DetailView::Diff)
            && self
                .current_attempt()
                .and_then(|attempt| attempt.diff_raw.as_ref())
                .map(|diff| !diff.is_empty())
                .unwrap_or(false)
    }

    pub fn apply_selection_to_fields(&mut self) {
        let (diff_lines, text_lines, prompt) = if let Some(attempt) = self.current_attempt() {
            (
                attempt.diff_lines.clone(),
                attempt.text_lines.clone(),
                attempt.prompt.clone(),
            )
        } else {
            self.diff_lines.clear();
            self.text_lines.clear();
            self.prompt = None;
            self.sd.set_content(vec!["<loading attempt>".to_string()]);
            return;
        };

        self.diff_lines = diff_lines.clone();
        self.text_lines = text_lines.clone();
        self.prompt = prompt;

        match self.current_view {
            DetailView::Diff => {
                if diff_lines.is_empty() {
                    self.sd.set_content(vec!["<no diff available>".to_string()]);
                } else {
                    self.sd.set_content(diff_lines);
                }
            }
            DetailView::Prompt => {
                if text_lines.is_empty() {
                    self.sd.set_content(vec!["<no output>".to_string()]);
                } else {
                    self.sd.set_content(text_lines);
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DetailView {
    Diff,
    Prompt,
}

/// Internal app events delivered from background tasks.
/// These let the UI event loop remain responsive and keep the spinner animating.
#[derive(Debug)]
pub enum AppEvent {
    TasksLoaded {
        env: Option<String>,
        result: anyhow::Result<Vec<TaskSummary>>,
    },
    // Background diff summary events were planned; removed for now to keep code minimal.
    /// Autodetection of a likely environment id finished
    EnvironmentAutodetected(anyhow::Result<crate::env_detect::AutodetectSelection>),
    /// Background completion of environment list fetch
    EnvironmentsLoaded(anyhow::Result<Vec<EnvironmentRow>>),
    DetailsDiffLoaded {
        id: TaskId,
        title: String,
        diff: String,
    },
    DetailsMessagesLoaded {
        id: TaskId,
        title: String,
        messages: Vec<String>,
        prompt: Option<String>,
        turn_id: Option<String>,
        sibling_turn_ids: Vec<String>,
        attempt_placement: Option<i64>,
        attempt_status: codex_cloud_tasks_client::AttemptStatus,
    },
    DetailsFailed {
        id: TaskId,
        title: String,
        error: String,
    },
    AttemptsLoaded {
        id: TaskId,
        attempts: Vec<codex_cloud_tasks_client::TurnAttempt>,
    },
    /// Background completion of new task submission
    NewTaskSubmitted(Result<codex_cloud_tasks_client::CreatedTask, String>),
    /// Background completion of apply preflight when opening modal or on demand
    ApplyPreflightFinished {
        id: TaskId,
        title: String,
        message: String,
        level: ApplyResultLevel,
        skipped: Vec<String>,
        conflicts: Vec<String>,
    },
    /// Background completion of apply action (actual patch application)
    ApplyFinished {
        id: TaskId,
        result: std::result::Result<codex_cloud_tasks_client::ApplyOutcome, String>,
    },
}

// Convenience aliases; currently unused.
#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use codex_cloud_tasks_client::CloudTaskError;

    struct FakeBackend {
        // maps env key to titles
        by_env: std::collections::HashMap<Option<String>, Vec<&'static str>>,
    }

    #[async_trait::async_trait]
    impl codex_cloud_tasks_client::CloudBackend for FakeBackend {
        async fn list_tasks(
            &self,
            env: Option<&str>,
        ) -> codex_cloud_tasks_client::Result<Vec<TaskSummary>> {
            let key = env.map(str::to_string);
            let titles = self
                .by_env
                .get(&key)
                .cloned()
                .unwrap_or_else(|| vec!["default-a", "default-b"]);
            let mut out = Vec::new();
            for (i, t) in titles.into_iter().enumerate() {
                out.push(TaskSummary {
                    id: TaskId(format!("T-{i}")),
                    title: t.to_string(),
                    status: codex_cloud_tasks_client::TaskStatus::Ready,
                    updated_at: Utc::now(),
                    environment_id: env.map(str::to_string),
                    environment_label: None,
                    summary: codex_cloud_tasks_client::DiffSummary::default(),
                    is_review: false,
                    attempt_total: Some(1),
                });
            }
            Ok(out)
        }

        async fn get_task_summary(
            &self,
            id: TaskId,
        ) -> codex_cloud_tasks_client::Result<TaskSummary> {
            self.list_tasks(None)
                .await?
                .into_iter()
                .find(|t| t.id == id)
                .ok_or_else(|| CloudTaskError::Msg(format!("Task {} not found", id.0)))
        }

        async fn get_task_diff(
            &self,
            _id: TaskId,
        ) -> codex_cloud_tasks_client::Result<Option<String>> {
            Err(codex_cloud_tasks_client::CloudTaskError::Unimplemented(
                "not used in test",
            ))
        }

        async fn get_task_messages(
            &self,
            _id: TaskId,
        ) -> codex_cloud_tasks_client::Result<Vec<String>> {
            Ok(vec![])
        }
        async fn get_task_text(
            &self,
            _id: TaskId,
        ) -> codex_cloud_tasks_client::Result<codex_cloud_tasks_client::TaskText> {
            Ok(codex_cloud_tasks_client::TaskText {
                prompt: Some("Example prompt".to_string()),
                messages: Vec::new(),
                turn_id: Some("fake-turn".to_string()),
                sibling_turn_ids: Vec::new(),
                attempt_placement: Some(0),
                attempt_status: codex_cloud_tasks_client::AttemptStatus::Completed,
            })
        }

        async fn list_sibling_attempts(
            &self,
            _task: TaskId,
            _turn_id: String,
        ) -> codex_cloud_tasks_client::Result<Vec<codex_cloud_tasks_client::TurnAttempt>> {
            Ok(Vec::new())
        }

        async fn apply_task(
            &self,
            _id: TaskId,
            _diff_override: Option<String>,
        ) -> codex_cloud_tasks_client::Result<codex_cloud_tasks_client::ApplyOutcome> {
            Err(codex_cloud_tasks_client::CloudTaskError::Unimplemented(
                "not used in test",
            ))
        }

        async fn apply_task_preflight(
            &self,
            _id: TaskId,
            _diff_override: Option<String>,
        ) -> codex_cloud_tasks_client::Result<codex_cloud_tasks_client::ApplyOutcome> {
            Err(codex_cloud_tasks_client::CloudTaskError::Unimplemented(
                "not used in test",
            ))
        }

        async fn create_task(
            &self,
            _env_id: &str,
            _prompt: &str,
            _git_ref: &str,
            _qa_mode: bool,
            _best_of_n: usize,
        ) -> codex_cloud_tasks_client::Result<codex_cloud_tasks_client::CreatedTask> {
            Err(codex_cloud_tasks_client::CloudTaskError::Unimplemented(
                "not used in test",
            ))
        }
    }

    #[tokio::test]
    async fn load_tasks_uses_env_parameter() {
        // Arrange: env-specific task titles
        let mut by_env = std::collections::HashMap::new();
        by_env.insert(None, vec!["root-1", "root-2"]);
        by_env.insert(Some("env-A".to_string()), vec!["A-1"]);
        by_env.insert(Some("env-B".to_string()), vec!["B-1", "B-2", "B-3"]);
        let backend = FakeBackend { by_env };

        // Act + Assert
        let root = load_tasks(&backend, None).await.unwrap();
        assert_eq!(root.len(), 2);
        assert_eq!(root[0].title, "root-1");

        let a = load_tasks(&backend, Some("env-A")).await.unwrap();
        assert_eq!(a.len(), 1);
        assert_eq!(a[0].title, "A-1");

        let b = load_tasks(&backend, Some("env-B")).await.unwrap();
        assert_eq!(b.len(), 3);
        assert_eq!(b[2].title, "B-3");
    }
}
