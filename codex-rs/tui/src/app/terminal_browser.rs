//! App-owned lifecycle for the Carbonyl-backed terminal browser.
//!
//! Rendering and input adaptation live in `crate::terminal_browser`. This module keeps the
//! process, permission policy, and app-server tool surface bound to one displayed conversation.

use std::sync::Arc;

use codex_features::Feature;
use codex_protocol::ThreadId;
use codex_terminal_browser::BrowserLaunchContext;
use codex_terminal_browser::BrowserNetworkPolicy;
use codex_terminal_browser::BrowserStatus;
use codex_terminal_browser::TerminalBrowser;
use codex_utils_absolute_path::AbsolutePathBuf;

use super::App;
use crate::app_event::AppEvent;
use crate::app_event::TerminalBrowserProfileApproval;
use crate::app_event::TerminalBrowserProfileCommand;
use crate::terminal_browser::profile_approval_view_params;

impl App {
    fn terminal_browser_network_policy(&self) -> BrowserNetworkPolicy {
        let config = self.chat_widget.config_ref();
        if config.permissions.network_sandbox_policy().is_enabled()
            && config.permissions.network.is_none()
        {
            BrowserNetworkPolicy::Direct
        } else {
            BrowserNetworkPolicy::Disabled
        }
    }

    fn terminal_browser_enabled(&self) -> bool {
        self.chat_widget
            .config_ref()
            .features
            .enabled(Feature::TerminalBrowser)
            && !self.app_server_target.uses_remote_workspace()
    }

    fn discover_terminal_browser(&self) -> Arc<TerminalBrowser> {
        let config = self.chat_widget.config_ref();
        let codex_linux_sandbox_exe = config.codex_linux_sandbox_exe.as_ref().and_then(|path| {
            match AbsolutePathBuf::from_absolute_path_checked(path) {
                Ok(path) => Some(path),
                Err(error) => {
                    tracing::warn!(
                        %error,
                        path = %path.display(),
                        "invalid Linux sandbox helper path for terminal browser"
                    );
                    None
                }
            }
        });
        Arc::new(TerminalBrowser::discover_with_launch_context(
            BrowserLaunchContext {
                codex_home: Some(config.codex_home.clone()),
                workspace_root: Some(config.cwd.clone()),
                codex_linux_sandbox_exe,
                use_legacy_landlock: config.features.use_legacy_landlock(),
            },
        ))
    }

    async fn terminal_browser_for_thread(
        &mut self,
        owner_thread_id: ThreadId,
    ) -> Option<Arc<TerminalBrowser>> {
        if !self.terminal_browser_enabled() {
            self.reset_terminal_browser_for_thread_change().await;
            return None;
        }

        if self.terminal_browser_owner_thread_id != Some(owner_thread_id) {
            self.reset_terminal_browser_for_thread_change().await;
        }

        if self.terminal_browser.is_none() {
            let browser = self.discover_terminal_browser();
            let mut updates = browser.subscribe();
            let app_event_tx = self.app_event_tx.clone();
            tokio::spawn(async move {
                while updates.changed().await.is_ok() {
                    app_event_tx.send(AppEvent::TerminalBrowserUpdated);
                }
            });
            self.terminal_browser_generation =
                self.terminal_browser_generation.wrapping_add(/*rhs*/ 1);
            self.terminal_browser = Some(browser);
            self.terminal_browser_owner_thread_id = Some(owner_thread_id);
        }

        let network_policy = self.terminal_browser_network_policy();
        let browser = self.terminal_browser.as_ref()?.clone();
        browser.set_network_policy(network_policy).await;
        Some(browser)
    }

    async fn terminal_browser_for_active_thread(&mut self) -> Option<Arc<TerminalBrowser>> {
        let owner_thread_id = self.current_displayed_thread_id()?;
        self.terminal_browser_for_thread(owner_thread_id).await
    }

    pub(super) fn terminal_browser_request_matches_active_thread(
        &self,
        request_thread_id: &str,
    ) -> bool {
        terminal_browser_request_matches_thread(
            self.current_displayed_thread_id(),
            request_thread_id,
        )
    }

