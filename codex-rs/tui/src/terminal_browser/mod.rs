//! TUI integration for the Carbonyl-backed terminal browser.

use crate::app::App;
use crate::app_event::TerminalBrowserProfileCommand;
use crate::pager_overlay::Overlay;
use crate::tui;
use codex_features::Feature;
use codex_terminal_browser::BrowserInputModifiers;
use codex_terminal_browser::BrowserKeyInput;
use codex_terminal_browser::BrowserLaunchContext;
use codex_terminal_browser::BrowserMouseButton;
use codex_terminal_browser::BrowserMouseInput;
use codex_terminal_browser::BrowserMouseKind;
use codex_terminal_browser::BrowserNetworkPolicy;
use codex_terminal_browser::BrowserStatus;
use codex_terminal_browser::TerminalBrowser;
use codex_utils_absolute_path::AbsolutePathBuf;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;
use crossterm::event::MouseButton;
use crossterm::event::MouseEvent;
use crossterm::event::MouseEventKind;
use ratatui::layout::Rect;
use std::sync::Arc;

mod overlay;
mod profile_approval;
mod tools;

pub(crate) use profile_approval::requested_profile_command;

pub(crate) use overlay::TerminalBrowserOverlay;
pub(crate) use tools::TERMINAL_BROWSER_NAMESPACE;
pub(crate) use tools::dynamic_tool_response;
pub(crate) use tools::dynamic_tool_specs;

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
            let codex_linux_sandbox_exe =
                self.config
                    .codex_linux_sandbox_exe
                    .as_ref()
                    .and_then(
                        |path| match AbsolutePathBuf::from_absolute_path_checked(path) {
                            Ok(path) => Some(path),
                            Err(error) => {
                                tracing::warn!(
                                    %error,
                                    path = %path.display(),
                                    "invalid Linux sandbox helper path for terminal browser"
                                );
                                None
                            }
                        },
                    );
            let browser = Arc::new(TerminalBrowser::discover_with_launch_context(
                BrowserLaunchContext {
                    codex_home: Some(self.config.codex_home.clone()),
                    workspace_root: Some(self.config.cwd.clone()),
                    codex_linux_sandbox_exe,
                    use_legacy_landlock: self.config.features.use_legacy_landlock(),
                },
            ));
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

    pub(crate) fn terminal_browser_human_control_active(&self) -> bool {
        self.terminal_browser
            .as_ref()
            .is_some_and(|browser| browser.is_human_control_active())
    }

    pub(crate) fn forward_terminal_browser_key(&mut self, key_event: KeyEvent) {
        let Some(input) = browser_key_input(key_event) else {
            return;
        };
        let Some(browser) = self.terminal_browser.as_ref().cloned() else {
            return;
        };
        if let Err(error) = browser.send_human_key(input) {
            self.chat_widget
                .add_error_message(format!("Failed to send browser key: {error}"));
        }
    }

    pub(crate) fn forward_terminal_browser_text(&mut self, text: &str) {
        let Some(browser) = self.terminal_browser.as_ref().cloned() else {
            return;
        };
        if let Err(error) = browser.send_human_text(text) {
            self.chat_widget
                .add_error_message(format!("Failed to paste into browser: {error}"));
        }
    }

    pub(crate) fn forward_terminal_browser_mouse(
        &mut self,
        terminal_area: Rect,
        mouse_event: MouseEvent,
    ) {
        let viewport = overlay::browser_viewport(overlay::overlay_area(terminal_area));
        let Some(input) = browser_mouse_input(mouse_event, viewport) else {
            return;
        };
        let Some(browser) = self.terminal_browser.as_ref().cloned() else {
            return;
        };
        if let Err(error) = browser.send_human_mouse(input) {
            self.chat_widget
                .add_error_message(format!("Failed to send browser mouse event: {error}"));
        }
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

    pub(crate) fn doctor_terminal_browser(&mut self) {
        let Some(browser) = self.ensure_terminal_browser() else {
            self.chat_widget
                .add_error_message("Terminal browser is not enabled.".to_string());
            return;
        };
        let app_event_tx = self.app_event_tx.clone();
        tokio::spawn(async move {
            let report = browser.doctor().await;
            app_event_tx.send(crate::app_event::AppEvent::TerminalBrowserDoctorCompleted {
                healthy: report.healthy,
                summary: report.summary,
            });
        });
    }

    pub(crate) async fn manage_terminal_browser_profile(
        &mut self,
        command: TerminalBrowserProfileCommand,
    ) {
        let Some(browser) = self.ensure_terminal_browser() else {
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

    pub(crate) fn toggle_terminal_browser_control(&mut self) {
        let Some(browser) = self.ensure_terminal_browser() else {
            self.chat_widget
                .add_error_message("Terminal browser is not enabled.".to_string());
            return;
        };
        let app_event_tx = self.app_event_tx.clone();
        tokio::spawn(async move {
            let result = browser.toggle_human_control().await;
            let active = result
                .as_ref()
                .copied()
                .unwrap_or_else(|_| browser.is_human_control_active());
            app_event_tx.send(
                crate::app_event::AppEvent::TerminalBrowserControlCompleted {
                    active,
                    error: result.err().map(|error| error.to_string()),
                },
            );
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
        let human_control = browser
            .as_ref()
            .is_some_and(|browser| browser.is_human_control_active());
        if let Err(error) = tui.set_mouse_capture(should_show && human_control) {
            tracing::debug!(%error, "failed to reconcile terminal-browser mouse capture");
        }
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

fn browser_key_input(event: KeyEvent) -> Option<BrowserKeyInput> {
    if !matches!(event.kind, KeyEventKind::Press | KeyEventKind::Repeat) {
        return None;
    }
    let modifiers = browser_modifiers(event.modifiers);
    let (key, code, text) = match event.code {
        KeyCode::Char(character) => {
            let code = if character.is_ascii_alphabetic() {
                format!("Key{}", character.to_ascii_uppercase())
            } else if character.is_ascii_digit() {
                format!("Digit{character}")
            } else if character == ' ' {
                "Space".to_string()
            } else {
                character.to_string()
            };
            let text = (!modifiers.control && !modifiers.alt && !modifiers.meta)
                .then(|| character.to_string());
            (character.to_string(), code, text)
        }
        KeyCode::Enter => ("Enter".to_string(), "Enter".to_string(), None),
        KeyCode::Tab | KeyCode::BackTab => ("Tab".to_string(), "Tab".to_string(), None),
        KeyCode::Backspace => ("Backspace".to_string(), "Backspace".to_string(), None),
        KeyCode::Delete => ("Delete".to_string(), "Delete".to_string(), None),
        KeyCode::Esc => ("Escape".to_string(), "Escape".to_string(), None),
        KeyCode::Left => ("ArrowLeft".to_string(), "ArrowLeft".to_string(), None),
        KeyCode::Right => ("ArrowRight".to_string(), "ArrowRight".to_string(), None),
        KeyCode::Up => ("ArrowUp".to_string(), "ArrowUp".to_string(), None),
        KeyCode::Down => ("ArrowDown".to_string(), "ArrowDown".to_string(), None),
        KeyCode::Home => ("Home".to_string(), "Home".to_string(), None),
        KeyCode::End => ("End".to_string(), "End".to_string(), None),
        KeyCode::PageUp => ("PageUp".to_string(), "PageUp".to_string(), None),
        KeyCode::PageDown => ("PageDown".to_string(), "PageDown".to_string(), None),
        KeyCode::F(number) => {
            let key = format!("F{number}");
            (key.clone(), key, None)
        }
        _ => return None,
    };
    let modifiers = if matches!(event.code, KeyCode::BackTab) {
        BrowserInputModifiers {
            shift: true,
            ..modifiers
        }
    } else {
        modifiers
    };
    Some(BrowserKeyInput {
        key,
        code,
        text,
        modifiers,
    })
}

fn browser_mouse_input(event: MouseEvent, viewport: Rect) -> Option<BrowserMouseInput> {
    if !viewport.contains((event.column, event.row).into()) || viewport.is_empty() {
        return None;
    }
    let (kind, button) = match event.kind {
        MouseEventKind::Moved => (BrowserMouseKind::Move, BrowserMouseButton::None),
        MouseEventKind::Down(button) => (BrowserMouseKind::Down, browser_mouse_button(button)),
        MouseEventKind::Up(button) => (BrowserMouseKind::Up, browser_mouse_button(button)),
        MouseEventKind::Drag(button) => (BrowserMouseKind::Move, browser_mouse_button(button)),
        MouseEventKind::ScrollUp => (
            BrowserMouseKind::Wheel {
                delta_x: 0.0,
                delta_y: -100.0,
            },
            BrowserMouseButton::None,
        ),
        MouseEventKind::ScrollDown => (
            BrowserMouseKind::Wheel {
                delta_x: 0.0,
                delta_y: 100.0,
            },
            BrowserMouseButton::None,
        ),
        MouseEventKind::ScrollLeft => (
            BrowserMouseKind::Wheel {
                delta_x: -100.0,
                delta_y: 0.0,
            },
            BrowserMouseButton::None,
        ),
        MouseEventKind::ScrollRight => (
            BrowserMouseKind::Wheel {
                delta_x: 100.0,
                delta_y: 0.0,
            },
            BrowserMouseButton::None,
        ),
    };
    Some(BrowserMouseInput {
        kind,
        button,
        column: event.column.saturating_sub(viewport.x),
        row: event.row.saturating_sub(viewport.y),
        viewport_cols: viewport.width,
        viewport_rows: viewport.height,
        modifiers: browser_modifiers(event.modifiers),
    })
}

fn browser_mouse_button(button: MouseButton) -> BrowserMouseButton {
    match button {
        MouseButton::Left => BrowserMouseButton::Left,
        MouseButton::Middle => BrowserMouseButton::Middle,
        MouseButton::Right => BrowserMouseButton::Right,
    }
}

fn browser_modifiers(modifiers: KeyModifiers) -> BrowserInputModifiers {
    BrowserInputModifiers {
        alt: modifiers.contains(KeyModifiers::ALT),
        control: modifiers.contains(KeyModifiers::CONTROL),
        meta: modifiers.contains(KeyModifiers::SUPER),
        shift: modifiers.contains(KeyModifiers::SHIFT),
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
