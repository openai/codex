use codex_api::SharedAuthProvider;
use codex_http_state::HttpStateContext;
use codex_http_state::HttpStateSurface;
use codex_login::AuthManager;
use codex_login::UnauthorizedRecovery;
use std::io;
use std::io::ErrorKind;
use std::sync::Arc;
use tokio::sync::watch;
use tracing::info;
use tracing::warn;

pub(super) struct RemoteControlConnectionAuth {
    pub(super) auth_provider: SharedAuthProvider,
    pub(super) server_token_auth_provider: SharedAuthProvider,
    pub(super) account_id: String,
}

pub(super) fn remote_control_auth_recovery(
    auth_manager: &Arc<AuthManager>,
) -> UnauthorizedRecovery {
    auth_manager.unauthorized_recovery_for_surface(HttpStateSurface::CodexRemoteControl)
}

pub(super) async fn load_remote_control_auth(
    auth_manager: &Arc<AuthManager>,
) -> io::Result<RemoteControlConnectionAuth> {
    let mut reloaded = false;
    let auth = loop {
        let Some(auth) = auth_manager
            .auth_for_surface(HttpStateSurface::CodexRemoteControl)
            .await
        else {
            if reloaded {
                return Err(io::Error::new(
                    ErrorKind::PermissionDenied,
                    "remote control requires ChatGPT authentication",
                ));
            }
            auth_manager.reload().await;
            reloaded = true;
            continue;
        };
        if !auth.uses_codex_backend() {
            break auth;
        }
        if auth.get_account_id().is_none() && !reloaded {
            auth_manager.reload().await;
            reloaded = true;
            continue;
        }
        break auth;
    };

    if !auth.uses_codex_backend() {
        return Err(io::Error::new(
            ErrorKind::PermissionDenied,
            "remote control requires ChatGPT authentication; API key auth is not supported",
        ));
    }

    let http_state = HttpStateContext::new(
        auth_manager.codex_home().to_path_buf(),
        HttpStateSurface::CodexRemoteControl,
    );
    Ok(RemoteControlConnectionAuth {
        auth_provider: codex_model_provider::with_native_integrity_state(
            codex_model_provider::auth_provider_from_auth(&auth),
            Some(&auth),
            Some(http_state.clone()),
        ),
        server_token_auth_provider: codex_model_provider::with_native_integrity_state(
            codex_model_provider::unauthenticated_auth_provider(),
            Some(&auth),
            Some(http_state),
        ),
        account_id: auth.get_account_id().ok_or_else(|| {
            io::Error::new(
                ErrorKind::WouldBlock,
                "remote control enrollment is waiting for a ChatGPT account id",
            )
        })?,
    })
}

pub(super) async fn recover_remote_control_auth(
    auth_recovery: &mut UnauthorizedRecovery,
    auth_change_rx: &mut watch::Receiver<u64>,
) -> bool {
    if !auth_recovery.has_next() {
        return false;
    }

    let mode = auth_recovery.mode_name();
    let step = auth_recovery.step_name();
    let auth_change_revision_before_recovery = *auth_change_rx.borrow();
    match auth_recovery.next().await {
        Ok(step_result) => {
            if step_result.auth_state_changed() == Some(true) {
                mark_recovery_auth_change_seen(
                    auth_change_rx,
                    auth_change_revision_before_recovery,
                );
            }
            info!(
                "remote control auth recovery succeeded: mode={mode}, step={step}, auth_state_changed={:?}",
                step_result.auth_state_changed()
            );
            true
        }
        Err(err) => {
            warn!("remote control auth recovery failed: mode={mode}, step={step}: {err}");
            false
        }
    }
}

pub(super) fn mark_recovery_auth_change_seen(
    auth_change_rx: &mut watch::Receiver<u64>,
    auth_change_revision_before_recovery: u64,
) {
    let auth_change_revision_after_recovery = *auth_change_rx.borrow();
    if auth_change_revision_after_recovery == auth_change_revision_before_recovery.wrapping_add(1) {
        // Recovery updated the same watch that wakes the outer reconnect
        // loop. Mark only that single revision seen; if more revisions
        // arrived while recovery was in flight, leave them pending so the
        // reconnect loop still reacts to the later external auth change.
        auth_change_rx.borrow_and_update();
    }
}
