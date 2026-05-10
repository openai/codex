//! App-layer handlers for the worktree TUI flow.

use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::WorktreeCreateParams;
use codex_app_server_protocol::WorktreeCreateResponse;
use codex_app_server_protocol::WorktreeDirtyPolicy as ApiWorktreeDirtyPolicy;
use codex_app_server_protocol::WorktreeDirtyState as ApiWorktreeDirtyState;
use codex_app_server_protocol::WorktreeInfo as ApiWorktreeInfo;
use codex_app_server_protocol::WorktreeInspectSourceParams;
use codex_app_server_protocol::WorktreeInspectSourceResponse;
use codex_app_server_protocol::WorktreeListParams;
use codex_app_server_protocol::WorktreeListResponse;
use codex_app_server_protocol::WorktreeLocation as ApiWorktreeLocation;
use codex_app_server_protocol::WorktreeRemoveParams;
use codex_app_server_protocol::WorktreeRemoveResponse;
use codex_app_server_protocol::WorktreeSource as ApiWorktreeSource;
use codex_protocol::ThreadId;
use codex_worktree::DirtyPolicy;
use codex_worktree::WorktreeInfo;
use codex_worktree::WorktreeLocation;
use codex_worktree::WorktreeRequest;
use codex_worktree::WorktreeResolution;
use codex_worktree::WorktreeSource;
use codex_worktree::WorktreeWarning;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
use uuid::Uuid;

use super::*;
use crate::bottom_pane::custom_prompt_view::CustomPromptView;

