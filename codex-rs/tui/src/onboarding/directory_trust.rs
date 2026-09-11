//! Folder consent for startup and task navigation.
//! Local daemon resumes recheck folders changed during consent. Remote checks
//! retain their existing explicit-cwd scope until authoritative roots are available.

use super::OnboardingResult;
use super::OnboardingScreen;
use super::Step;
use super::run_onboarding_screen;
use crate::AppServerTarget;
use crate::app_server_session::AppServerSession;
use crate::config_update::ProjectTrustHost;
use crate::config_update::RemoteProjectTrust;
use crate::config_update::read_remote_project_trust;
use crate::legacy_core::config::Config;
use crate::onboarding::trust_directory::TrustCancelAction;
use crate::onboarding::trust_directory::TrustDirectorySelection;
use crate::onboarding::trust_directory::TrustDirectoryWidget;
use crate::startup_draft::StartupDraftPump;
use crate::tui::Tui;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::Thread;
use codex_app_server_protocol::ThreadReadParams;
use codex_app_server_protocol::ThreadReadResponse;
use codex_exec_server::LOCAL_FS;
use codex_git_utils::resolve_root_git_project_for_trust;
use codex_protocol::config_types::TrustLevel;
use color_eyre::eyre::Result;
use std::collections::VecDeque;
use std::path::Path;
use uuid::Uuid;

pub(crate) async fn check_directory_trust(
    tui: &mut Tui,
    app_server: &AppServerSession,
    config: &Config,
    target: &AppServerTarget,
    cwd: &Path,
    resumed_thread: Option<&Thread>,
    mut startup_draft: Option<&mut StartupDraftPump>,
) -> Result<OnboardingResult> {
    let connected = !matches!(target, AppServerTarget::Embedded);
    let mut consent = OnboardingResult::default();
    // Another client can load the saved task while consent is pending. Check both folders.
    let saved_cwd = resumed_thread
        .filter(|thread| {
            matches!(target, AppServerTarget::LocalDaemon { .. }) && thread.cwd.as_path() != cwd
        })
        .map(|thread| thread.cwd.as_path());
    let mut pending_cwds: VecDeque<_> = std::iter::once(cwd)
        .chain(saved_cwd)
        .map(Path::to_path_buf)
        .collect();
    let mut checked_cwds = Vec::new();
    while let Some(cwd) = pending_cwds.pop_front() {
        if checked_cwds.contains(&cwd) {
            continue;
        }
        checked_cwds.push(cwd.clone());
        let cwd = cwd.as_path();
        let lookup = async {
            if connected {
                let host = if matches!(target, AppServerTarget::LocalDaemon { .. }) {
                    ProjectTrustHost::Local
                } else {
                    ProjectTrustHost::Remote
                };
                read_remote_project_trust(app_server.request_handle(), cwd, host).await
            } else if config.active_project.trust_level == Some(TrustLevel::Trusted) {
                Ok(None)
            } else {
                Ok(Some(RemoteProjectTrust {
                    cwd: cwd.to_path_buf(),
                    trust_target: resolve_root_git_project_for_trust(
                        LOCAL_FS.as_ref(),
                        &config.cwd,
                    )
                    .await
                    .map(Into::into)
                    .unwrap_or_else(|| cwd.to_path_buf()),
                    trust_level: config.active_project.trust_level,
                }))
            }
        };
        let project = if let Some(draft) = startup_draft.as_deref_mut() {
            draft.run_until(tui, lookup).await??
        } else {
            lookup.await?
        };

        let Some(project) = project else {
            continue;
        };
        // Remote connections retain the existing behavior for saved untrusted folders.
        if target.uses_remote_workspace() && project.trust_level == Some(TrustLevel::Untrusted) {
            continue;
        }
        let remote_trust_key =
            connected.then(|| project.trust_target.to_string_lossy().into_owned());
        if let Some(draft) = startup_draft.as_deref_mut() {
            draft.flush_pending_events(tui).await?;
        }
        let screen = OnboardingScreen {
            request_frame: tui.frame_requester(),
            steps: vec![Step::TrustDirectory(TrustDirectoryWidget {
                restricted: project.trust_level == Some(TrustLevel::Untrusted),
                existing_task: connected && resumed_thread.is_some(),
                cancel: if connected {
                    TrustCancelAction::AgentsOverview
                } else {
                    TrustCancelAction::Quit
                },
                cwd: project.cwd,
                trust_target: project.trust_target,
                show_windows_create_sandbox_hint: false,
                should_quit: false,
                selection: None,
                highlighted: TrustDirectorySelection::Trust,
                error: None,
            })],
            remote_trust_key,
            is_done: false,
            should_exit: false,
        };
        let result = run_onboarding_screen(
            screen,
            Some(app_server.request_handle()),
            /*app_server*/ None,
            tui,
        )
        .await?;
        tui.terminal.clear()?;
        tui.frame_requester().schedule_frame();
        if result.should_exit {
            return Ok(result);
        }
        consent.directory_trust_persisted |= result.directory_trust_persisted;
        if matches!(target, AppServerTarget::LocalDaemon { .. })
            && let Some(thread) = resumed_thread
        {
            // Another client may have reopened this task in a different folder during consent.
            let request_handle = app_server.request_handle();
            let recheck = request_handle.request_typed(ClientRequest::ThreadRead {
                request_id: RequestId::String(format!("tui-trust-recheck-{}", Uuid::new_v4())),
                params: ThreadReadParams {
                    thread_id: thread.id.clone(),
                    include_turns: false,
                },
            });
            let latest: ThreadReadResponse = if let Some(draft) = startup_draft.as_deref_mut() {
                draft.run_until(tui, recheck).await??
            } else {
                recheck.await?
            };
            pending_cwds.push_back(latest.thread.cwd.to_path_buf());
        }
    }
    Ok(consent)
}