    pub(crate) fn terminal_browser_owned_by_current_thread(&self) -> bool {
        self.terminal_browser.is_some()
            && self
                .terminal_browser_owner_thread_id
                .is_some_and(|owner| self.current_displayed_thread_id() == Some(owner))
    }

    pub(super) async fn terminal_browser_for_active_request(
        &mut self,
    ) -> Option<Arc<TerminalBrowser>> {
        self.terminal_browser_for_active_thread().await
    }

    pub(super) async fn reconcile_terminal_browser_network_policy(&mut self) {
        let Some(browser) = self.terminal_browser.as_ref().cloned() else {
            return;
        };
        let network_policy = self.terminal_browser_network_policy();
        let network_disabled = matches!(network_policy, BrowserNetworkPolicy::Disabled);
        browser.set_network_policy(network_policy).await;
        if network_disabled {
            browser.set_visibility(/*visible*/ false);
        }
    }

    pub(super) async fn toggle_terminal_browser(&mut self) {
        let Some(browser) = self.terminal_browser_for_active_thread().await else {
            self.chat_widget.add_info_message(
                "Terminal browser is disabled for this session.".to_string(),
                Some(
                    "Enable the Terminal browser experiment, then start a new session.".to_string(),
                ),
            );
            return;
        };
        if !browser.is_available() {
            let message = match browser.view().status {
                BrowserStatus::Unavailable { reason } => reason,
                _ => "Carbonyl could not be discovered on this machine.".to_string(),
            };
            self.chat_widget
                .add_error_message(format!("Terminal browser is unavailable: {message}"));
            return;
        }

        browser.set_visibility(!browser.view().visible);
        self.app_event_tx.send(AppEvent::TerminalBrowserUpdated);
    }

    pub(super) fn close_terminal_browser(&mut self) {
        let Some(browser) = self.terminal_browser.as_ref().cloned() else {
            self.chat_widget.add_info_message(
                "Terminal browser is not enabled.".to_string(),
                /*hint*/ None,
            );
            return;
        };
        let app_event_tx = self.app_event_tx.clone();
        tokio::spawn(async move {
            browser.close().await;
            app_event_tx.send(AppEvent::TerminalBrowserClosed);
        });
    }

    fn take_terminal_browser_for_thread_change(&mut self) -> Option<Arc<TerminalBrowser>> {
        self.terminal_browser_owner_thread_id = None;
        self.terminal_browser_generation =
            self.terminal_browser_generation.wrapping_add(/*rhs*/ 1);
        let browser = self.terminal_browser.take()?;
        browser.set_visibility(/*visible*/ false);
        Some(browser)
    }

    pub(super) async fn reset_terminal_browser_for_thread_change(&mut self) {
        let Some(browser) = self.take_terminal_browser_for_thread_change() else {
            return;
        };
        browser.close().await;
        self.app_event_tx.send(AppEvent::TerminalBrowserClosed);
    }

    pub(super) fn reset_terminal_browser_for_focus_change(&mut self) {
        let Some(browser) = self.take_terminal_browser_for_thread_change() else {
            return;
        };
        let app_event_tx = self.app_event_tx.clone();
        app_event_tx.send(AppEvent::TerminalBrowserClosed);
        tokio::spawn(async move {
            browser.close().await;
        });
    }

    pub(super) async fn doctor_terminal_browser(&mut self) {
        let Some(browser) = self.terminal_browser_for_active_thread().await else {
            self.chat_widget
                .add_error_message("Terminal browser is not enabled.".to_string());
            return;
        };
        let app_event_tx = self.app_event_tx.clone();
        tokio::spawn(async move {
            let report = browser.doctor().await;
            app_event_tx.send(AppEvent::TerminalBrowserDoctorCompleted {
                healthy: report.healthy,
                summary: report.summary,
            });
        });
    }

