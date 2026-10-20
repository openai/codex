//! Tracks credential changes separately from ownership changes, before notifications coalesce.

use super::CodexAuth;

/// Opaque revisions local to one auth manager. Consumers must reset on reconnect.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AuthChangeState {
    /// Advances on cached credential changes and explicit invalidation of pending reads.
    pub generation: u64,
    /// Advances on login, logout, or changes of user, workspace, or auth mode.
    /// A verified legacy-to-OAuth credential migration preserves the same owner.
    /// Credential changes with incomplete owner identity also advance this revision.
    /// Losing login metadata alone preserves the account owner.
    pub owner_generation: u64,
    /// Advances on an underlying actor credential/source change, including a new OAuth login.
    /// Ordinary refresh within an identifiable OAuth session preserves this revision.
    pub source_generation: u64,
    /// Preserves strict login ownership for EMA, including loss of login metadata.
    pub login_generation: u64,
}

pub(super) fn same_owner(previous: Option<&CodexAuth>, current: Option<&CodexAuth>) -> bool {
    let (Some(previous), Some(current)) = (previous, current) else {
        return false;
    };
    if previous.api_auth_mode() != current.api_auth_mode() {
        return false;
    }
    let (Some(user), Some(workspace)) = (previous.get_chatgpt_user_id(), previous.get_account_id())
    else {
        return false;
    };
    let current_login = login_owner_id(Some(current));
    (current_login.is_none() || login_owner_id(Some(previous)) == current_login)
        && !user.trim().is_empty()
        && !workspace.trim().is_empty()
        && current.get_chatgpt_user_id().as_ref() == Some(&user)
        && current.get_account_id().as_ref() == Some(&workspace)
}

pub(super) fn same_login(previous: Option<&CodexAuth>, current: Option<&CodexAuth>) -> bool {
    same_owner(previous, current) && login_owner_id(previous) == login_owner_id(current)
}

fn login_owner_id(auth: Option<&CodexAuth>) -> Option<String> {
    auth.and_then(|auth| auth.get_token_data().ok())
        .and_then(|tokens| tokens.login_owner_id)
}
