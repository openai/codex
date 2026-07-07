//! App-owned lifecycle for the Carbonyl-backed terminal browser.
//!
//! Rendering and input adaptation live in `crate::terminal_browser`. This module keeps the
//! process, permission policy, and app-server tool surface bound to one displayed conversation.

use std::sync::Arc;

use codex_app_server_protocol::RequestId as AppServerRequestId;
use codex_features::Feature;
use codex_protocol::ThreadId;
use codex_terminal_browser::BrowserLaunchContext;
use codex_terminal_browser::BrowserMouseInput;
use codex_terminal_browser::BrowserNetworkPolicy;
use codex_terminal_browser::BrowserStatus;
use codex_terminal_browser::BrowserToolOutput;
use codex_terminal_browser::BrowserView;
use codex_terminal_browser::TerminalBrowser;
use codex_utils_absolute_path::AbsolutePathBuf;
use crossterm::event::KeyEvent;
use crossterm::event::MouseEvent;
use ratatui::layout::Rect;
use tokio::sync::oneshot;

use super::App;
use super::owned_screen_frame::OwnedScreenRightRailContent;
use crate::app_event::AppEvent;
use crate::app_event::OwnedScreenPanel;
use crate::app_event::TerminalBrowserControlTarget;
use crate::app_event::TerminalBrowserProfileApproval;
use crate::app_event::TerminalBrowserProfileCommand;
use crate::terminal_browser::BrowserMouseRoute;
use crate::terminal_browser::TerminalBrowserNetworkAvailability;
use crate::terminal_browser::browser_key_input;
use crate::terminal_browser::browser_mouse_input;
use crate::terminal_browser::browser_mouse_route;
use crate::terminal_browser::dynamic_tool_response;
use crate::terminal_browser::profile_approval_view_params;
use crate::tui::MousePrimaryEvent;
use crate::tui::MousePrimaryEventKind;

pub(super) struct ReopenableTerminalBrowser {
    browser: Arc<TerminalBrowser>,
    closed: oneshot::Receiver<()>,
}

pub(super) struct TerminalBrowserControlClick {
    browser: Arc<TerminalBrowser>,
    target: TerminalBrowserControlTarget,
    inputs: [BrowserMouseInput; 2],
}

pub(super) struct PendingTerminalBrowserOpen {
    request_id: AppServerRequestId,
    session_key: String,
    arguments: serde_json::Value,
}

impl ReopenableTerminalBrowser {
    pub(super) fn terminate(&self) {
        self.browser.terminate();
    }
}

impl App {
    pub(super) fn defer_terminal_browser_open(
        &mut self,
        request_id: AppServerRequestId,
        session_key: String,
        arguments: serde_json::Value,
    ) -> bool {
        if self.terminal_browser_pending_open.is_some() {
            return false;
        }
        self.terminal_browser_pending_open = Some(PendingTerminalBrowserOpen {
            request_id,
            session_key,
            arguments,
        });
        true
    }

    pub(super) fn has_pending_terminal_browser_open(&self) -> bool {
        self.terminal_browser_pending_open.is_some()
    }

    pub(super) fn discard_pending_terminal_browser_open(
        &mut self,
        request_id: &AppServerRequestId,
    ) -> bool {
        let matches_request = self
            .terminal_browser_pending_open
            .as_ref()
            .is_some_and(|pending| &pending.request_id == request_id);
        if matches_request {
            self.terminal_browser_pending_open = None;
        }
        matches_request
    }

    fn complete_terminal_browser_open(
        &self,
        request_id: AppServerRequestId,
        result: anyhow::Result<BrowserToolOutput>,
    ) {
        self.app_event_tx
            .unscoped()
            .send(AppEvent::TerminalBrowserToolCompleted {
                request_id,
                response: dynamic_tool_response(result),
                profile_approval: None,
            });
    }

