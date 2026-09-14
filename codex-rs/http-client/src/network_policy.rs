//! Application destination checks and revocation of outstanding network operations.
//!
//! The configuration owner publishes an already-composed policy. Transports retain only its
//! read side and a permit for each request; invalidation cannot leave an old request authorized.

use std::collections::BTreeSet;
use std::fmt;
use std::future::Future;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::MutexGuard;
use std::sync::Weak;

use reqwest::Url;
use tokio::sync::watch;

/// Effective application destinations, after the requirements owner has applied precedence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DestinationPolicy {
    Unrestricted,
    /// Only HTTPS and WSS requests to these exact, normalized hosts are permitted.
    Restricted {
        allowed_hosts: BTreeSet<String>,
    },
}

impl DestinationPolicy {
    fn allows(&self, url: &Url) -> bool {
        match self {
            Self::Unrestricted => true,
            Self::Restricted { allowed_hosts } => {
                matches!(url.scheme(), "https" | "wss")
                    && url.host_str().is_some_and(|host| {
                        allowed_hosts.contains(host.strip_suffix('.').unwrap_or(host))
                    })
            }
        }
    }
}

/// A deterministic denial, distinct from retryable network failures.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum NetworkPolicyDenied {
    #[error("application network policy is unavailable")]
    Unavailable,
    #[error("destination denied by application network policy")]
    Destination,
    #[error("application network permission was revoked")]
    Revoked,
    #[error("this SDK transport is disabled by application network restrictions")]
    UnsupportedTransport,
}

/// Read access to application policy. Clones observe the same updates.
#[derive(Clone, Default)]
pub struct NetworkPolicy {
    state: Option<Arc<Mutex<State>>>,
    endpoints: Option<Arc<BTreeSet<Url>>>,
    account: Option<u64>,
}

impl fmt::Debug for NetworkPolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NetworkPolicy")
            .field("managed", &self.state.is_some())
            .finish_non_exhaustive()
    }
}

impl PartialEq for NetworkPolicy {
    fn eq(&self, other: &Self) -> bool {
        self.endpoints == other.endpoints
            && self.account == other.account
            && match (&self.state, &other.state) {
                (Some(left), Some(right)) => Arc::ptr_eq(left, right),
                (None, None) => true,
                (Some(_), None) | (None, Some(_)) => false,
            }
    }
}

impl Eq for NetworkPolicy {}

struct State {
    policy: Option<DestinationPolicy>,
    revision: u64,
    account: u64,
    permits: Vec<Weak<Permit>>,
    changes: watch::Sender<()>,
}

struct Permit {
    destination: PermitDestination,
    revoked: watch::Sender<bool>,
}

enum PermitDestination {
    Url(Url),
    UnrestrictedSdk,
}

impl PermitDestination {
    fn allowed_by(&self, policy: &DestinationPolicy) -> bool {
        match self {
            Self::Url(url) => policy.allows(url),
            Self::UnrestrictedSdk => matches!(policy, DestinationPolicy::Unrestricted),
        }
    }
}

/// Publication access retained by the account/configuration owner.
#[derive(Clone)]
pub struct NetworkPolicyController {
    state: Arc<Mutex<State>>,
}

/// Identifies the account/configuration generation that a load is allowed to publish into.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NetworkPolicyRevision(u64);

impl Default for NetworkPolicyController {
    fn default() -> Self {
        Self {
            state: Arc::new(Mutex::new(State {
                policy: None,
                revision: 0,
                account: 0,
                permits: Vec::new(),
                changes: watch::channel(()).0,
            })),
        }
    }
}

fn lock_state(state: &Mutex<State>) -> MutexGuard<'_, State> {
    state
        .lock()
        .unwrap_or_else(|_| panic!("application network policy lock poisoned"))
}

impl NetworkPolicyController {
    pub fn policy(&self) -> NetworkPolicy {
        NetworkPolicy {
            state: Some(self.state.clone()),
            endpoints: None,
            account: None,
        }
    }

    /// Fails a requirements load without changing the current account identity.
    pub fn unavailable(&self, revision: NetworkPolicyRevision) {
        let mut state = lock_state(&self.state);
        if state.revision != revision.0 {
            return;
        }
        if state.policy.take().is_some() {
            state.changes.send_replace(());
        }
        for permit in state
            .permits
            .drain(..)
            .filter_map(|permit| permit.upgrade())
        {
            permit.revoked.send_replace(/*value*/ true);
        }
    }

    /// Publishes a successful load only if its account/configuration generation is still current.
    pub fn publish(&self, revision: NetworkPolicyRevision, policy: DestinationPolicy) -> bool {
        let mut state = lock_state(&self.state);
        if state.revision != revision.0 {
            return false;
        }
        state.permits.retain(|permit| {
            let Some(permit) = permit.upgrade() else {
                return false;
            };
            if !permit.destination.allowed_by(&policy) {
                permit.revoked.send_replace(/*value*/ true);
            }
            true
        });
        if state.policy.as_ref() != Some(&policy) {
            state.policy = Some(policy);
            state.changes.send_replace(());
        }
        true
    }
}

