//! Platform-specific app actions and small global shortcuts.
//!
//! This module owns platform state used by `App`, the side-conversation return shortcut predicate,
//! and Windows sandbox helper actions that are compiled only on Windows.

use super::*;
#[cfg(all(test, not(target_os = "windows")))]
use crate::app_event::WindowsSandboxEnableMode;
#[cfg(target_os = "windows")]
use codex_app_server_protocol::WindowsSandboxSetupMode;
#[cfg(any(target_os = "windows", test))]
use codex_utils_approval_presets::ApprovalPreset;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WindowsSandboxHost {
    Local,
    Mixed,
    Remote,
    Unknown,
}

#[derive(Default)]
pub(super) struct WindowsSandboxState {
    pub(super) setup_started_at: Option<Instant>,
    #[cfg_attr(not(any(target_os = "windows", test)), allow(dead_code))]
    pub(super) prompt_after_trust: bool,
    #[cfg(any(target_os = "windows", test))]
    pub(super) pending_setup: Option<(
        WindowsSandboxEnableMode,
        ApprovalPreset,
        Option<PermissionProfileSelection>,
    )>,
}

#[cfg(target_os = "windows")]
pub(super) async fn windows_sandbox_ready(app_server: &mut AppServerSession) -> bool {
    let request_id = app_server.next_request_id();
    matches!(
        tokio::time::timeout(
            Duration::from_secs(5),
            app_server
                .request_handle()
                .request_typed(ClientRequest::WindowsSandboxReadiness {
                    request_id,
                    params: None,
                }),
        )
        .await,
        Ok(Ok(
            codex_app_server_protocol::WindowsSandboxReadinessResponse {
                status: codex_app_server_protocol::WindowsSandboxReadiness::Ready,
            }
        ))
    )
}

impl App {
    pub(super) fn windows_sandbox_host(&self) -> WindowsSandboxHost {
        if self.app_server_target.uses_remote_workspace() {
            WindowsSandboxHost::Remote
        } else {
            self.chat_widget.windows_sandbox_host
        }
    }

    /// A local app server owns setup for both embedded and daemon connections.
    pub(super) fn windows_sandbox_setup_is_local(&self) -> bool {
        self.chat_widget.windows_sandbox_local_server
            && self.windows_sandbox_host() == WindowsSandboxHost::Local
    }

    pub(super) fn windows_sandbox_blocks_thread_switch(&self) -> bool {
        #[cfg(any(target_os = "windows", test))]
        {
            self.windows_sandbox_setup_is_local()
                && (self.windows_sandbox.pending_setup.is_some()
                    || self.windows_sandbox.setup_started_at.is_some()
                    || self.chat_widget.initial_user_message.is_some()
                        && self.chat_widget.windows_sandbox_config.requires_elevated()
                        && !self.chat_widget.windows_sandbox_elevated_setup_complete)
        }
        #[cfg(not(any(target_os = "windows", test)))]
        {
            false
        }
    }

    #[cfg(any(target_os = "windows", test))]
    pub(super) async fn refresh_windows_sandbox_for_thread(
        &mut self,
        app_server: &mut AppServerSession,
        thread_id: ThreadId,
    ) {
        if self.chat_widget.thread_id() == Some(thread_id)
            && !self.app_server_target.uses_remote_workspace()
            && self.refresh_windows_sandbox_config(app_server).await
        {
            #[cfg(target_os = "windows")]
            if self.windows_sandbox_setup_is_local()
                && self.chat_widget.windows_sandbox_config.requires_elevated()
                && !self.chat_widget.windows_sandbox_elevated_setup_complete
            {
                self.chat_widget.windows_sandbox_elevated_setup_complete =
                    windows_sandbox_ready(app_server).await;
            }
            let show_nux = std::mem::take(&mut self.windows_sandbox.prompt_after_trust)
                && self.chat_widget.windows_sandbox_config.mode.is_none()
                || self.chat_widget.windows_sandbox_config.requires_elevated();
            if self.windows_sandbox_setup_is_local() {
                self.chat_widget.maybe_prompt_windows_sandbox_enable(
                    show_nux && !self.chat_widget.windows_sandbox_elevated_setup_complete,
                );
            } else if self.windows_sandbox_host() == WindowsSandboxHost::Mixed && show_nux {
                self.app_event_tx.send(AppEvent::InsertHistoryCell(Box::new(
                    history_cell::StartupWarningsCell::new(vec![
                        "Windows sandbox setup is unavailable when local and remote executors are configured together."
                            .to_string(),
                    ]),
                )));
            }
            self.chat_widget.submit_initial_user_message_if_pending();
        }
    }

    #[cfg(any(target_os = "windows", test))]
    pub(super) async fn refresh_windows_sandbox_config(
        &mut self,
        app_server: &AppServerSession,
    ) -> bool {
        match crate::windows_sandbox::WindowsSandboxConfig::read(
            app_server.request_handle(),
            self.chat_widget.config_ref().cwd.display().to_string(),
        )
        .await
        {
            Ok(state) => {
                self.chat_widget.windows_sandbox_config = state;
                self.chat_widget
                    .set_windows_sandbox_mode(self.chat_widget.windows_sandbox_config.mode);
                true
            }
            Err(_) => {
                self.chat_widget.windows_sandbox_config = Default::default();
                self.chat_widget
                    .retain_input_after_failed_permission_selection();
                self.chat_widget.set_windows_sandbox_mode(/*mode*/ None);
                self.chat_widget.add_error_message("Could not read Windows sandbox configuration and requirements from the app server.".to_string());
                false
            }
        }
    }

