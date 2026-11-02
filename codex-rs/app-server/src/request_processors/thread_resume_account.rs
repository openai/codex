//! Applies the rollout-gated CLI account check before resuming or attaching to a local thread.
//! Creator tags are immutable; untagged threads and other clients retain existing behavior.

use crate::error_code::invalid_request;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_features::Feature;
use codex_features::Features;
use codex_login::AuthManager;
use codex_login::CodexAuth;
use codex_thread_store::StoredThread;

pub(super) fn check(
    features: &Features,
    client_name: Option<&str>,
    auth_manager: &AuthManager,
    thread: &StoredThread,
) -> Result<(), JSONRPCErrorError> {
    if !features.enabled(Feature::CliResumeAccountCheck)
        || !matches!(client_name, Some("codex-tui" | "codex_exec"))
        || thread.rollout_path.is_none()
        || (thread.creator_user_id.is_none() && thread.creator_account_id.is_none())
    {
        return Ok(());
    }
    let Some(auth) = auth_manager
        .auth_cached()
        .filter(CodexAuth::is_chatgpt_auth)
    else {
        // API keys and other auth modes do not identify a comparable ChatGPT account.
        return Ok(());
    };
    if thread
        .creator_user_id
        .as_ref()
        .is_some_and(|creator| auth.get_chatgpt_user_id().as_ref() != Some(creator))
        || thread
            .creator_account_id
            .as_ref()
            .is_some_and(|creator| auth.get_account_id().as_ref() != Some(creator))
    {
        return Err(invalid_request(
            "This thread was created with a different ChatGPT account or workspace. Sign in with the original account to resume it.",
        ));
    }
    Ok(())
}