    pub(super) async fn resolve_pending_terminal_browser_open(&mut self, allow: bool) {
        let Some(pending) = self.terminal_browser_pending_open.take() else {
            return;
        };
        let PendingTerminalBrowserOpen {
            request_id,
            session_key,
            arguments,
        } = pending;
        if !allow {
            self.complete_terminal_browser_open(
                request_id,
                Err(anyhow::anyhow!(
                    "terminal browser network access remains disabled by the active permission profile"
                )),
            );
            return;
        }
        if !self.terminal_browser_request_matches_active_thread(&session_key) {
            self.complete_terminal_browser_open(
                request_id,
                Err(anyhow::anyhow!(
                    "terminal browser permission policy only allows the active TUI thread"
                )),
            );
            return;
        }
        let Some(browser) = self.terminal_browser_for_active_request().await else {
            self.complete_terminal_browser_open(
                request_id,
                Err(anyhow::anyhow!(
                    "terminal browser permission policy disables this TUI session"
                )),
            );
            return;
        };
        let app_event_tx = self.app_event_tx.unscoped();
        tokio::spawn(async move {
            let response =
                dynamic_tool_response(browser.execute(&session_key, "open", arguments).await);
            app_event_tx.send(AppEvent::TerminalBrowserToolCompleted {
                request_id,
                response,
                profile_approval: None,
            });
        });
    }

    fn terminal_browser_network_availability(&self) -> TerminalBrowserNetworkAvailability {
        TerminalBrowserNetworkAvailability::from_config_and_runtime(
            self.chat_widget.config_ref(),
            self.chat_widget.session_network_proxy(),
        )
    }

