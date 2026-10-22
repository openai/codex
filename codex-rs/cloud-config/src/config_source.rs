//! Common interface for fetching managed configuration

use crate::backend::BundleRequestError;
use codex_config::CloudConfigBundle;
use codex_config::CloudConfigBundleLoadError;
use std::future::Future;

/// Supplies authentication and cache identity to the managed-config service.
/// Cache lookup and fetch must use the same authorization identity.
pub(crate) trait BundleSource: Send + Sync {
    type Auth: Send + Sync;
    type FetchContext: Send;
    /// One context per load or refresh, retained across the service's retries.
    fn begin_fetch(&self) -> Self::FetchContext;

    /// `None` means this source does not apply, such as an ineligible ChatGPT plan.
    /// A required source returns `Some` even when its endpoint is unauthenticated.
    fn auth(
        &self,
    ) -> impl Future<Output = Result<Option<Self::Auth>, CloudConfigBundleLoadError>> + Send;
    /// Stable subject and authority identifiers; missing values disable cache reads.
    fn identity(&self, auth: &Self::Auth) -> (Option<String>, Option<String>);
    /// Cache lookups do not call this operation or require a token refresh.
    fn get_bundle(
        &self,
        auth: &mut Self::Auth,
        context: &mut Self::FetchContext,
    ) -> impl Future<Output = Result<CloudConfigBundle, BundleRequestError>> + Send;

    /// Recover one unauthorized request. The service retries only when this returns true.
    fn recover_auth(
        &self,
        auth: &mut Self::Auth,
        context: &mut Self::FetchContext,
        status_code: Option<u16>,
        recovery_attempt: usize,
    ) -> impl Future<Output = Result<bool, BundleRequestError>> + Send;

    /// The default preserves the existing ChatGPT cache location.
    fn cache_namespace(&self) -> Option<String> {
        None
    }
}