    pub(super) async fn manage_terminal_browser_profile(
        &mut self,
        command: TerminalBrowserProfileCommand,
    ) {
        let Some(browser) = self.terminal_browser_for_active_thread().await else {
            self.chat_widget
                .add_error_message("Terminal browser is not enabled.".to_string());
            return;
        };
        let result = match command {
            TerminalBrowserProfileCommand::List => browser.profiles().map(|profiles| {
                let selected = browser
                    .selected_profile()
                    .map_or_else(|| "ephemeral".to_string(), |name| format!("named `{name}`"));
                format!(
                    "Terminal-browser profiles: {}. Current profile: {selected}.",
                    if profiles.is_empty() {
                        "none".to_string()
                    } else {
                        profiles.join(", ")
                    }
                )
            }),
            TerminalBrowserProfileCommand::Create(name) => browser
                .create_profile(&name)
                .await
                .map(|()| format!("Created and selected browser profile `{name}`.")),
            TerminalBrowserProfileCommand::Use(name) => browser
                .select_profile(&name)
                .await
                .map(|()| format!("Selected browser profile `{name}`.")),
            TerminalBrowserProfileCommand::Ephemeral => browser
                .select_ephemeral_profile()
                .await
                .map(|()| "Selected a new ephemeral browser profile.".to_string()),
            TerminalBrowserProfileCommand::Forget(name) => browser
                .forget_profile(&name)
                .await
                .map(|()| format!("Permanently deleted browser profile `{name}`.")),
        };
        match result {
            Ok(message) => self.chat_widget.add_info_message(message, /*hint*/ None),
            Err(error) => self
                .chat_widget
                .add_error_message(format!("Browser profile command failed: {error}")),
        }
    }

    pub(super) fn terminal_browser_profile_approval(
        &self,
        command: TerminalBrowserProfileCommand,
    ) -> Option<TerminalBrowserProfileApproval> {
        let thread_id = self.terminal_browser_owner_thread_id?;
        self.terminal_browser_owned_by_current_thread()
            .then_some(TerminalBrowserProfileApproval {
                command,
                thread_id,
                generation: self.terminal_browser_generation,
            })
    }

    fn terminal_browser_profile_approval_is_current(
        &self,
        approval: &TerminalBrowserProfileApproval,
    ) -> bool {
        self.terminal_browser.is_some()
            && self.current_displayed_thread_id() == Some(approval.thread_id)
            && self.terminal_browser_owner_thread_id == Some(approval.thread_id)
            && self.terminal_browser_generation == approval.generation
    }

    pub(super) fn show_terminal_browser_profile_approval(
        &mut self,
        approval: TerminalBrowserProfileApproval,
    ) {
        if !self.terminal_browser_profile_approval_is_current(&approval) {
            return;
        }
        self.chat_widget
            .show_selection_view(profile_approval_view_params(approval));
    }

    pub(super) async fn approve_terminal_browser_profile(
        &mut self,
        approval: TerminalBrowserProfileApproval,
    ) {
        if !self.terminal_browser_profile_approval_is_current(&approval) {
            self.chat_widget.add_info_message(
                "Browser profile request expired after the active conversation changed."
                    .to_string(),
                /*hint*/ None,
            );
            return;
        }
        self.manage_terminal_browser_profile(approval.command).await;
    }

    pub(super) async fn toggle_terminal_browser_control(&mut self) {
        let Some(browser) = self.terminal_browser_for_active_thread().await else {
            self.chat_widget
                .add_error_message("Terminal browser is not enabled.".to_string());
            return;
        };
        let app_event_tx = self.app_event_tx.clone();
        tokio::spawn(async move {
            let result = browser.toggle_human_control().await;
            app_event_tx.send(AppEvent::TerminalBrowserControlCompleted {
                error: result.err().map(|error| error.to_string()),
            });
        });
    }
}

fn terminal_browser_request_matches_thread(
    active_thread_id: Option<ThreadId>,
    request_thread_id: &str,
) -> bool {
    active_thread_id.is_some_and(|thread_id| thread_id.to_string() == request_thread_id)
}

#[cfg(test)]
#[path = "terminal_browser_tests.rs"]
mod tests;