    fn terminal_browser_network_policy(&self) -> BrowserNetworkPolicy {
        match self.terminal_browser_network_availability() {
            TerminalBrowserNetworkAvailability::Direct => BrowserNetworkPolicy::Direct,
            TerminalBrowserNetworkAvailability::ManagedProxy { http_addr } => {
                BrowserNetworkPolicy::ManagedProxy { http_addr }
            }
            TerminalBrowserNetworkAvailability::Restricted
            | TerminalBrowserNetworkAvailability::ManagedProxyUnavailable
            | TerminalBrowserNetworkAvailability::ManagedProxyMitmUnsupported => {
                BrowserNetworkPolicy::Disabled
            }
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

        if self.terminal_browser.is_some()
            && self.terminal_browser_owner_thread_id != Some(owner_thread_id)
        {
            self.reset_terminal_browser_for_thread_change().await;
        }

        if self.terminal_browser.is_none() {
            let (browser, needs_update_watcher) = if let Some(reopenable) =
                self.terminal_browser_reopenable.remove(&owner_thread_id)
            {
                let ReopenableTerminalBrowser { browser, closed } = reopenable;
                if closed.await.is_err() {
                    browser.close().await;
                }
                (browser, false)
            } else {
                (self.discover_terminal_browser(), true)
            };
            if needs_update_watcher {
                let mut updates = browser.subscribe();
                let app_event_tx = self.app_event_tx.unscoped();
                tokio::spawn(async move {
                    while updates.changed().await.is_ok() {
                        app_event_tx.send(AppEvent::TerminalBrowserUpdated);
                    }
                });
            }
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
        let owner_thread_id = self.chat_widget.focused_thread_id()?;
        self.terminal_browser_for_thread(owner_thread_id).await
    }

    pub(super) fn terminal_browser_request_matches_active_thread(
        &self,
        request_thread_id: &str,
    ) -> bool {
        terminal_browser_request_matches_thread(
            self.chat_widget.focused_thread_id(),
            request_thread_id,
        )
    }

    pub(crate) fn terminal_browser_owned_by_current_thread(&self) -> bool {
        self.terminal_browser.is_some()
            && self
                .terminal_browser_owner_thread_id
                .is_some_and(|owner| self.chat_widget.focused_thread_id() == Some(owner))
    }

    pub(super) fn terminal_browser_for_current_thread(&self) -> Option<Arc<TerminalBrowser>> {
        self.terminal_browser_owned_by_current_thread()
            .then(|| self.terminal_browser.clone())
            .flatten()
    }

    pub(super) fn terminal_browser_control_target(&self) -> Option<TerminalBrowserControlTarget> {
        let owner_thread_id = self.terminal_browser_owner_thread_id?;
        let browser = self.terminal_browser_for_current_thread()?;
        Some(TerminalBrowserControlTarget {
            owner_thread_id,
            generation: self.terminal_browser_generation,
            token: browser.human_control_token(),
        })
    }

    pub(super) fn active_terminal_browser_control_target(
        &self,
    ) -> Option<TerminalBrowserControlTarget> {
        let target = self.terminal_browser_control_target()?;
        self.terminal_browser_for_current_thread()?
            .is_human_control_active()
            .then_some(target)
    }

    pub(super) fn terminal_browser_control_target_is_current(
        &self,
        target: TerminalBrowserControlTarget,
    ) -> bool {
        self.terminal_browser_control_target() == Some(target)
    }

    pub(super) fn terminal_browser_human_control_active(&self) -> bool {
        self.has_owned_screen()
            && self.owned_screen_frame.focus()
                == super::owned_screen_frame::OwnedScreenFrameFocus::Summary
            && self.owned_screen_frame.right_rail_content() == OwnedScreenRightRailContent::Browser
            && self
                .owned_screen_frame
                .panel_body(crate::app_event::OwnedScreenPanel::Summary)
                .is_some()
            && self
                .terminal_browser_for_current_thread()
                .is_some_and(|browser| browser.is_human_control_active())
    }

    pub(super) fn terminal_browser_control_click_returns_to_app(
        event: MousePrimaryEvent,
        viewport: Option<Rect>,
    ) -> bool {
        event.kind == MousePrimaryEventKind::Press
            && viewport.is_none_or(|viewport| !viewport.contains((event.column, event.row).into()))
    }

    pub(super) fn sync_terminal_browser_panel(&mut self) -> bool {
        if !self.has_owned_screen() {
            return false;
        }
        let browser_view = self
            .terminal_browser_for_current_thread()
            .map(|browser| browser.view());
        if let Some(view) = &browser_view {
            if view.visible
                && self.owned_screen_frame.right_rail_content()
                    != OwnedScreenRightRailContent::Browser
            {
                self.owned_screen_frame
                    .select_right_rail_content(OwnedScreenRightRailContent::Browser);
            } else if self.owned_screen_frame.right_rail_content()
                == OwnedScreenRightRailContent::Browser
                && !view.visible
            {
                self.owned_screen_frame
                    .set_right_rail_content(OwnedScreenRightRailContent::Summary);
            }
        }
        browser_view.is_some_and(|view| view.visible && view.human_control)
    }

    pub(super) fn hide_terminal_browser_panel(&mut self) {
        if let Some(browser) = self.terminal_browser_for_current_thread() {
            let invalidate_human_handles = browser.is_human_control_active();
            browser.set_visibility(/*visible*/ false);
            if invalidate_human_handles {
                tokio::spawn(async move {
                    browser.complete_human_control_cleanup().await;
                });
            }
        }
        if self.owned_screen_frame.right_rail_content() == OwnedScreenRightRailContent::Browser {
            self.owned_screen_frame
                .set_right_rail_content(OwnedScreenRightRailContent::Summary);
        }
    }

    pub(super) fn forward_terminal_browser_key(&mut self, key_event: KeyEvent) {
        let Some(input) = browser_key_input(key_event) else {
            return;
        };
        let Some(browser) = self.terminal_browser_for_current_thread() else {
            return;
        };
        if let Err(error) = browser.send_human_key(input) {
            self.chat_widget
                .add_error_message(format!("Failed to send browser key: {error}"));
        }
    }

    pub(super) fn forward_terminal_browser_text(&mut self, text: &str) {
        let Some(browser) = self.terminal_browser_for_current_thread() else {
            return;
        };
        if let Err(error) = browser.send_human_text(text) {
            self.chat_widget
                .add_error_message(format!("Failed to paste into browser: {error}"));
        }
    }

    pub(super) async fn forward_terminal_browser_mouse(
        &mut self,
        mouse_event: MouseEvent,
        viewport: Option<Rect>,
    ) {
        let Some(viewport) = viewport else {
            return;
        };
        let input = match browser_mouse_route(mouse_event, viewport) {
            BrowserMouseRoute::Input(input) => input,
            BrowserMouseRoute::ReleaseButtons => {
                self.release_terminal_browser_mouse_buttons().await;
                return;
            }
            BrowserMouseRoute::Ignore => return,
        };
        let Some(browser) = self.terminal_browser_for_current_thread() else {
            return;
        };
        if let Err(error) = browser.send_human_mouse(input) {
            self.chat_widget
                .add_error_message(format!("Failed to send browser mouse event: {error}"));
        }
    }

    pub(super) async fn release_terminal_browser_mouse_buttons(&mut self) {
        let Some(browser) = self.terminal_browser_for_current_thread() else {
            return;
        };
        if let Err(error) = browser.release_human_mouse_buttons().await {
            self.chat_widget
                .add_error_message(format!("Failed to release browser mouse buttons: {error}"));
        }
    }

    pub(super) fn terminal_browser_control_inputs(
        &self,
        event: MousePrimaryEvent,
        view: &BrowserView,
    ) -> Option<[BrowserMouseInput; 2]> {
        if self.overlay.is_some()
            || !self.chat_widget.no_modal_or_popup_active()
            || self.owned_screen_frame.right_rail_content() != OwnedScreenRightRailContent::Browser
            || !view.visible
            || view.human_control
            || !matches!(&view.status, BrowserStatus::Running)
        {
            return None;
        }
        let (panel, press) = self.owned_screen_frame.completed_panel_click(event)?;
        if panel != OwnedScreenPanel::Summary {
            return None;
        }
        let viewport = self
            .owned_screen_frame
            .panel_body(OwnedScreenPanel::Summary)
            .map(crate::terminal_browser::browser_viewport)?;
        Some([
            browser_mouse_input(press.into(), viewport)?,
            browser_mouse_input(event.into(), viewport)?,
        ])
    }

    pub(super) fn terminal_browser_control_click(
        &self,
        event: MousePrimaryEvent,
    ) -> Option<TerminalBrowserControlClick> {
        let browser = self.terminal_browser_for_current_thread()?;
        let inputs = self.terminal_browser_control_inputs(event, &browser.view())?;
        if browser.is_human_control_active() {
            return None;
        }
        let target = self.terminal_browser_control_target()?;
        Some(TerminalBrowserControlClick {
            browser,
            target,
            inputs,
        })
    }

    pub(super) async fn begin_terminal_browser_control_from_click(
        &mut self,
        click: TerminalBrowserControlClick,
    ) {
        let TerminalBrowserControlClick {
            browser,
            target,
            inputs,
        } = click;
        let result = browser.toggle_human_control(target.token).await;
        let mut completion_target = target;
        let error = match result {
            Ok(token) => {
                completion_target.token = token;
                for input in inputs {
                    if let Err(error) = browser.send_human_mouse(input) {
                        self.chat_widget.add_error_message(format!(
                            "Failed to send browser mouse event: {error}"
                        ));
                        break;
                    }
                }
                None
            }
            Err(error) => Some(error.to_string()),
        };
        self.app_event_tx
            .unscoped()
            .send(AppEvent::TerminalBrowserControlCompleted {
                target: completion_target,
                error,
            });
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

    pub(super) async fn show_terminal_browser(&mut self) {
        if !self.has_owned_screen() {
            self.chat_widget.add_info_message(
                "Terminal browser panels require `tui.alternate_screen = \"always\"`.".to_string(),
                /*hint*/ None,
            );
            return;
        }
        if let Some(message) = self
            .terminal_browser_network_availability()
            .unavailable_message()
        {
            self.chat_widget
                .add_info_message(message.to_string(), /*hint*/ None);
            return;
        }
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

        browser.set_visibility(/*visible*/ true);
        self.app_event_tx
            .unscoped()
            .send(AppEvent::TerminalBrowserUpdated);
    }

    pub(super) fn close_terminal_browser(&mut self) {
        let Some(owner_thread_id) = self
            .terminal_browser_owner_thread_id
            .filter(|_| self.terminal_browser_owned_by_current_thread())
        else {
            self.chat_widget.add_info_message(
                "Terminal browser is not enabled.".to_string(),
                /*hint*/ None,
            );
            return;
        };
        let Some(browser) = self.take_terminal_browser_for_thread_change() else {
            return;
        };
        let (closed_tx, closed_rx) = oneshot::channel();
        if let Some(replaced) = self.terminal_browser_reopenable.insert(
            owner_thread_id,
            ReopenableTerminalBrowser {
                browser: Arc::clone(&browser),
                closed: closed_rx,
            },
        ) {
            replaced.terminate();
        }
        let app_event_tx = self.app_event_tx.unscoped();
        tokio::spawn(async move {
            browser.close().await;
            let _ = closed_tx.send(());
            app_event_tx.send(AppEvent::TerminalBrowserClosed);
        });
    }

    fn take_terminal_browser_for_thread_change(&mut self) -> Option<Arc<TerminalBrowser>> {
        self.terminal_browser_owner_thread_id = None;
        self.terminal_browser_generation =
            self.terminal_browser_generation.wrapping_add(/*rhs*/ 1);
        let browser = self.terminal_browser.take()?;
        browser.set_visibility(/*visible*/ false);
        if self.owned_screen_frame.right_rail_content() == OwnedScreenRightRailContent::Browser {
            self.owned_screen_frame
                .set_right_rail_content(OwnedScreenRightRailContent::Summary);
        }
        Some(browser)
    }

    pub(super) async fn reset_terminal_browser_for_thread_change(&mut self) {
        let Some(browser) = self.take_terminal_browser_for_thread_change() else {
            return;
        };
        browser.close().await;
        self.app_event_tx
            .unscoped()
            .send(AppEvent::TerminalBrowserClosed);
    }

    pub(super) fn reset_terminal_browser_for_focus_change(&mut self) {
        let Some(browser) = self.take_terminal_browser_for_thread_change() else {
            return;
        };
        let app_event_tx = self.app_event_tx.unscoped();
        app_event_tx.send(AppEvent::TerminalBrowserClosed);
        tokio::spawn(async move {
            browser.close().await;
        });
    }

    pub(super) fn discard_reopenable_terminal_browser(&mut self, thread_id: ThreadId) {
        if let Some(browser) = self.terminal_browser_reopenable.remove(&thread_id) {
            browser.terminate();
        }
    }

    pub(super) fn discard_all_reopenable_terminal_browsers(&mut self) {
        for (_, browser) in self.terminal_browser_reopenable.drain() {
            browser.terminate();
        }
    }

    pub(super) async fn doctor_terminal_browser(&mut self) {
        if !self.terminal_browser_enabled() {
            self.chat_widget
                .add_error_message("Terminal browser is not enabled.".to_string());
            return;
        }
        let browser = self.discover_terminal_browser();
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
            && self.chat_widget.focused_thread_id() == Some(approval.thread_id)
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
        if !self.has_owned_screen() {
            self.chat_widget.add_info_message(
                "Terminal browser control requires `tui.alternate_screen = \"always\"`."
                    .to_string(),
                /*hint*/ None,
            );
            return;
        }
        let Some(browser) = self.terminal_browser_for_active_thread().await else {
            self.chat_widget
                .add_error_message("Terminal browser is not enabled.".to_string());
            return;
        };
        let Some(target) = self.terminal_browser_control_target() else {
            return;
        };
        if !browser.view().visible
            || self.owned_screen_frame.right_rail_content() != OwnedScreenRightRailContent::Browser
        {
            self.chat_widget.add_info_message(
                "Show the terminal browser before taking control.".to_string(),
                /*hint*/ None,
            );
            return;
        }
        if !browser.is_human_control_active() {
            self.owned_screen_frame
                .select_right_rail_content(OwnedScreenRightRailContent::Browser);
        }
        let app_event_tx = self.app_event_tx.unscoped();
        tokio::spawn(async move {
            let result = browser.toggle_human_control(target.token).await;
            let mut completion_target = target;
            let error = match result {
                Ok(token) => {
                    completion_target.token = token;
                    None
                }
                Err(error) => Some(error.to_string()),
            };
            app_event_tx.send(AppEvent::TerminalBrowserControlCompleted {
                target: completion_target,
                error,
            });
        });
    }

    pub(super) fn end_terminal_browser_control(&mut self, target: TerminalBrowserControlTarget) {
        if !self.terminal_browser_control_target_is_current(target) {
            return;
        }
        let Some(browser) = self.terminal_browser_for_current_thread() else {
            return;
        };
        let app_event_tx = self.app_event_tx.unscoped();
        tokio::spawn(async move {
            let result = browser.end_human_control(target.token).await;
            let mut completion_target = target;
            let error = match result {
                Ok(token) => {
                    completion_target.token = token;
                    None
                }
                Err(error) => Some(error.to_string()),
            };
            app_event_tx.send(AppEvent::TerminalBrowserControlCompleted {
                target: completion_target,
                error,
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
