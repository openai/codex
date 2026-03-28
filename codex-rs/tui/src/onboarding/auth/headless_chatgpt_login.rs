#![allow(dead_code)]

use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::LoginAccountParams;
use codex_app_server_protocol::LoginAccountResponse;
use codex_core::auth::CLIENT_ID;
use codex_login::ServerOptions;
use codex_login::complete_device_code_login;
use codex_login::request_device_code;
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

use crate::local_chatgpt_auth::LocalChatgptAuth;
use crate::local_chatgpt_auth::load_local_chatgpt_auth;
use crate::shimmer::shimmer_spans;
use crate::tui::FrameRequester;

use super::AuthModeWidget;
use super::ContinueInBrowserState;
use super::ContinueWithDeviceCodeState;
use super::SignInState;
use super::mark_url_hyperlink;
use super::maybe_open_auth_url_in_browser;
use super::onboarding_request_id;

pub(super) fn start_headless_chatgpt_login(widget: &mut AuthModeWidget) {
    let mut opts = ServerOptions::new(
        widget.codex_home.clone(),
        CLIENT_ID.to_string(),
        widget.forced_chatgpt_workspace_id.clone(),
        widget.cli_auth_credentials_store_mode,
    );
    opts.open_browser = false;

    let sign_in_state = widget.sign_in_state.clone();
    let request_frame = widget.request_frame.clone();
    let error = widget.error.clone();
    let request_handle = widget.app_server_request_handle.clone();
    let codex_home = widget.codex_home.clone();
    let cli_auth_credentials_store_mode = widget.cli_auth_credentials_store_mode;
    let forced_chatgpt_workspace_id = widget.forced_chatgpt_workspace_id.clone();
    let cancel = begin_device_code_attempt(&sign_in_state, &request_frame);

    tokio::spawn(async move {
        let device_code = match request_device_code(&opts).await {
            Ok(device_code) => device_code,
            Err(err) => {
                if err.kind() == std::io::ErrorKind::NotFound {
                    fallback_to_browser_login(
                        request_handle,
                        sign_in_state,
                        request_frame,
                        error,
                        cancel,
                    )
                    .await;
                } else {
                    set_device_code_error_for_active_attempt(
                        &sign_in_state,
                        &request_frame,
                        &error,
                        &cancel,
                        err.to_string(),
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
            result = complete_device_code_login(opts, device_code) => {
                match result {
                    Ok(()) => {
                        let local_auth = load_local_chatgpt_auth(
                            &codex_home,
                            cli_auth_credentials_store_mode,
                            forced_chatgpt_workspace_id.as_deref(),
                        );
                        handle_chatgpt_auth_tokens_login_result_for_active_attempt(
                            request_handle,
                            sign_in_state,
                            request_frame,
                            error,
                            cancel,
                            local_auth,
                        ).await;
                    }
                    Err(err) => {
                        set_device_code_error_for_active_attempt(
                            &sign_in_state,
                            &request_frame,
                            &error,
                            &cancel,
                            err.to_string(),
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

    // Capture the verification URL for OSC 8 hyperlink marking after render.
    let verification_url = if let Some(device_code) = &state.device_code {
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
        Some(device_code.verification_url.clone())
    } else {
        lines.push("  Requesting a one-time code...".dim().into());
        lines.push("".into());
        None
    };

    lines.push("  Press Esc to cancel".dim().into());
    Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .render(area, buf);

    // Wrap cyan+underlined URL cells with OSC 8 so the terminal treats
    // the entire region as a single clickable hyperlink.
    if let Some(url) = &verification_url {
        mark_url_hyperlink(buf, area, url);
    }
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
    cancel: &Arc<Notify>,
) -> bool {
    let mut guard = sign_in_state.write().unwrap();
    if !device_code_attempt_matches(&guard, cancel) {
        return false;
    }

    *guard = SignInState::ChatGptSuccessMessage;
    drop(guard);
    request_frame.schedule_frame();
    true
}

fn set_device_code_error_for_active_attempt(
    sign_in_state: &Arc<RwLock<SignInState>>,
    request_frame: &FrameRequester,
    error: &Arc<RwLock<Option<String>>>,
    cancel: &Arc<Notify>,
    message: String,
) -> bool {
    if !set_device_code_state_for_active_attempt(
        sign_in_state,
        request_frame,
        cancel,
        SignInState::PickMode,
    ) {
        return false;
    }
    *error.write().unwrap() = Some(message);
    request_frame.schedule_frame();
    true
}

async fn fallback_to_browser_login(
    request_handle: codex_app_server_client::AppServerRequestHandle,
    sign_in_state: Arc<RwLock<SignInState>>,
    request_frame: FrameRequester,
    error: Arc<RwLock<Option<String>>>,
    cancel: Arc<Notify>,
) {
    let should_fallback = {
        let guard = sign_in_state.read().unwrap();
        device_code_attempt_matches(&guard, &cancel)
    };
    if !should_fallback {
        return;
    }

    match request_handle
        .request_typed::<LoginAccountResponse>(ClientRequest::LoginAccount {
            request_id: onboarding_request_id(),
            params: LoginAccountParams::Chatgpt,
        })
        .await
    {
        Ok(LoginAccountResponse::Chatgpt { login_id, auth_url }) => {
            maybe_open_auth_url_in_browser(&request_handle, &auth_url);
            *error.write().unwrap() = None;
            let _updated = set_device_code_state_for_active_attempt(
                &sign_in_state,
                &request_frame,
                &cancel,
                SignInState::ChatGptContinueInBrowser(ContinueInBrowserState {
                    login_id,
                    auth_url,
                }),
            );
        }
        Ok(other) => {
            set_device_code_error_for_active_attempt(
                &sign_in_state,
                &request_frame,
                &error,
                &cancel,
                format!("Unexpected account/login/start response: {other:?}"),
            );
        }
        Err(err) => {
            set_device_code_error_for_active_attempt(
                &sign_in_state,
                &request_frame,
                &error,
                &cancel,
                err.to_string(),
            );
        }
    }
}

async fn handle_chatgpt_auth_tokens_login_result_for_active_attempt(
    request_handle: codex_app_server_client::AppServerRequestHandle,
    sign_in_state: Arc<RwLock<SignInState>>,
    request_frame: FrameRequester,
    error: Arc<RwLock<Option<String>>>,
    cancel: Arc<Notify>,
    local_auth: Result<LocalChatgptAuth, String>,
) {
    let local_auth = match local_auth {
        Ok(local_auth) => local_auth,
        Err(err) => {
            set_device_code_error_for_active_attempt(
                &sign_in_state,
                &request_frame,
                &error,
                &cancel,
                err,
            );
            return;
        }
    };

    let result = request_handle
        .request_typed::<LoginAccountResponse>(ClientRequest::LoginAccount {
            request_id: onboarding_request_id(),
            params: LoginAccountParams::ChatgptAuthTokens {
                access_token: local_auth.access_token,
                chatgpt_account_id: local_auth.chatgpt_account_id,
                chatgpt_plan_type: local_auth.chatgpt_plan_type,
            },
        })
        .await;
    apply_chatgpt_auth_tokens_login_response_for_active_attempt(
        &sign_in_state,
        &request_frame,
        &error,
        &cancel,
        result.map_err(|err| err.to_string()),
    );
}

fn apply_chatgpt_auth_tokens_login_response_for_active_attempt(
    sign_in_state: &Arc<RwLock<SignInState>>,
    request_frame: &FrameRequester,
    error: &Arc<RwLock<Option<String>>>,
    cancel: &Arc<Notify>,
    result: Result<LoginAccountResponse, String>,
) {
    match result {
        Ok(LoginAccountResponse::ChatgptAuthTokens {}) => {
            *error.write().unwrap() = None;
            let _updated = set_device_code_success_message_for_active_attempt(
                sign_in_state,
                request_frame,
                cancel,
            );
        }
        Ok(other) => {
            set_device_code_error_for_active_attempt(
                sign_in_state,
                request_frame,
                error,
                cancel,
                format!("Unexpected account/login/start response: {other:?}"),
            );
        }
        Err(err) => {
            set_device_code_error_for_active_attempt(
                sign_in_state,
                request_frame,
                error,
                cancel,
                err,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use pretty_assertions::assert_eq;

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
        assert_eq!(
            set_device_code_success_message_for_active_attempt(
                &sign_in_state,
                &request_frame,
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
                &cancel,
            ),
            false
        );
        assert!(matches!(
            &*sign_in_state.read().unwrap(),
            SignInState::ChatGptDeviceCode(_)
        ));
    }

    #[test]
    fn chatgpt_auth_tokens_success_sets_success_message_without_login_id() {
        let sign_in_state = device_code_sign_in_state(Arc::new(Notify::new()));
        let request_frame = FrameRequester::test_dummy();
        let error = Arc::new(RwLock::new(None));
        let cancel = match &*sign_in_state.read().unwrap() {
            SignInState::ChatGptDeviceCode(state) => {
                state.cancel.as_ref().expect("cancel handle").clone()
            }
            _ => panic!("expected device-code state"),
        };

        apply_chatgpt_auth_tokens_login_response_for_active_attempt(
            &sign_in_state,
            &request_frame,
            &error,
            &cancel,
            Ok(LoginAccountResponse::ChatgptAuthTokens {}),
        );

        assert!(matches!(
            &*sign_in_state.read().unwrap(),
            SignInState::ChatGptSuccessMessage
        ));
    }
}
