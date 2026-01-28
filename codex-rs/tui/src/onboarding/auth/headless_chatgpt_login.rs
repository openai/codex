use codex_core::AuthManager;
use codex_login::ServerOptions;
use codex_login::complete_device_code_login;
use codex_login::request_device_code;
use codex_login::run_login_server;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::prelude::Widget;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Wrap;
use std::sync::Arc;
use std::sync::RwLock;
use tokio::sync::Notify;

use crate::shimmer::shimmer_spans;
use crate::tui::FrameRequester;

use super::AuthModeWidget;
use super::ContinueInBrowserState;
use super::ContinueWithDeviceCodeState;
use super::SignInState;

pub(super) fn start_headless_chatgpt_login(widget: &mut AuthModeWidget, mut opts: ServerOptions) {
    opts.open_browser = false;
    let sign_in_state = widget.sign_in_state.clone();
    let request_frame = widget.request_frame.clone();
    let auth_manager = widget.auth_manager.clone();
    let cancel = begin_device_code_attempt(&sign_in_state, &request_frame);

    tokio::spawn(async move {
        let device_code = match request_device_code(&opts).await {
            Ok(device_code) => device_code,
            Err(err) => {
                if err.kind() == std::io::ErrorKind::NotFound {
                    let should_fallback = {
                        let guard = sign_in_state.read().unwrap();
                        device_code_attempt_matches(&guard, &cancel)
                    };

                    if !should_fallback {
                        return;
                    }

                    match run_login_server(opts) {
                        Ok(child) => {
                            let auth_url = child.auth_url.clone();
                            {
                                *sign_in_state.write().unwrap() =
                                    SignInState::ChatGptContinueInBrowser(ContinueInBrowserState {
                                        auth_url,
                                        shutdown_flag: Some(child.cancel_handle()),
                                    });
                            }
                            request_frame.schedule_frame();
                            let r = child.block_until_done().await;
                            match r {
                                Ok(()) => {
                                    auth_manager.reload();
                                    *sign_in_state.write().unwrap() =
                                        SignInState::ChatGptSuccessMessage;
                                    request_frame.schedule_frame();
                                }
                                _ => {
                                    *sign_in_state.write().unwrap() = SignInState::PickMode;
                                    request_frame.schedule_frame();
                                }
                            }
                        }
                        Err(_) => {
                            set_device_code_state_for_active_attempt(
                                &sign_in_state,
                                &request_frame,
                                &cancel,
                                SignInState::PickMode,
                            );
                        }
                    }
                } else {
                    set_device_code_state_for_active_attempt(
                        &sign_in_state,
                        &request_frame,
                        &cancel,
                        SignInState::PickMode,
                    );
                }

                return;
            }
        };

        if !set_device_code_state_for_active_attempt(
            &sign_in_state,
            &request_frame,
            &cancel,
            SignInState::ChatGptDeviceCode(ContinueWithDeviceCodeState {
                device_code: Some(device_code.clone()),
                cancel: Some(cancel.clone()),
            }),
        ) {
            return;
        }

        tokio::select! {
            _ = cancel.notified() => {}
            r = complete_device_code_login(opts, device_code) => {
                match r {
                    Ok(()) => {
                        set_device_code_success_message_for_active_attempt(
                            &sign_in_state,
                            &request_frame,
                            &auth_manager,
                            &cancel,
                        );
                    }
                    Err(_) => {
                        set_device_code_state_for_active_attempt(
                            &sign_in_state,
                            &request_frame,
                            &cancel,
                            SignInState::PickMode,
                        );
                    }
                }
            }
        }
    });
}

pub(super) fn render_device_code_login(
    widget: &AuthModeWidget,
    area: Rect,
    buf: &mut Buffer,
    state: &ContinueWithDeviceCodeState,
) {
    let banner = if state.device_code.is_some() {
        "Finish signing in via your browser"
    } else {
        "Preparing device code login"
    };

    let mut spans = vec!["  ".into()];
    if widget.animations_enabled {
        // Schedule a follow-up frame to keep the shimmer animation going.
        widget
            .request_frame
            .schedule_frame_in(std::time::Duration::from_millis(100));
        spans.extend(shimmer_spans(banner));
    } else {
        spans.push(banner.into());
    }

    let mut lines = vec![spans.into(), "".into()];

    if let Some(device_code) = &state.device_code {
        lines.push("  1. Open this link in your browser and sign in".into());
        lines.push("".into());
        lines.push(Line::from(vec![
            "  ".into(),
            device_code.verification_url.as_str().cyan().underlined(),
        ]));
        lines.push("".into());
        lines.push(
            "  2. Enter this one-time code after you are signed in (expires in 15 minutes)".into(),
        );
        lines.push("".into());
        lines.push(Line::from(vec![
            "  ".into(),
            device_code.user_code.as_str().cyan().bold(),
        ]));
        lines.push("".into());
        lines.push(
            "  Device codes are a common phishing target. Never share this code."
                .dim()
                .into(),
        );
        lines.push("".into());
    } else {
        lines.push("  Requesting a one-time code...".dim().into());
        lines.push("".into());
    }

    lines.push("  Press Esc to cancel".dim().into());
    Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .render(area, buf);
}

