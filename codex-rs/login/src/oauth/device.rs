//! Standard device authorization through `oauth2`, using Codex's configured HTTP transport.
//! Callers own prompts, cancellation, and credential persistence. Issuer errors stay redacted.

use std::fmt;
use std::future::Future;
use std::io;
use std::pin::Pin;
use std::time::Duration;

use codex_http_client::ClientRouteClass;
use codex_http_client::HttpClientFactory;
use codex_http_client::NetworkPolicyDenied;
use codex_http_client::RouteAwareClientPool;
use codex_http_client::bounded_response_bytes;
use codex_http_client::validate_endpoint_url;
use oauth2::AsyncHttpClient;
use oauth2::ClientId;
use oauth2::DeviceAuthorizationUrl;
use oauth2::DeviceCodeErrorResponseType;
use oauth2::RequestTokenError;
use oauth2::Scope;
use oauth2::StandardDeviceAuthorizationResponse;
use oauth2::TokenUrl;
use oauth2::basic::BasicClient;
use oauth2::basic::BasicTokenResponse;
use url::Url;

// OAuth responses contain credentials, not arbitrary downloads.
const MAX_OAUTH_RESPONSE_BYTES: usize = 64 * 1024;

/// The URL and short code a user needs to approve a device authorization request.
pub struct OAuthDeviceCodePrompt {
    pub verification_uri: Url,
    pub user_code: String,
}

impl fmt::Debug for OAuthDeviceCodePrompt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OAuthDeviceCodePrompt")
            .finish_non_exhaustive()
    }
}

pub(crate) struct DeviceAuthorizationOptions<'a> {
    pub device_url: &'a str,
    pub token_url: &'a str,
    pub client_id: &'a str,
    pub scopes: &'a [String],
    pub request_timeout: Duration,
}

#[derive(Clone, Copy, Debug, thiserror::Error)]
pub(crate) enum DeviceAuthorizationError {
    #[error("Device authorization expired; start sign-in again")]
    Expired,
    #[error("Device authorization was denied; start sign-in again when ready")]
    Denied,
    #[error("The gateway returned an invalid device verification URL")]
    InvalidVerificationUrl,
    #[error(transparent)]
    Policy(NetworkPolicyDenied),
}

// Adapts oauth2's async HTTP interface to Codex's proxy and network-policy-aware client.
// Like oauth2's reqwest adapter, this only translates requests and responses:
// https://github.com/ramosbugs/oauth2-rs/blob/f3424b4b2190c83c6d031fdc71eed2351d49e0df/src/reqwest_client.rs#L6-L38
struct DeviceHttpClient {
    client: RouteAwareClientPool,
    request_timeout: Duration,
}

impl<'c> AsyncHttpClient<'c> for DeviceHttpClient {
    type Error = io::Error;
    type Future =
        Pin<Box<dyn Future<Output = Result<oauth2::HttpResponse, Self::Error>> + Send + 'c>>;

    fn call(&'c self, request: oauth2::HttpRequest) -> Self::Future {
        Box::pin(async move {
            let (parts, body) = request.into_parts();
            let response = self
                .client
                .request(parts.method, parts.uri.to_string())
                .headers(parts.headers)
                .body(body)
                .timeout(self.request_timeout)
                .send()
                .await
                .map_err(|_| io::Error::other("OAuth HTTP request failed"))?;
            let status = response.status();
            let headers = response.headers().clone();
            let body = bounded_response_bytes(response, MAX_OAUTH_RESPONSE_BYTES)
                .await
                .map_err(|_| io::Error::other("OAuth HTTP response could not be read"))?;
            let mut response = oauth2::HttpResponse::new(body.to_vec());
            *response.status_mut() = status;
            *response.headers_mut() = headers;
            Ok(response)
        })
    }
}

pub(crate) async fn authorize_device(
    factory: &HttpClientFactory,
    options: DeviceAuthorizationOptions<'_>,
    present: impl FnOnce(&OAuthDeviceCodePrompt),
) -> io::Result<BasicTokenResponse> {
    let oauth = BasicClient::new(ClientId::new(options.client_id.to_owned()))
        .set_device_authorization_url(
            DeviceAuthorizationUrl::new(options.device_url.to_owned())
                .map_err(|_| io::Error::other("Invalid device authorization URL"))?,
        )
        .set_token_uri(
            TokenUrl::new(options.token_url.to_owned())
                .map_err(|_| io::Error::other("Invalid OAuth token URL"))?,
        );
    let http = DeviceHttpClient {
        client: RouteAwareClientPool::new_without_redirects_or_request_logging(
            factory.clone(),
            ClientRouteClass::Auth,
        ),
        request_timeout: options.request_timeout,
    };
    // oauth2 retries transport errors while polling. Report network policy denials immediately.
    let network_policy_denial = |denial| {
        io::Error::new(
            io::ErrorKind::PermissionDenied,
            DeviceAuthorizationError::Policy(denial),
        )
    };
    let permit_for = |url| {
        factory
            .network_policy()
            .acquire(url)
            .map_err(network_policy_denial)
    };
    let device_permit = permit_for(oauth.device_authorization_url().url())?;
    let request = oauth
        .exchange_device_code()
        .add_scopes(options.scopes.iter().cloned().map(Scope::new));
    let authorization: StandardDeviceAuthorizationResponse = device_permit
        .run(request.request_async(&http))
        .await
        .map_err(network_policy_denial)?
        .map_err(|_| io::Error::other("Device authorization request failed"))?;
    let verification_uri = validate_endpoint_url(
        authorization.verification_uri().url().as_str(),
        "device verification URL",
    )
    .map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            DeviceAuthorizationError::InvalidVerificationUrl,
        )
    })?;
    let token_permit = permit_for(oauth.token_uri().url())?;
    present(&OAuthDeviceCodePrompt {
        verification_uri,
        user_code: authorization.user_code().secret().clone(),
    });
    let poll = oauth.exchange_device_access_token(&authorization);
    token_permit
        .run(poll.request_async(&http, tokio::time::sleep, /*timeout*/ None))
        .await
        .map_err(network_policy_denial)?
        .map_err(|error| match error {
            RequestTokenError::ServerResponse(response) => match response.error() {
                DeviceCodeErrorResponseType::AccessDenied => io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    DeviceAuthorizationError::Denied,
                ),
                DeviceCodeErrorResponseType::ExpiredToken => {
                    io::Error::new(io::ErrorKind::TimedOut, DeviceAuthorizationError::Expired)
                }
                _ => io::Error::other("Device authorization failed"),
            },
            _ => io::Error::other("Device authorization failed"),
        })
}
