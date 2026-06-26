//! TUI integration for the Carbonyl-backed terminal browser.

use crate::app::App;
use crate::pager_overlay::Overlay;
use crate::tui;
use codex_features::Feature;
use codex_terminal_browser::BrowserNetworkPolicy;
use codex_terminal_browser::BrowserStatus;
use codex_terminal_browser::TerminalBrowser;
use std::sync::Arc;

mod overlay;
mod tools;

pub(crate) use overlay::TerminalBrowserOverlay;
pub(crate) use tools::TERMINAL_BROWSER_NAMESPACE;
pub(crate) use tools::dynamic_tool_response;
pub(crate) use tools::dynamic_tool_specs;
pub(crate) use tools::terminal_browser_available;

impl App {
    fn terminal_browser_network_policy(&self) -> BrowserNetworkPolicy {
        let config = self.chat_widget.config_ref();
        if !config.permissions.network_sandbox_policy().is_enabled() {
            BrowserNetworkPolicy::Disabled
        } else if config.permissions.network.is_none() {
            BrowserNetworkPolicy::Direct
        } else if let Some(proxy) = self.chat_widget.session_network_proxy.as_ref() {
            match proxy.http_addr.parse::<std::net::SocketAddr>() {
                Ok(http_addr) => BrowserNetworkPolicy::ManagedProxy { http_addr },
                Err(error) => {
                    tracing::warn!(
                        %error,
                        http_addr = %proxy.http_addr,
                        "invalid managed proxy address for terminal browser"
                    );
                    BrowserNetworkPolicy::Disabled
                }
            }
        } else {
            BrowserNetworkPolicy::Disabled
        }
    }

    pub(crate) fn ensure_terminal_browser(&mut self) -> Option<Arc<TerminalBrowser>> {
        if self.terminal_browser.is_none()
            && self.config.features.enabled(Feature::TerminalBrowser)
            && !self.app_server_target.uses_remote_workspace()
        {
            let browser = Arc::new(TerminalBrowser::discover());
            let mut updates = browser.subscribe();
            let app_event_tx = self.app_event_tx.clone();
            tokio::spawn(async move {
                while updates.changed().await.is_ok() {
                    app_event_tx.send(crate::app_event::AppEvent::TerminalBrowserUpdated);
                }
            });
            self.terminal_browser = Some(browser);
        }
        self.terminal_browser.as_ref().cloned()
    }

    pub(crate) async fn reconcile_terminal_browser_network_policy(
        &mut self,
    ) -> Option<Arc<TerminalBrowser>> {
        let network_policy = self.terminal_browser_network_policy();
        let browser = self.ensure_terminal_browser()?;
        browser.set_network_policy(network_policy).await;
        Some(browser)
    }

    pub(crate) fn terminal_browser_overlay_active(&self) -> bool {
        self.overlay
            .as_ref()
            .is_some_and(Overlay::is_terminal_browser)
    }

    pub(crate) fn toggle_terminal_browser(&mut self, tui: &mut tui::Tui) {
        let Some(browser) = self.ensure_terminal_browser() else {
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
        self.sync_terminal_browser_overlay(tui);
    }

    pub(crate) fn close_terminal_browser(&mut self) {
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
            app_event_tx.send(crate::app_event::AppEvent::TerminalBrowserClosed);
        });
    }

    pub(crate) async fn reset_terminal_browser_for_thread_change(&mut self, tui: &mut tui::Tui) {
        if self.terminal_browser_overlay_active() {
            self.hide_terminal_browser(tui);
        }
        if let Some(browser) = self.terminal_browser.as_ref().cloned() {
            browser.close().await;
        }
    }

    pub(crate) fn hide_terminal_browser(&mut self, tui: &mut tui::Tui) {
        if let Some(browser) = self.terminal_browser.as_ref() {
            browser.set_visibility(/*visible*/ false);
        }
        self.sync_terminal_browser_overlay(tui);
    }

    pub(crate) fn sync_terminal_browser_overlay(&mut self, tui: &mut tui::Tui) {
        let browser = self.terminal_browser.as_ref().cloned();
        let should_show = browser
            .as_ref()
            .is_some_and(|browser| browser.view().visible);
        let was_showing = self.terminal_browser_overlay_active();

        if should_show && self.overlay.is_none() {
            if let Some(browser) = browser {
                let _ = tui.enter_alt_screen();
                if let Err(err) = tui.clear_ambient_pet_image() {
                    tracing::debug!(error = %err, "failed to clear ambient pet image for terminal browser");
                }
                if let Err(err) = tui.draw_pet_picker_preview_image(/*request*/ None) {
                    tracing::debug!(error = %err, "failed to clear pet preview image for terminal browser");
                }
                self.overlay = Some(Overlay::new_terminal_browser(TerminalBrowserOverlay::new(
                    browser,
                )));
            }
        } else if !should_show && self.terminal_browser_overlay_active() {
            let _ = tui.leave_alt_screen();
            self.overlay = None;
            if !self.deferred_history_lines.is_empty() {
                let lines = std::mem::take(&mut self.deferred_history_lines);
                tui.insert_history_hyperlink_lines_with_wrap_policy(
                    lines,
                    self.history_line_wrap_policy(),
                );
            }
        }

        if should_show || was_showing {
            tui.frame_requester().schedule_frame();
        }
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
