//! Fetch managed configuration from a custom config server.
//! Cache entries are scoped to the endpoint and the current OAuth sign-in.

use crate::backend::BundleRequestError;
use crate::backend::RetryableFailureKind;
use crate::backend::fetch_bundle;
use crate::config_source::BundleSource;
use crate::service::CLOUD_CONFIG_BUNDLE_TIMEOUT;
use crate::service::CloudConfigBundleService;
use codex_backend_client::Client as BackendClient;
use codex_config::CloudConfigBundle;
use codex_config::CloudConfigBundleLoadError;
use codex_config::CloudConfigBundleLoadErrorCode;
use codex_config::CloudConfigBundleLoader;
use codex_config::ConfigServer;
use codex_http_client::HttpClientFactory;
use codex_keyring_store::DefaultKeyringStore;
use codex_login::AuthRouteConfig;
use codex_login::OAuthSessionConfig;
use codex_login::OAuthSessionError;
use codex_login::OAuthSessionManager;
use codex_login::OAuthSessionRequest;
use sha2::Digest;
use sha2::Sha256;
use std::path::PathBuf;
use std::sync::Arc;

pub fn config_server_bundle_loader(
    config: ConfigServer,
    codex_home: PathBuf,
    factory: HttpClientFactory,
) -> CloudConfigBundleLoader {
    match ConfigServerSource::from_config_server(config.clone(), codex_home.clone(), factory) {
        Ok(source) => {
            let service = CloudConfigBundleService::from_source(
                source,
                codex_home,
                CLOUD_CONFIG_BUNDLE_TIMEOUT,
            );
            crate::bundle_loader::managed_config_bundle_loader_for_service(service).0
        }
        Err(error) => CloudConfigBundleLoader::new(std::future::ready(Err(error))),
    }
    .with_config_server(config)
}

pub fn config_server_auth_manager(
    service_config: &ConfigServer,
    codex_home: PathBuf,
    factory: &HttpClientFactory,
) -> std::io::Result<Option<OAuthSessionManager>> {
    service_config
        .validated_base_url()
        .map_err(std::io::Error::other)?;
    oauth_session_manager(service_config.oauth.as_deref(), codex_home, factory)
}

pub(crate) fn config_server_http_client_factory(
    config: &ConfigServer,
    routes: &AuthRouteConfig,
) -> HttpClientFactory {
    let endpoint = config.validated_base_url().ok().map(|base_url| {
        BackendClient::for_config_endpoint(base_url, routes.http_client_factory().clone())
            .config_bundle_url()
    });
    let endpoints = endpoint
        .as_deref()
        .into_iter()
        .chain(config.oauth.as_ref().map(|oauth| oauth.token_url.as_str()))
        .chain(
            config
                .oauth
                .as_ref()
                .and_then(|oauth| oauth.device_authorization_url.as_deref()),
        )
        .filter_map(|endpoint| endpoint.parse().ok());
    routes.http_client_factory_for_bootstrap(endpoints)
}

struct ConfigServerSource {
    cache_namespace: String,
    client: BackendClient,
    auth: Option<OAuthSessionManager>,
}

fn oauth_session_manager(
    oauth: Option<&OAuthSessionConfig>,
    codex_home: PathBuf,
    factory: &HttpClientFactory,
) -> std::io::Result<Option<OAuthSessionManager>> {
    oauth
        .map(|oauth| {
            OAuthSessionManager::new(
                oauth.clone(),
                codex_home,
                factory,
                Arc::new(DefaultKeyringStore),
            )
            .map(OAuthSessionManager::without_interactive_login)
        })
        .transpose()
}

impl ConfigServerSource {
    fn from_config_server(
        config: ConfigServer,
        codex_home: PathBuf,
        factory: HttpClientFactory,
    ) -> Result<Self, CloudConfigBundleLoadError> {
        let base_url = config.validated_base_url().map_err(|message| {
            CloudConfigBundleLoadError::new(
                CloudConfigBundleLoadErrorCode::Internal,
                /*status_code*/ None,
                message,
            )
        })?;
        Self::new(
            base_url,
            config.oauth.map(|oauth| *oauth),
            codex_home,
            factory,
        )
    }