    #[cfg(target_os = "windows")]
    pub(super) async fn begin_windows_sandbox_setup(
        &mut self,
        app_server: &mut AppServerSession,
        preset: ApprovalPreset,
        profile_selection: Option<PermissionProfileSelection>,
        mode: WindowsSandboxEnableMode,
    ) {
        let setup_mode = match mode {
            WindowsSandboxEnableMode::Elevated => WindowsSandboxSetupMode::Elevated,
            WindowsSandboxEnableMode::Legacy => WindowsSandboxSetupMode::Unelevated,
        };
        if self.windows_sandbox.pending_setup.is_some() {
            if self.windows_sandbox.setup_started_at.is_none() {
                self.chat_widget.add_info_message(
                    "Windows sandbox setup is still running. Restart Codex to retry.".to_string(),
                    /*hint*/ None,
                );
            }
            return;
        }
        if !self.refresh_windows_sandbox_config(app_server).await {
            return;
        }
        if !self.chat_widget.windows_sandbox_config.allows(setup_mode) {
            self.chat_widget
                .retain_input_after_failed_permission_selection();
            self.chat_widget.add_info_message(
                "That Windows sandbox option is disallowed by requirements.".to_string(),
                /*hint*/ None,
            );
            return;
        }

        self.windows_sandbox.pending_setup = Some((mode, preset, profile_selection));
        self.chat_widget.show_windows_sandbox_setup_status();
        self.windows_sandbox.setup_started_at = Some(Instant::now());
        let request_id = app_server.next_request_id();
        let response = tokio::time::timeout(
            std::time::Duration::from_secs(15),
            app_server
                .request_handle()
                .request_typed(ClientRequest::WindowsSandboxSetupStart {
                    request_id,
                    params: codex_app_server_protocol::WindowsSandboxSetupStartParams {
                        mode: setup_mode,
                        cwd: Some(self.chat_widget.config_ref().cwd.clone()),
                    },
                }),
        )
        .await;
        self.finish_windows_sandbox_setup_start(response);
    }

    #[cfg(any(target_os = "windows", test))]
    pub(super) fn finish_windows_sandbox_setup_start(
        &mut self,
        response: std::result::Result<
            std::result::Result<
                codex_app_server_protocol::WindowsSandboxSetupStartResponse,
                TypedRequestError,
            >,
            tokio::time::error::Elapsed,
        >,
    ) {
        match response {
            Ok(Ok(codex_app_server_protocol::WindowsSandboxSetupStartResponse {
                started: true,
            })) => {}
            Err(_) => {
                self.chat_widget.add_error_message(
                    "Windows sandbox setup request timed out. Waiting for completion; restart Codex if it does not finish."
                        .to_string(),
                );
            }
            Ok(Err(
                TypedRequestError::Transport { .. } | TypedRequestError::Deserialize { .. },
            )) => {
                self.chat_widget.add_error_message(
                    "Windows sandbox setup response was lost. Waiting for completion or reconnection."
                        .to_string(),
                );
            }
            Ok(result) => {
                self.windows_sandbox.pending_setup = None;
                self.chat_widget
                    .retain_input_after_failed_permission_selection();
                self.chat_widget.clear_windows_sandbox_setup_status();
                self.windows_sandbox.setup_started_at = None;
                let message = match result {
                    Err(TypedRequestError::Server { source, .. }) if source.code == -32601 => {
                        "Update the local app server to set up the Windows sandbox.".to_string()
                    }
                    Err(_) => "Windows sandbox setup failed.".to_string(),
                    Ok(_) => "Windows sandbox setup did not start.".to_string(),
                };
                self.chat_widget.add_error_message(message);
            }
        }
    }
}

pub(super) fn side_return_shortcut_matches(key_event: KeyEvent) -> bool {
    matches!(
        key_event,
        KeyEvent {
            code: KeyCode::Char(c),
            modifiers,
            kind: KeyEventKind::Press,
            ..
        } if modifiers.contains(KeyModifiers::CONTROL)
            && (c.eq_ignore_ascii_case(&'c') || c.eq_ignore_ascii_case(&'d'))
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn side_return_shortcuts_match_ctrl_c_and_ctrl_d() {
        assert!(side_return_shortcut_matches(KeyEvent::new(
            KeyCode::Char('c'),
            KeyModifiers::CONTROL,
        )));
        assert!(side_return_shortcut_matches(KeyEvent::new(
            KeyCode::Char('C'),
            KeyModifiers::CONTROL,
        )));
        assert!(side_return_shortcut_matches(KeyEvent::new(
            KeyCode::Char('d'),
            KeyModifiers::CONTROL,
        )));
        assert!(side_return_shortcut_matches(KeyEvent::new(
            KeyCode::Char('D'),
            KeyModifiers::CONTROL,
        )));
        assert!(!side_return_shortcut_matches(KeyEvent::new_with_kind(
            KeyCode::Esc,
            KeyModifiers::NONE,
            KeyEventKind::Press,
        )));
        assert!(!side_return_shortcut_matches(KeyEvent::new_with_kind(
            KeyCode::Esc,
            KeyModifiers::NONE,
            KeyEventKind::Release,
        )));
    }
}
