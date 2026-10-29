//! Keeps request preparation bound to the credential revision it started with.

use std::future::Future;
use std::io;

use codex_login::AuthChangeState;
use codex_login::RequestAuthChange;
use codex_protocol::error::Result;

use crate::ModelProvider;

/// Prepare a request without dispatching it. When the request uses managed auth,
/// repeat preparation after a credential refresh and fail if the owner changes.
pub async fn prepare_provider_request<T, Fut>(
    provider: &dyn ModelProvider,
    check_auth: bool,
    mut prepare: impl FnMut(Option<AuthChangeState>) -> Fut,
) -> Result<T>
where
    Fut: Future<Output = Result<T>>,
{
    let auth_state = || {
        provider
            .auth_manager()
            .map(|manager| *manager.auth_change_state_receiver().borrow())
    };
    let mut before = auth_state();
    loop {
        let prepared = prepare(before).await?;
        if !check_auth {
            return Ok(prepared);
        }
        // A managed provider can begin or cease exposing its auth manager during setup.
        let after = auth_state();
        let change = match (before, after) {
            (Some(before), Some(after)) => {
                after.compare_for_request(before.owner_generation, before)
            }
            (None, None) => RequestAuthChange::Unchanged,
            _ => RequestAuthChange::OwnerChanged,
        };
        match change {
            RequestAuthChange::Unchanged => return Ok(prepared),
            RequestAuthChange::CredentialsChanged => before = after,
            RequestAuthChange::OwnerChanged => {
                return Err(
                    io::Error::other("account changed while preparing model request").into(),
                );
            }
        }
    }
}

#[cfg(test)]
#[path = "request_preparation_tests.rs"]
mod tests;