const WORKTREE_SWITCH_RENDER_DELAY: Duration = Duration::from_millis(20);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WorktreeSwitchMode {
    StartFresh,
    Fork(ThreadId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WorktreeSessionTransition {
    Forked,
    Started,
}

impl WorktreeSessionTransition {
    fn message_prefix(self) -> &'static str {
        match self {
            WorktreeSessionTransition::Forked => "Forked into",
            WorktreeSessionTransition::Started => "Started session in",
        }
    }
}

impl App {
    pub(super) async fn open_worktree_picker(
        &mut self,
        tui: &mut tui::Tui,
        app_server: &AppServerSession,
    ) {
        self.chat_widget
            .show_selection_view(crate::worktree::loading_params(
                tui.frame_requester(),
                self.config.animations,
            ));
        let result = self
            .list_current_repo_worktrees_for_session(app_server)
            .await
            .map_err(|err| err.to_string());
        self.on_worktrees_loaded(self.session_workspace_cwd(app_server).to_path_buf(), result);
    }

    pub(super) fn open_worktree_create_prompt(&mut self) {
        let tx = self.app_event_tx.clone();
        let view = CustomPromptView::new(
            "New worktree".to_string(),
            "Type a branch name and press Enter".to_string(),
            /*initial_text*/ String::new(),
            /*context_label*/
            Some("Creates a sibling worktree and starts this chat there.".to_string()),
            Box::new(move |branch: String| {
                tx.send(AppEvent::OpenWorktreeBaseRefPrompt {
                    branch: branch.trim().to_string(),
                });
            }),
        );
        self.chat_widget.show_bottom_pane_view(Box::new(view));
    }

    pub(super) fn open_worktree_base_ref_prompt(&mut self, branch: String) {
        let tx = self.app_event_tx.clone();
        let view = CustomPromptView::new_allow_empty(
            "Base ref".to_string(),
            "Optional base ref; leave blank for default".to_string(),
            /*initial_text*/ String::new(),
            /*context_label*/
            Some(format!(
                "Create {branch} from this ref, or leave blank for the default."
            )),
            Box::new(move |base_ref: String| {
                let base_ref = base_ref.trim();
                tx.send(AppEvent::CreateWorktreeAndSwitch {
                    branch: branch.clone(),
                    base_ref: (!base_ref.is_empty()).then(|| base_ref.to_string()),
                    dirty_policy: None,
                });
            }),
        );
        self.chat_widget.show_bottom_pane_view(Box::new(view));
    }

    pub(super) fn on_worktrees_loaded(
        &mut self,
        cwd: PathBuf,
        result: Result<Vec<WorktreeInfo>, String>,
    ) {
        if self.remote_app_server_url.is_none() && cwd.as_path() != self.config.cwd.as_path() {
            return;
        }
        let params = match result {
            Ok(entries) if entries.is_empty() => crate::worktree::empty_params(),
            Ok(entries) => crate::worktree::picker_params(entries, cwd.as_path()),
            Err(err) => crate::worktree::error_params(err),
        };
        self.replace_worktree_view(params);
    }

    pub(super) async fn create_worktree_and_switch(
        &mut self,
        tui: &mut tui::Tui,
        app_server: &AppServerSession,
        branch: String,
        base_ref: Option<String>,
        dirty_policy: Option<DirtyPolicy>,
    ) {
        let dirty_policy = match dirty_policy {
            Some(policy) => policy,
            None => match self.source_worktree_dirty_state(app_server).await {
                Ok(state) if state.is_dirty() => {
                    let params = crate::worktree::dirty_policy_prompt_params(branch, base_ref);
                    self.chat_widget.show_selection_view(params);
                    return;
                }
                Ok(_) => DirtyPolicy::Fail,
                Err(err) => {
                    self.chat_widget
                        .add_error_message(format!("Failed to inspect source checkout: {err}"));
                    return;
                }
            },
        };

        self.show_worktree_creating_view(tui, branch.clone());
        self.spawn_worktree_create_request(
            app_server,
            WorktreeRequest {
                codex_home: self.config.codex_home.to_path_buf(),
                source_cwd: self.session_workspace_cwd(app_server).to_path_buf(),
                branch,
                base_ref,
                dirty_policy,
            },
        );
    }

    pub(super) async fn on_worktree_created(
        &mut self,
        tui: &mut tui::Tui,
        app_server: &mut AppServerSession,
        cwd: PathBuf,
        result: Result<WorktreeResolution, String>,
    ) {
        if cwd.as_path() != self.session_workspace_cwd(app_server) {
            return;
        }
        let resolution = match result {
            Ok(resolution) => resolution,
            Err(err) => {
                self.show_worktree_error("Failed to create worktree.".to_string(), err);
                return;
            }
        };
        let target = resolution
            .info
            .branch
            .clone()
            .unwrap_or_else(|| resolution.info.name.clone());
        self.show_worktree_switching_view(tui, target);
        self.switch_to_worktree_info(
            tui,
            app_server,
            resolution.info,
            resolution
                .warnings
                .into_iter()
                .map(|warning| warning.message)
                .collect(),
        )
        .await;
    }

    pub(super) fn begin_switch_to_worktree_target(&mut self, tui: &mut tui::Tui, target: String) {
        self.show_worktree_switching_view(tui, target.clone());
        self.defer_switch_to_worktree_target(target);
    }

    pub(super) fn current_worktree_selected(&mut self, target: String) {
        self.chat_widget
            .add_info_message(format!("Already in worktree {target}."), /*hint*/ None);
    }

    pub(super) async fn switch_to_worktree_target_after_loading(
        &mut self,
        tui: &mut tui::Tui,
        app_server: &mut AppServerSession,
        target: String,
    ) {
        let entries = match self
            .list_current_repo_worktrees_for_session(app_server)
            .await
        {
            Ok(entries) => entries,
            Err(err) => {
                self.show_worktree_error("Failed to list worktrees.".to_string(), err.to_string());
                return;
            }
        };
        let info = match crate::worktree::find_worktree(&entries, &target) {
            Ok(info) => info.clone(),
            Err(err) => {
                self.show_worktree_error("Failed to switch worktree.".to_string(), err);
                return;
            }
        };
        self.switch_to_worktree_info(tui, app_server, info, Vec::new())
            .await;
    }

    pub(super) async fn show_worktree_path(
        &mut self,
        app_server: &AppServerSession,
        target: String,
    ) {
        match self
            .list_current_repo_worktrees_for_session(app_server)
            .await
        {
            Ok(entries) => match crate::worktree::find_worktree(&entries, &target) {
                Ok(info) => {
                    self.chat_widget.add_info_message(
                        info.workspace_cwd.display().to_string(),
                        /*hint*/ None,
                    );
                }
                Err(err) => self.chat_widget.add_error_message(err),
            },
            Err(err) => {
                self.chat_widget
                    .add_error_message(format!("Failed to list worktrees: {err}"));
            }
        }
    }

    pub(super) async fn remove_worktree(
        &mut self,
        app_server: &AppServerSession,
        target: String,
        force: bool,
        delete_branch: bool,
        confirmed: bool,
    ) {
        let entries = match self
            .list_current_repo_worktrees_for_session(app_server)
            .await
        {
            Ok(entries) => entries,
            Err(err) => {
                self.chat_widget
                    .add_error_message(format!("Failed to list worktrees: {err}"));
                return;
            }
        };
        let info = match crate::worktree::find_worktree(&entries, &target) {
            Ok(info) => info,
            Err(err) => {
                self.chat_widget.add_error_message(err);
                return;
            }
        };
        if info.source != WorktreeSource::Cli {
            let source = crate::worktree::source_label(info.source);
            self.chat_widget.add_error_message(format!(
                "Refusing to remove {source} worktree '{target}'. Only Codex CLI-managed worktrees can be removed."
            ));
            return;
        }
        if !confirmed {
            let params = crate::worktree::remove_confirmation_params(target, force, delete_branch);
            self.chat_widget.show_selection_view(params);
            return;
        }

        let result = self
            .remove_worktree_via_app_server(app_server, target.clone(), force, delete_branch)
            .await;
        match result {
            Ok(result) => {
                let mut message = format!("Removed worktree {}", result.removed_path);
                if let Some(branch) = result.deleted_branch {
                    message.push_str(&format!(" and deleted branch {branch}"));
                }
                self.chat_widget.add_info_message(message, /*hint*/ None);
            }
            Err(err) => {
                self.chat_widget
                    .add_error_message(format!("Failed to remove worktree: {err}"));
            }
        }
    }

    async fn list_current_repo_worktrees_for_session(
        &self,
        app_server: &AppServerSession,
    ) -> anyhow::Result<Vec<WorktreeInfo>> {
        let response: WorktreeListResponse = app_server
            .request_handle()
            .request_typed(ClientRequest::WorktreeList {
                request_id: worktree_request_id("worktree-list"),
                params: WorktreeListParams {
                    cwd: Some(self.session_workspace_cwd(app_server).display().to_string()),
                    all: false,
                },
            })
            .await?;
        Ok(response
            .data
            .into_iter()
            .map(worktree_info_from_api)
            .collect())
    }

    async fn source_worktree_dirty_state(
        &self,
        app_server: &AppServerSession,
    ) -> anyhow::Result<codex_worktree::DirtyState> {
        let response: WorktreeInspectSourceResponse = app_server
            .request_handle()
            .request_typed(ClientRequest::WorktreeInspectSource {
                request_id: worktree_request_id("worktree-inspect-source"),
                params: WorktreeInspectSourceParams {
                    cwd: Some(self.session_workspace_cwd(app_server).display().to_string()),
                },
            })
            .await?;
        Ok(dirty_state_from_api(response.dirty))
    }

    fn session_workspace_cwd<'a>(&'a self, app_server: &'a AppServerSession) -> &'a Path {
        if self.remote_app_server_url.is_some() {
            app_server
                .remote_cwd_override()
                .or_else(|| {
                    self.primary_session_configured
                        .as_ref()
                        .map(|session| session.cwd.as_path())
                })
                .unwrap_or(self.config.cwd.as_path())
        } else {
            self.config.cwd.as_path()
        }
    }

    fn spawn_worktree_create_request(
        &self,
        app_server: &AppServerSession,
        request: WorktreeRequest,
    ) {
        let cwd = request.source_cwd.clone();
        let app_event_tx = self.app_event_tx.clone();
        let request_handle = app_server.request_handle();
        tokio::spawn(async move {
            let result: Result<WorktreeCreateResponse, _> = request_handle
                .request_typed(ClientRequest::WorktreeCreate {
                    request_id: worktree_request_id("worktree-create"),
                    params: WorktreeCreateParams {
                        cwd: Some(request.source_cwd.display().to_string()),
                        branch: request.branch,
                        base_ref: request.base_ref,
                        dirty_policy: dirty_policy_to_api(request.dirty_policy),
                    },
                })
                .await;
            let result = result
                .map(worktree_resolution_from_api)
                .map_err(|err| err.to_string());
            app_event_tx.send(AppEvent::WorktreeCreated { cwd, result });
        });
    }

    async fn switch_to_worktree_info(
        &mut self,
        tui: &mut tui::Tui,
        app_server: &mut AppServerSession,
        info: WorktreeInfo,
        warnings: Vec<String>,
    ) {
        let mut config = if app_server.is_remote() {
            self.config.clone()
        } else {
            match self
                .rebuild_config_for_cwd(info.workspace_cwd.clone())
                .await
            {
                Ok(config) => config,
                Err(err) => {
                    self.show_worktree_error(
                        "Failed to rebuild configuration for worktree.".to_string(),
                        err.to_string(),
                    );
                    return;
                }
            }
        };
        self.apply_runtime_policy_overrides(&mut config);

        let mode = self.worktree_switch_mode().await;
        self.spawn_worktree_session_request(app_server, info, config, mode, warnings);
        tui.frame_requester().schedule_frame();
    }

    pub(super) async fn on_worktree_session_ready(
        &mut self,
        tui: &mut tui::Tui,
        app_server: &mut AppServerSession,
        info: WorktreeInfo,
        config: Config,
        forked: bool,
        warnings: Vec<String>,
        result: Result<AppServerStartedThread, String>,
    ) {
        match result {
            Ok(started) => {
                self.shutdown_current_thread(app_server).await;
                self.install_worktree_config(tui, config);
                if let Err(err) = self
                    .replace_chat_widget_with_app_server_thread(
                        tui, app_server, started, /*initial_user_message*/ None,
                    )
                    .await
                {
                    self.show_worktree_error(
                        "Failed to attach to worktree thread.".to_string(),
                        err.to_string(),
                    );
                } else {
                    if app_server.is_remote() {
                        app_server.set_remote_cwd_override(Some(info.workspace_cwd.clone()));
                    }
                    let transition = if forked {
                        WorktreeSessionTransition::Forked
                    } else {
                        WorktreeSessionTransition::Started
                    };
                    self.add_worktree_session_message(&info, transition);
                    for warning in warnings {
                        self.chat_widget.add_info_message(warning, /*hint*/ None);
                    }
                }
            }
            Err(err) => {
                let summary = if forked {
                    "Failed to fork current session into worktree."
                } else {
                    "Failed to start session in worktree."
                };
                self.show_worktree_error(summary.to_string(), err);
            }
        }
        tui.frame_requester().schedule_frame();
    }

    fn spawn_worktree_session_request(
        &self,
        app_server: &AppServerSession,
        info: WorktreeInfo,
        config: Config,
        mode: WorktreeSwitchMode,
        warnings: Vec<String>,
    ) {
        let request_handle = app_server.request_handle();
        let remote_cwd_override = if app_server.is_remote() {
            Some(info.workspace_cwd.clone())
        } else {
            app_server.remote_cwd_override().map(Path::to_path_buf)
        };
        let app_event_tx = self.app_event_tx.clone();
        tokio::spawn(async move {
            let forked = matches!(mode, WorktreeSwitchMode::Fork(_));
            let result = match mode {
                WorktreeSwitchMode::Fork(thread_id) => {
                    crate::app_server_session::fork_thread_with_request_handle(
                        request_handle,
                        config.clone(),
                        thread_id,
                        remote_cwd_override,
                    )
                    .await
                }
                WorktreeSwitchMode::StartFresh => {
                    crate::app_server_session::start_thread_with_request_handle(
                        request_handle,
                        config.clone(),
                        remote_cwd_override,
                    )
                    .await
                }
            }
            .map_err(|err| err.to_string());
            app_event_tx.send(AppEvent::WorktreeSessionReady {
                info,
                config,
                forked,
                warnings,
                result,
            });
        });
    }

    fn add_worktree_session_message(
        &mut self,
        info: &WorktreeInfo,
        transition: WorktreeSessionTransition,
    ) {
        let (message, hint) = worktree_session_message(info, transition);
        self.chat_widget.add_info_message(message, Some(hint));
    }

    async fn worktree_switch_mode(&self) -> WorktreeSwitchMode {
        let Some(thread_id) = self.current_displayed_thread_id() else {
            return WorktreeSwitchMode::StartFresh;
        };

        if self
            .session_for_thread(thread_id)
            .await
            .as_ref()
            .is_some_and(Self::session_has_materialized_rollout)
        {
            WorktreeSwitchMode::Fork(thread_id)
        } else {
            WorktreeSwitchMode::StartFresh
        }
    }

    async fn session_for_thread(&self, thread_id: ThreadId) -> Option<ThreadSessionState> {
        if self.primary_thread_id == Some(thread_id)
            && let Some(session) = self.primary_session_configured.clone()
        {
            return Some(session);
        }

        let channel = self.thread_event_channels.get(&thread_id)?;
        let store = channel.store.lock().await;
        store.session.clone()
    }

    fn session_has_materialized_rollout(session: &ThreadSessionState) -> bool {
        session
            .rollout_path
            .as_ref()
            .is_some_and(|rollout_path| rollout_path.exists())
    }

    fn show_worktree_switching_view(&mut self, tui: &mut tui::Tui, target: String) {
        let params = crate::worktree::switching_params(
            target.clone(),
            tui.frame_requester(),
            self.config.animations,
        );
        if !self.replace_worktree_view(params) {
            self.chat_widget
                .show_selection_view(crate::worktree::switching_params(
                    target,
                    tui.frame_requester(),
                    self.config.animations,
                ));
        }
        tui.frame_requester().schedule_frame();
    }

    fn show_worktree_creating_view(&mut self, tui: &mut tui::Tui, branch: String) {
        self.chat_widget
            .show_selection_view(crate::worktree::creating_params(
                branch,
                tui.frame_requester(),
                self.config.animations,
            ));
        tui.frame_requester().schedule_frame();
    }

    fn defer_switch_to_worktree_target(&self, target: String) {
        let app_event_tx = self.app_event_tx.clone();
        tokio::spawn(async move {
            tokio::time::sleep(WORKTREE_SWITCH_RENDER_DELAY).await;
            app_event_tx.send(AppEvent::SwitchToWorktreeAfterLoading { target });
        });
    }

    fn replace_worktree_view(&mut self, params: crate::bottom_pane::SelectionViewParams) -> bool {
        self.chat_widget
            .replace_selection_view_if_active(crate::worktree::WORKTREE_SELECTION_VIEW_ID, params)
    }

    fn show_worktree_error(&mut self, summary: String, error: String) {
        let params = crate::worktree::error_with_summary_params(summary.clone(), error.clone());
        if !self.replace_worktree_view(params) {
            self.chat_widget
                .add_error_message(format!("{summary} {error}"));
        }
    }

    fn install_worktree_config(&mut self, tui: &mut tui::Tui, config: Config) {
        self.config = config;
        tui.set_notification_settings(
            self.config.tui_notifications.method,
            self.config.tui_notifications.condition,
        );
        self.file_search
            .update_search_dir(self.config.cwd.to_path_buf());
    }

    async fn remove_worktree_via_app_server(
        &self,
        app_server: &AppServerSession,
        target: String,
        force: bool,
        delete_branch: bool,
    ) -> anyhow::Result<WorktreeRemoveResponse> {
        app_server
            .request_handle()
            .request_typed(ClientRequest::WorktreeRemove {
                request_id: worktree_request_id("worktree-remove"),
                params: WorktreeRemoveParams {
                    cwd: Some(self.session_workspace_cwd(app_server).display().to_string()),
                    name_or_path: target,
                    force,
                    delete_branch,
                },
            })
            .await
            .map_err(Into::into)
    }
}

fn worktree_request_id(prefix: &str) -> RequestId {
    RequestId::String(format!("{prefix}-{}", Uuid::new_v4()))
}

fn dirty_policy_to_api(value: DirtyPolicy) -> ApiWorktreeDirtyPolicy {
    match value {
        DirtyPolicy::Fail => ApiWorktreeDirtyPolicy::Fail,
        DirtyPolicy::Ignore => ApiWorktreeDirtyPolicy::Ignore,
        DirtyPolicy::CopyTracked => ApiWorktreeDirtyPolicy::CopyTracked,
        DirtyPolicy::CopyAll => ApiWorktreeDirtyPolicy::CopyAll,
        DirtyPolicy::MoveTracked => ApiWorktreeDirtyPolicy::MoveTracked,
        DirtyPolicy::MoveAll => ApiWorktreeDirtyPolicy::MoveAll,
    }
}

fn dirty_state_from_api(value: ApiWorktreeDirtyState) -> codex_worktree::DirtyState {
    codex_worktree::DirtyState {
        has_staged_changes: value.has_staged_changes,
        has_unstaged_changes: value.has_unstaged_changes,
        has_untracked_files: value.has_untracked_files,
    }
}

fn worktree_info_from_api(value: ApiWorktreeInfo) -> WorktreeInfo {
    WorktreeInfo {
        id: value.id,
        name: value.name,
        slug: value.slug,
        source: worktree_source_from_api(value.source),
        location: worktree_location_from_api(value.location),
        repo_name: value.repo_name,
        repo_root: PathBuf::from(value.repo_root),
        common_git_dir: PathBuf::from(value.common_git_dir),
        worktree_git_root: PathBuf::from(value.worktree_git_root),
        workspace_cwd: PathBuf::from(value.workspace_cwd),
        original_relative_cwd: PathBuf::from(value.original_relative_cwd),
        branch: value.branch,
        head: value.head,
        owner_thread_id: value.owner_thread_id,
        metadata_path: PathBuf::from(value.metadata_path),
        dirty: dirty_state_from_api(value.dirty),
    }
}

fn worktree_source_from_api(value: ApiWorktreeSource) -> WorktreeSource {
    match value {
        ApiWorktreeSource::Cli => WorktreeSource::Cli,
        ApiWorktreeSource::App => WorktreeSource::App,
        ApiWorktreeSource::Legacy => WorktreeSource::Legacy,
        ApiWorktreeSource::Git => WorktreeSource::Git,
    }
}

fn worktree_location_from_api(value: ApiWorktreeLocation) -> WorktreeLocation {
    match value {
        ApiWorktreeLocation::Sibling => WorktreeLocation::Sibling,
        ApiWorktreeLocation::CodexHome => WorktreeLocation::CodexHome,
        ApiWorktreeLocation::External => WorktreeLocation::External,
    }
}

fn worktree_resolution_from_api(value: WorktreeCreateResponse) -> WorktreeResolution {
    WorktreeResolution {
        reused: value.reused,
        info: worktree_info_from_api(value.info),
        warnings: value
            .warnings
            .into_iter()
            .map(|warning| WorktreeWarning {
                message: warning.message,
            })
            .collect(),
    }
}

fn worktree_session_message(
    info: &WorktreeInfo,
    transition: WorktreeSessionTransition,
) -> (String, String) {
    let worktree_name = info.branch.as_deref().unwrap_or(info.name.as_str());
    let state = if info.dirty.is_dirty() {
        "dirty"
    } else {
        "clean"
    };
    let source = crate::worktree::source_label(info.source);
    (
        format!(
            "{} {source} worktree {worktree_name} · {state} · {}",
            transition.message_prefix(),
            info.repo_name
        ),
        info.workspace_cwd.display().to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_app_server_protocol::AskForApproval;
    use codex_protocol::config_types::ApprovalsReviewer;
    use codex_protocol::models::PermissionProfile;
    use codex_utils_absolute_path::AbsolutePathBuf;
    use codex_worktree::DirtyState;
    use codex_worktree::WorktreeLocation;
    use tempfile::TempDir;

    #[tokio::test]
    async fn worktree_switch_mode_starts_fresh_without_current_thread() {
        let app = crate::app::test_support::make_test_app().await;

        assert_eq!(
            app.worktree_switch_mode().await,
            WorktreeSwitchMode::StartFresh
        );
    }

    #[tokio::test]
    async fn worktree_switch_mode_starts_fresh_for_unmaterialized_primary_rollout() {
        let temp_dir = TempDir::new().expect("temp dir");
        let thread_id = ThreadId::new();
        let missing_rollout_path = temp_dir.path().join("missing-rollout.jsonl");
        let session = test_thread_session(
            thread_id,
            temp_dir.path().join("repo"),
            missing_rollout_path,
        );
        let mut app = crate::app::test_support::make_test_app().await;
        app.primary_thread_id = Some(thread_id);
        app.active_thread_id = Some(thread_id);
        app.primary_session_configured = Some(session);

        assert_eq!(
            app.worktree_switch_mode().await,
            WorktreeSwitchMode::StartFresh
        );
    }

    #[tokio::test]
    async fn worktree_switch_mode_forks_materialized_primary_rollout() {
        let temp_dir = TempDir::new().expect("temp dir");
        let thread_id = ThreadId::new();
        let rollout_path = temp_dir.path().join("rollout.jsonl");
        std::fs::write(&rollout_path, "{}\\n").expect("write rollout");
        let session = test_thread_session(thread_id, temp_dir.path().join("repo"), rollout_path);
        let mut app = crate::app::test_support::make_test_app().await;
        app.primary_thread_id = Some(thread_id);
        app.active_thread_id = Some(thread_id);
        app.primary_session_configured = Some(session);

        assert_eq!(
            app.worktree_switch_mode().await,
            WorktreeSwitchMode::Fork(thread_id)
        );
    }

    #[tokio::test]
    async fn worktree_switch_mode_uses_active_non_primary_thread_session() {
        let temp_dir = TempDir::new().expect("temp dir");
        let primary_thread_id = ThreadId::new();
        let active_thread_id = ThreadId::new();
        let active_rollout_path = temp_dir.path().join("active-rollout.jsonl");
        std::fs::write(&active_rollout_path, "{}\\n").expect("write rollout");
        let active_session = test_thread_session(
            active_thread_id,
            temp_dir.path().join("active"),
            active_rollout_path,
        );
        let mut app = crate::app::test_support::make_test_app().await;
        app.primary_thread_id = Some(primary_thread_id);
        app.active_thread_id = Some(active_thread_id);
        app.primary_session_configured = Some(test_thread_session(
            primary_thread_id,
            temp_dir.path().join("primary"),
            temp_dir.path().join("missing-primary-rollout.jsonl"),
        ));
        app.thread_event_channels.insert(
            active_thread_id,
            ThreadEventChannel::new_with_session(
                THREAD_EVENT_CHANNEL_CAPACITY,
                active_session,
                Vec::new(),
            ),
        );

        assert_eq!(
            app.worktree_switch_mode().await,
            WorktreeSwitchMode::Fork(active_thread_id)
        );
    }

    #[test]
    fn worktree_session_message_describes_forked_workspace() {
        let info = test_worktree_info(
            WorktreeSource::Cli,
            Some("fcoury/demo".to_string()),
            /*dirty*/ false,
        );

        assert_eq!(
            worktree_session_message(&info, WorktreeSessionTransition::Forked),
            (
                "Forked into cli worktree fcoury/demo · clean · codex".to_string(),
                "/repo/codex.fcoury-demo".to_string()
            )
        );
    }

    #[test]
    fn worktree_session_message_describes_started_dirty_workspace() {
        let info = test_worktree_info(
            WorktreeSource::App,
            /*branch*/ None,
            /*dirty*/ true,
        );

        assert_eq!(
            worktree_session_message(&info, WorktreeSessionTransition::Started),
            (
                "Started session in app worktree app-worktree · dirty · codex".to_string(),
                "/repo/codex.fcoury-demo".to_string()
            )
        );
    }

    fn test_thread_session(
        thread_id: ThreadId,
        cwd: PathBuf,
        rollout_path: PathBuf,
    ) -> ThreadSessionState {
        ThreadSessionState {
            thread_id,
            forked_from_id: None,
            fork_parent_title: None,
            thread_name: None,
            model: "gpt-test".to_string(),
            model_provider_id: "test-provider".to_string(),
            service_tier: None,
            approval_policy: AskForApproval::Never,
            approvals_reviewer: ApprovalsReviewer::User,
            permission_profile: PermissionProfile::read_only(),
            active_permission_profile: None,
            cwd: AbsolutePathBuf::try_from(cwd).expect("absolute cwd"),
            instruction_source_paths: Vec::new(),
            reasoning_effort: None,
            message_history: None,
            network_proxy: None,
            rollout_path: Some(rollout_path),
        }
    }

    fn test_worktree_info(
        source: WorktreeSource,
        branch: Option<String>,
        dirty: bool,
    ) -> WorktreeInfo {
        let path = PathBuf::from("/repo/codex.fcoury-demo");
        WorktreeInfo {
            id: "repo-id".to_string(),
            name: "app-worktree".to_string(),
            slug: "fcoury-demo".to_string(),
            source,
            location: WorktreeLocation::Sibling,
            repo_name: "codex".to_string(),
            repo_root: path.clone(),
            common_git_dir: PathBuf::from("/repo/codex/.git"),
            worktree_git_root: path.clone(),
            workspace_cwd: path,
            original_relative_cwd: PathBuf::new(),
            branch,
            head: Some("abcdef".to_string()),
            owner_thread_id: None,
            metadata_path: PathBuf::from("/repo/codex/.git/codex-worktree.json"),
            dirty: DirtyState {
                has_staged_changes: false,
                has_unstaged_changes: dirty,
                has_untracked_files: false,
            },
        }
    }
}
