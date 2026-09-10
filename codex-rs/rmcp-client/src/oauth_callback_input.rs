//! Accepts a pasted OAuth redirect without navigating to it. The prepared callback address
//! is checked before the existing OAuth flow validates state/issuer and exchanges the code.

use super::CallbackResult;
use super::OAuthHttpContext;
use super::OAuthLoginPurpose;
use super::OAuthProviderError;
use super::OauthCallbackResult;
use super::OauthLoginFlow;
use crate::McpOAuthClientRegistration;
use crate::StreamableHttpRedirectMode;
use crate::save_oauth_tokens;
use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use anyhow::bail;
use codex_config::types::AuthKeyringBackendKind;
use codex_config::types::OAuthCredentialsStoreMode;
use codex_exec_server::HttpClient;
use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;
use tokio::time::timeout;
use url::Url;

/// Runs MCP OAuth without opening a browser, accepting either the HTTP callback or a
/// full redirect URL returned by `read_callback`. The reader receives the authorization
/// URL and must release terminal state when its future is dropped (callback or timeout).
#[allow(clippy::too_many_arguments)]
pub async fn perform_oauth_login_with_callback_input<F>(
    server_name: &str,
    server_url: &str,
    store_mode: OAuthCredentialsStoreMode,
    keyring_backend_kind: AuthKeyringBackendKind,
    http_headers: Option<HashMap<String, String>>,
    env_http_headers: Option<HashMap<String, String>>,
    scopes: &[String],
    oauth_client_id: Option<&str>,
    client_registration: McpOAuthClientRegistration,
    oauth_resource: Option<&str>,
    callback_port: Option<u16>,
    callback_url: Option<&str>,
    global_callback_url: Option<&str>,
    http_client: Arc<dyn HttpClient>,
    read_callback: impl FnOnce(String) -> F,
) -> Result<()>
where
    F: Future<Output = Result<String>>,
{
    let mut flow = OauthLoginFlow::new(
        server_name,
        server_url,
        store_mode,
        keyring_backend_kind,
        OAuthHttpContext {
            http_headers,
            env_http_headers,
            http_client,
            redirect_mode: StreamableHttpRedirectMode::Legacy,
        },
        scopes,
        oauth_client_id,
        OAuthLoginPurpose::Mcp,
        client_registration,
        oauth_resource,
        /*launch_browser*/ false,
        callback_port,
        callback_url,
        global_callback_url,
        /*timeout_secs*/ None,
    )
    .await?;
    let authorization_url = flow.authorization_url();
    let callback = timeout(flow.timeout, async {
        tokio::select! {
            callback = &mut flow.rx => callback.context("OAuth callback was cancelled"),
            input = read_callback(authorization_url.clone()) => {
                parse_callback_url(&input?, &flow.redirect_uri, &authorization_url)
            }
        }
    })
    .await
    .context("timed out waiting for OAuth callback")??;
    // RMCP's issuer-mismatch error includes the received value. Reject it here
    // without echoing any part of a pasted callback into terminal diagnostics.
    if let CallbackResult::Success(callback) = &callback
        && callback
            .issuer
            .as_deref()
            .is_some_and(|issuer| Some(issuer) != flow.authorization_server_issuer.as_deref())
    {
        bail!("OAuth callback issuer does not match this login");
    }
    let stored = flow.complete_callback(callback).await?;
    save_oauth_tokens(server_name, &stored, store_mode, keyring_backend_kind).await
}

fn parse_callback_url(
    input: &str,
    redirect_uri: &str,
    authorization_url: &str,
) -> Result<CallbackResult> {
    if input.len() > 64 * 1024 {
        bail!("OAuth callback URL exceeds 64 KiB");
    }
    let mut callback = Url::parse(input.trim()).context("Invalid OAuth callback URL")?;
    let mut expected = Url::parse(redirect_uri).context("Invalid OAuth redirect URI")?;
    if callback.fragment().is_some()
        || !callback.username().is_empty()
        || callback.password().is_some()
    {
        bail!("OAuth callback URL must not contain credentials or a fragment");
    }
    let mut response_params: Vec<_> = callback.query_pairs().into_owned().collect();
    // Configured redirect query parameters must survive unchanged. OAuth response
    // parameters are appended by the authorization server, not part of that address.
    let expected_params: Vec<_> = expected.query_pairs().into_owned().collect();
    for expected_param in &expected_params {
        let position = response_params
            .iter()
            .position(|param| param == expected_param)
            .ok_or_else(|| {
                anyhow!("OAuth callback URL does not match this login's redirect URI")
            })?;
        response_params.remove(position);
    }
    if response_params
        .iter()
        .any(|(name, _)| expected_params.iter().any(|(key, _)| key == name))
    {
        bail!("OAuth callback URL changes this login's redirect query parameters");
    }
    callback.set_query(/*query*/ None);
    expected.set_query(/*query*/ None);
    if callback != expected {
        bail!("OAuth callback URL does not match this login's redirect URI");
    }

    let mut params = HashMap::new();
    for (name, value) in response_params {
        if params.insert(name, value).is_some() {
            bail!("OAuth callback URL contains duplicate parameters");
        }
    }
    let state = params
        .remove("state")
        .filter(|state| !state.is_empty())
        .ok_or_else(|| anyhow!("OAuth callback URL is missing state"))?;
    if params.contains_key("error") {
        if params.contains_key("code") {
            bail!("OAuth callback URL contains both a code and an error");
        }
        let authorization = Url::parse(authorization_url)?;
        if !authorization
            .query_pairs()
            .any(|(key, value)| key == "state" && value == state)
        {
            bail!("OAuth callback state does not match this login");
        }
        // Do not print arbitrary pasted error descriptions or other callback values.
        return Ok(CallbackResult::Error(OAuthProviderError::new(
            /*error*/ None, /*error_description*/ None,
        )));
    }
    let code = params
        .remove("code")
        .filter(|code| !code.is_empty())
        .ok_or_else(|| anyhow!("OAuth callback URL is missing an authorization code"))?;
    Ok(CallbackResult::Success(OauthCallbackResult {
        code,
        state,
        issuer: params.remove("iss"),
    }))
}

#[cfg(test)]
#[path = "oauth_callback_input_tests.rs"]
mod tests;