fn device_code_attempt_matches(state: &SignInState, cancel: &Arc<Notify>) -> bool {
    matches!(
        state,
        SignInState::ChatGptDeviceCode(state)
            if state
                .cancel
                .as_ref()
                .is_some_and(|existing| Arc::ptr_eq(existing, cancel))
    )
}

fn begin_device_code_attempt(
    sign_in_state: &Arc<RwLock<SignInState>>,
    request_frame: &FrameRequester,
) -> Arc<Notify> {
    let cancel = Arc::new(Notify::new());
    *sign_in_state.write().unwrap() = SignInState::ChatGptDeviceCode(ContinueWithDeviceCodeState {
        device_code: None,
        cancel: Some(cancel.clone()),
    });
    request_frame.schedule_frame();
    cancel
}

fn set_device_code_state_for_active_attempt(
    sign_in_state: &Arc<RwLock<SignInState>>,
    request_frame: &FrameRequester,
    cancel: &Arc<Notify>,
    next_state: SignInState,
) -> bool {
    let mut guard = sign_in_state.write().unwrap();
    if !device_code_attempt_matches(&guard, cancel) {
        return false;
    }

    *guard = next_state;
    drop(guard);
    request_frame.schedule_frame();
    true
}

fn set_device_code_success_message_for_active_attempt(
    sign_in_state: &Arc<RwLock<SignInState>>,
    request_frame: &FrameRequester,
    auth_manager: &AuthManager,
    cancel: &Arc<Notify>,
) -> bool {
    let mut guard = sign_in_state.write().unwrap();
    if !device_code_attempt_matches(&guard, cancel) {
        return false;
    }

    auth_manager.reload();
    *guard = SignInState::ChatGptSuccessMessage;
    drop(guard);
    request_frame.schedule_frame();
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_core::auth::AuthCredentialsStoreMode;
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    fn device_code_sign_in_state(cancel: Arc<Notify>) -> Arc<RwLock<SignInState>> {
        Arc::new(RwLock::new(SignInState::ChatGptDeviceCode(
            ContinueWithDeviceCodeState {
                device_code: None,
                cancel: Some(cancel),
            },
        )))
    }

    #[test]
    fn device_code_attempt_matches_only_for_matching_cancel() {
        let cancel = Arc::new(Notify::new());
        let state = SignInState::ChatGptDeviceCode(ContinueWithDeviceCodeState {
            device_code: None,
            cancel: Some(cancel.clone()),
        });

        assert_eq!(device_code_attempt_matches(&state, &cancel), true);
        assert_eq!(
            device_code_attempt_matches(&state, &Arc::new(Notify::new())),
            false
        );
        assert_eq!(
            device_code_attempt_matches(&SignInState::PickMode, &cancel),
            false
        );
    }

    #[test]
    fn begin_device_code_attempt_sets_state() {
        let sign_in_state = Arc::new(RwLock::new(SignInState::PickMode));
        let request_frame = FrameRequester::test_dummy();

        let cancel = begin_device_code_attempt(&sign_in_state, &request_frame);
        let guard = sign_in_state.read().unwrap();

        let state: &SignInState = &guard;
        assert_eq!(device_code_attempt_matches(state, &cancel), true);
        assert!(matches!(
            state,
            SignInState::ChatGptDeviceCode(state) if state.device_code.is_none()
        ));
    }

    #[test]
    fn set_device_code_state_for_active_attempt_updates_only_when_active() {
        let request_frame = FrameRequester::test_dummy();
        let cancel = Arc::new(Notify::new());
        let sign_in_state = device_code_sign_in_state(cancel.clone());

        assert_eq!(
            set_device_code_state_for_active_attempt(
                &sign_in_state,
                &request_frame,
                &cancel,
                SignInState::PickMode,
            ),
            true
        );
        assert!(matches!(
            &*sign_in_state.read().unwrap(),
            SignInState::PickMode
        ));

        let sign_in_state = device_code_sign_in_state(Arc::new(Notify::new()));
        assert_eq!(
            set_device_code_state_for_active_attempt(
                &sign_in_state,
                &request_frame,
                &cancel,
                SignInState::PickMode,
            ),
            false
        );
        assert!(matches!(
            &*sign_in_state.read().unwrap(),
            SignInState::ChatGptDeviceCode(_)
        ));
    }

    #[test]
    fn set_device_code_success_message_for_active_attempt_updates_only_when_active() {
        let request_frame = FrameRequester::test_dummy();
        let cancel = Arc::new(Notify::new());
        let sign_in_state = device_code_sign_in_state(cancel.clone());
        let temp_dir = TempDir::new().unwrap();
        let auth_manager = AuthManager::shared(
            temp_dir.path().to_path_buf(),
            false,
            AuthCredentialsStoreMode::File,
        );

        assert_eq!(
            set_device_code_success_message_for_active_attempt(
                &sign_in_state,
                &request_frame,
                &auth_manager,
                &cancel,
            ),
            true
        );
        assert!(matches!(
            &*sign_in_state.read().unwrap(),
            SignInState::ChatGptSuccessMessage
        ));

        let sign_in_state = device_code_sign_in_state(Arc::new(Notify::new()));
        assert_eq!(
            set_device_code_success_message_for_active_attempt(
                &sign_in_state,
                &request_frame,
                &auth_manager,
                &cancel,
            ),
            false
        );
        assert!(matches!(
            &*sign_in_state.read().unwrap(),
            SignInState::ChatGptDeviceCode(_)
        ));
    }
}
