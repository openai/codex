//! Finish authentication against the embedded server running the selected gateway.

use super::auth::AuthModeWidget;
use super::auth::SignInState;
use super::auth::onboarding_request_id;
use super::onboarding_screen::OnboardingScreen;
use super::onboarding_screen::OnboardingScreenArgs;
use super::onboarding_screen::run_onboarding_screen;
use crate::AppServerTarget;
use crate::app_server_session::AppServerSession;
use crate::get_login_status;
use crate::legacy_core::config::Config;
use crate::should_show_login_screen;
use crate::startup_draft::StartupCancelled;
use crate::tui::Tui;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::GatewayOAuthCancelResponse;
use codex_app_server_protocol::GatewayOAuthChangedNotification;
use codex_app_server_protocol::GatewayOAuthLoginParams;
use codex_app_server_protocol::GatewayOAuthLoginResponse;
use codex_app_server_protocol::GatewayOAuthReadParams;
use codex_app_server_protocol::GatewayOAuthReadResponse;
use codex_app_server_protocol::GatewayOAuthStatus;
use codex_protocol::config_types::ForcedLoginMethod;
use color_eyre::Result;
use color_eyre::eyre::OptionExt;

pub(crate) async fn authenticate_gateway(
    tui: &mut Tui,
    server: &mut AppServerSession,
    config: &Config,
) -> Result<()> {
    let (login_status, account) = get_login_status(server).await?;
    let primary_login = should_show_login_screen(login_status, account.requires_openai_auth);
    // Some gateways require both credentials. The primary login must finish first.
    for login in [Login::Primary, Login::GatewayOAuth] {
        match login {
            Login::Primary if !primary_login => continue,
            Login::GatewayOAuth if config.model_provider.gateway_oauth.is_none() => continue,
            Login::GatewayOAuth | Login::Primary => {}
        }
        let handle = server.request_handle();
        let mut screen = OnboardingScreen::new(
            tui,
            OnboardingScreenArgs {
                show_login_screen: true,
                gateway_setup_enabled: false,
                gcp_setup_enabled: false,
                bedrock_setup_enabled: false,
                show_trust_screen: false,
                remote_project_trust: None,
                login_status,
                app_server_target: AppServerTarget::Embedded,
                app_server_request_handle: Some(handle.clone()),
                config: config.clone(),
            },
        )
        .await;
        let widget = screen
            .auth_widget_mut()
            .ok_or_eyre("The gateway login screen is unavailable.")?;
        match login {
            Login::Primary
                if widget
                    .auth_config
                    .is_login_method_allowed(ForcedLoginMethod::Chatgpt) =>
            {
                widget.start_chatgpt_login();
            }
            Login::Primary => widget.start_api_key_entry(),
            Login::GatewayOAuth => widget.start_gateway_oauth(config.model_provider_id.clone()),
        }
        if run_onboarding_screen(screen, Some(handle), Some(server), tui)
            .await?
            .should_exit
        {
            if matches!(login, Login::GatewayOAuth) {
                let _ = server
                    .request_handle()
                    .request_typed::<GatewayOAuthCancelResponse>(
                        ClientRequest::GatewayOAuthCancel {
                            request_id: onboarding_request_id(),
                            params: None,
                        },
                    )
                    .await;
            }
            return Err(
                std::io::Error::new(std::io::ErrorKind::Interrupted, StartupCancelled).into(),
            );
        }
    }
    Ok(())
}

enum Login {
    Primary,
    GatewayOAuth,
}

impl AuthModeWidget {
    pub(super) fn start_gateway_oauth(&mut self, provider_id: String) {
        *self
            .sign_in_state
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = SignInState::GatewayOAuth {
            provider_id: provider_id.clone(),
            url: None,
        };
        let state = self.sign_in_state.clone();
        let frame = self.request_frame.clone();
        let handle = self.app_server_request_handle.clone();
        tokio::spawn(async move {
            let result: Result<()> = async {
                let status = handle
                    .request_typed::<GatewayOAuthReadResponse>(ClientRequest::GatewayOAuthRead {
                        request_id: onboarding_request_id(),
                        params: Some(GatewayOAuthReadParams {
                            model_provider: Some(provider_id.clone()),
                        }),
                    })
                    .await?;
                color_eyre::eyre::ensure!(
                    status.provider_id == provider_id,
                    "The selected gateway changed during sign-in."
                );
                // Startup shows the device code for the selected chat before it requests models.
                if status.required
                    && status.status != Some(GatewayOAuthStatus::Succeeded)
                    && !crate::device_login::needs_device_login(
                        &status,
                        crate::device_login::prefer_device_login(),
                    )
                    .await
                {
                    handle
                        .request_typed::<GatewayOAuthLoginResponse>(
                            ClientRequest::GatewayOAuthLogin {
                                request_id: onboarding_request_id(),
                                params: Some(GatewayOAuthLoginParams {
                                    model_provider: Some(status.provider_id),
                                    ..Default::default()
                                }),
                            },
                        )
                        .await?;
                }
                Ok(())
            }
            .await;
            *state
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = match result {
                Ok(()) => SignInState::GatewayConfigured,
                Err(err) => SignInState::GatewayOAuthFailed {
                    provider_id,
                    message: err.to_string(),
                },
            };
            frame.schedule_frame();
        });
    }

    pub(super) fn on_gateway_oauth_changed(
        &mut self,
        notification: GatewayOAuthChangedNotification,
    ) {
        let Some(auth_url) = notification.auth_url else {
            return;
        };
        let mut state = self
            .sign_in_state
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let SignInState::GatewayOAuth { provider_id, url } = &mut *state
            && notification.provider_id == *provider_id
            && url.as_ref() != Some(&auth_url)
        {
            *url = Some(auth_url.clone());
            drop(state);
            self.request_frame.schedule_frame();
            tokio::task::spawn_blocking(move || {
                let _ = webbrowser::open(&auth_url);
            });
        }
    }
}
