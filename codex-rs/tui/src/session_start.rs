//! Interactive recovery when starting an archived session.

use crate::app::AppExitInfo;
use crate::app::ExitReason;
use crate::app_server_session::AppServerSession;
use crate::app_server_session::AppServerStartedThread;
use crate::app_server_session::ResumeModelSettings;
use crate::legacy_core::config::Config;
use crate::resume_picker::SessionTarget;
use crate::unarchive_prompt::UnarchiveChoice;
use color_eyre::Result;
use color_eyre::eyre::WrapErr;

#[derive(Clone, Copy)]
pub(crate) enum SessionStartAction {
    Resume(ResumeModelSettings),
    Fork,
}

impl SessionStartAction {
    pub(crate) fn verb(self) -> &'static str {
        match self {
            Self::Resume(_) => "resume",
            Self::Fork => "fork",
        }
    }

    async fn start(
        self,
        app_server: &mut AppServerSession,
        config: &Config,
        target: &SessionTarget,
    ) -> Result<AppServerStartedThread> {
        match self {
            Self::Resume(settings) => {
                app_server
                    .resume_thread(config.clone(), target.thread_id, settings)
                    .await
            }
            Self::Fork => {
                app_server
                    .fork_thread(config.clone(), target.thread_id)
                    .await
            }
        }
    }
}

pub(crate) async fn complete_session_start(
    app_server: &mut AppServerSession,
    config: &Config,
    target: &SessionTarget,
    action: SessionStartAction,
    initial_result: Result<AppServerStartedThread>,
    confirm: impl AsyncFnOnce() -> Result<UnarchiveChoice>,
) -> Result<Option<AppServerStartedThread>> {
    match initial_result {
        Ok(started) => return Ok(Some(started)),
        Err(err) => {
            // Match the requested ID as well as the server's archive guidance: an unrelated
            // startup failure must never cause us to unarchive a session.
            let archived_prefix = format!("session {} is archived. ", target.thread_id);
            if !archived_session_guidance(&err)
                .is_some_and(|message| message.starts_with(&archived_prefix))
            {
                return Err(session_start_error(action.verb(), target, err));
            }
        }
    }

    if confirm().await? == UnarchiveChoice::Cancel {
        return Ok(None);
    }

    app_server
        .thread_unarchive(target.thread_id)
        .await
        .wrap_err_with(|| format!("Failed to unarchive session {}", target.thread_id))?;
    // Retry by ID, not by the old rollout path, which unarchiving may have moved.
    action
        .start(app_server, config, target)
        .await
        .map(Some)
        .map_err(|err| session_start_error(action.verb(), target, err))
}

pub(crate) async fn cancel_session_start(app_server: AppServerSession) -> AppExitInfo {
    if let Err(err) = app_server.shutdown().await {
        tracing::warn!("app-server shutdown failed: {err}");
    }
    AppExitInfo {
        token_usage: Default::default(),
        thread_id: None,
        resume_hint: None,
        update_action: None,
        exit_reason: ExitReason::UserRequested,
    }
}

fn session_start_error(
    action: &str,
    target_session: &SessionTarget,
    err: color_eyre::Report,
) -> color_eyre::Report {
    if let Some(message) = archived_session_guidance(&err) {
        return color_eyre::eyre::eyre!("{message}");
    }

    let target_label = target_session.display_label();
    color_eyre::eyre::eyre!("Failed to {action} session from {target_label}: {err}")
}

fn archived_session_guidance(err: &color_eyre::Report) -> Option<String> {
    let err = err.to_string();
    let message = &err[err.find("session ")?..];
    if !message.contains(" is archived. Run `codex unarchive ") {
        return None;
    }
    let message = message
        .split_once(" (code ")
        .map_or(message, |(message, _)| message);
    Some(message.to_string())
}

#[cfg(test)]
#[path = "session_start_tests.rs"]
mod tests;