impl NetworkPolicy {
    pub const fn unmanaged() -> Self {
        Self {
            state: None,
            endpoints: None,
            account: None,
        }
    }

    /// Binds a content client to the current account so retained credentials cannot
    /// be used after another workspace's policy has been installed.
    pub fn for_current_account(mut self) -> Self {
        self.account = self.state.as_ref().map(|state| lock_state(state).account);
        self
    }

    /// Whether this transport participates in the application policy lifecycle.
    pub fn is_managed(&self) -> bool {
        self.state.is_some()
    }

    /// Watches effective changes; unmanaged callers have no application policy owner.
    pub fn changes(&self) -> Option<watch::Receiver<()>> {
        self.state
            .as_ref()
            .map(|state| lock_state(state).changes.subscribe())
    }

    pub fn revision(&self) -> NetworkPolicyRevision {
        NetworkPolicyRevision(
            self.state
                .as_ref()
                .map_or(0, |state| lock_state(state).revision),
        )
    }

    /// Revokes outstanding operations before an account change or a failed requirements load.
    /// This read-side operation can only remove access; publication stays with the controller.
    pub fn invalidate(&self) {
        if let Some(state) = &self.state {
            let mut state = lock_state(state);
            state.revision += 1;
            state.account += 1;
            state.policy = None;
            state.changes.send_replace(());
            for permit in state
                .permits
                .drain(..)
                .filter_map(|permit| permit.upgrade())
            {
                permit.revoked.send_replace(/*value*/ true);
            }
        }
    }

    /// Checks a destination before proxy resolution, DNS, or transport work starts.
    pub fn acquire(&self, url: &Url) -> Result<NetworkPermit, NetworkPolicyDenied> {
        if self
            .endpoints
            .as_ref()
            .is_some_and(|endpoints| !endpoints.contains(url))
        {
            return Err(NetworkPolicyDenied::Destination);
        }
        self.acquire_destination(PermitDestination::Url(url.clone()))
    }

    /// Disables SDKs without destination enforcement whenever restrictions apply.
    /// The returned permit also cancels work if an unrestricted policy changes.
    pub fn acquire_for_unsupported_sdk(&self) -> Result<NetworkPermit, NetworkPolicyDenied> {
        if self.endpoints.is_some() {
            return Err(NetworkPolicyDenied::UnsupportedTransport);
        }
        self.acquire_destination(PermitDestination::UnrestrictedSdk)
    }

    fn acquire_destination(
        &self,
        destination: PermitDestination,
    ) -> Result<NetworkPermit, NetworkPolicyDenied> {
        let Some(state) = &self.state else {
            return Ok(NetworkPermit { inner: None });
        };
        let mut state = lock_state(state);
        if self.account.is_some_and(|account| account != state.account) {
            return Err(NetworkPolicyDenied::Revoked);
        }
        let policy = state
            .policy
            .as_ref()
            .ok_or(NetworkPolicyDenied::Unavailable)?;
        if !destination.allowed_by(policy) {
            return Err(match destination {
                PermitDestination::Url(_) => NetworkPolicyDenied::Destination,
                PermitDestination::UnrestrictedSdk => NetworkPolicyDenied::UnsupportedTransport,
            });
        }
        let (revoked, _) = watch::channel(/*init*/ false);
        let permit = Arc::new(Permit {
            destination,
            revoked,
        });
        state.permits.retain(|permit| permit.strong_count() != 0);
        state.permits.push(Arc::downgrade(&permit));
        Ok(NetworkPermit {
            inner: Some(permit),
        })
    }
}

/// Authorization retained for the entire operation, including response or WebSocket streaming.
#[derive(Clone)]
pub struct NetworkPermit {
    inner: Option<Arc<Permit>>,
}

impl fmt::Debug for NetworkPermit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NetworkPermit").finish_non_exhaustive()
    }
}

impl NetworkPermit {
    pub fn check(&self) -> Result<(), NetworkPolicyDenied> {
        if self
            .inner
            .as_ref()
            .is_some_and(|permit| *permit.revoked.borrow())
        {
            Err(NetworkPolicyDenied::Revoked)
        } else {
            Ok(())
        }
    }

    pub async fn revoked(&self) {
        match &self.inner {
            Some(permit) => {
                let _ = permit
                    .revoked
                    .subscribe()
                    .wait_for(|revoked| *revoked)
                    .await;
            }
            None => std::future::pending().await,
        }
    }

    pub async fn run<T>(
        &self,
        operation: impl Future<Output = T>,
    ) -> Result<T, NetworkPolicyDenied> {
        tokio::select! {
            biased;
            _ = self.revoked() => Err(NetworkPolicyDenied::Revoked),
            result = operation => Ok(result),
        }
    }
}

#[cfg(test)]
#[path = "network_policy_tests.rs"]
mod tests;