    fn new(
        base_url: String,
        oauth: Option<OAuthSessionConfig>,
        codex_home: PathBuf,
        factory: HttpClientFactory,
    ) -> Result<Self, CloudConfigBundleLoadError> {
        let auth =
            oauth_session_manager(oauth.as_ref(), codex_home, &factory).map_err(auth_io_error)?;
        let client = BackendClient::for_config_endpoint(base_url, factory);
        // Reuse the OAuth session's issuer/client/audience/scope identity; the endpoint
        // additionally separates policy authorities that share one OAuth grant.
        let namespace = serde_json::to_vec(&(
            client.config_bundle_url(),
            auth.as_ref().map(OAuthSessionManager::credential_id),
        ))
        .map_err(|_| auth_failure())?;
        Ok(Self {
            client,
            cache_namespace: format!("{:x}", Sha256::digest(namespace)),
            auth,
        })
    }
}

impl BundleSource for ConfigServerSource {
    type Auth = Option<String>;
    type FetchContext = Option<OAuthSessionRequest>;
    fn begin_fetch(&self) -> Self::FetchContext {
        self.auth.as_ref().map(OAuthSessionManager::begin_request)
    }

    async fn auth(&self) -> Result<Option<Self::Auth>, CloudConfigBundleLoadError> {
        // Cache reads need the grant identity, not a refreshed access token.
        Ok(Some(match &self.auth {
            Some(auth) => Some(auth.authorization_id().await.map_err(auth_io_error)?),
            None => None,
        }))
    }

    fn cache_namespace(&self) -> Option<String> {
        Some(self.cache_namespace.clone())
    }

    fn identity(&self, auth: &Self::Auth) -> (Option<String>, Option<String>) {
        (
            Some(auth.clone().unwrap_or_else(|| "anonymous".into())),
            self.cache_namespace(),
        )
    }

    async fn get_bundle(
        &self,
        identity: &mut Self::Auth,
        context: &mut Self::FetchContext,
    ) -> Result<CloudConfigBundle, BundleRequestError> {
        let Some(request) = context else {
            return fetch_bundle(&self.client).await;
        };
        let credentials = request
            .credentials()
            .await
            .map_err(|error| auth_request_error(error, request.is_recovering()))?;
        if credentials.authorization_id.is_none() {
            return Err(BundleRequestError::Fatal {
                error: auth_failure(),
                metric_reason: Some("auth"),
            });
        }
        *identity = credentials.authorization_id;
        fetch_bundle(
            &self
                .client
                .clone()
                .with_bearer_token(credentials.access_token),
        )
        .await
    }

    async fn recover_auth(
        &self,
        _identity: &mut Self::Auth,
        context: &mut Self::FetchContext,
        status_code: Option<u16>,
        recovery_attempt: usize,
    ) -> Result<bool, BundleRequestError> {
        let Some(auth) = context else {
            return Ok(false);
        };
        if status_code != Some(401) || recovery_attempt != 0 {
            return Ok(false);
        }
        Ok(auth.reject_credentials())
    }
}

fn auth_request_error(error: std::io::Error, recovering: bool) -> BundleRequestError {
    if error
        .get_ref()
        .is_some_and(<dyn std::error::Error + Send + Sync>::is::<OAuthSessionError>)
        || matches!(
            error.kind(),
            std::io::ErrorKind::PermissionDenied
                | std::io::ErrorKind::InvalidInput
                | std::io::ErrorKind::InvalidData
        )
    {
        BundleRequestError::Fatal {
            error: auth_io_error(error),
            metric_reason: Some("auth"),
        }
    } else {
        BundleRequestError::Retryable(if recovering {
            RetryableFailureKind::AuthRecovery {
                status_code: Some(401),
            }
        } else {
            RetryableFailureKind::Request { status_code: None }
        })
    }
}

fn auth_io_error(error: std::io::Error) -> CloudConfigBundleLoadError {
    if error
        .get_ref()
        .and_then(|source| source.downcast_ref::<OAuthSessionError>())
        == Some(&OAuthSessionError::LoginRequired)
    {
        CloudConfigBundleLoadError::new(
            CloudConfigBundleLoadErrorCode::Auth,
            /*status_code*/ None,
            "Sign in to the configuration server to load your organization's settings",
        )
    } else {
        auth_failure()
    }
}

fn auth_failure() -> CloudConfigBundleLoadError {
    CloudConfigBundleLoadError::new(
        CloudConfigBundleLoadErrorCode::Auth,
        /*status_code*/ None,
        "Could not authenticate with the configuration server",
    )
}

#[cfg(test)]
#[path = "config_server_tests.rs"]
mod tests;
