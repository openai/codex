//! Keep model requests and catalog refreshes bound to the same registered actor.

use codex_api::AuthError;
use codex_features::RuntimeAuthMode;
use codex_http_client::CHATGPT_ACCOUNT_ID_HEADER;
use codex_http_client::OPENAI_ORGANIZATION_HEADER;
use codex_http_client::OPENAI_PROJECT_HEADER;
use codex_http_client::OpenAiDestination;
use codex_http_client::classify_openai_api_url;
use codex_login::AuthManager;
use codex_login::CodexAuth;
use codex_model_provider_info::ModelProviderInfo;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result as CodexResult;
use codex_runtime_agent_instance::ActorInfo;
use codex_runtime_agent_instance::PlatformResourceSelection;
use codex_runtime_agent_instance::RuntimeAuth;
use codex_runtime_agent_instance::RuntimeAuthBinding;
use codex_runtime_agent_instance::RuntimeAuthError;
use codex_runtime_agent_instance::RuntimeAuthManager;
use http::HeaderMap;
use std::sync::Arc;

/// Select the runtime for the resource scope carried by the model request.
pub(crate) async fn select_runtime_auth(
    manager: &Arc<RuntimeAuthBinding>,
    headers: &HeaderMap,
) -> Result<RuntimeAuth, AuthError> {
    manager
        .auth_with_selection(&resource_selection(headers)?)
        .await
        .map_err(AuthError::from)
}

/// Select the request's resource scope while preserving captured credential ownership.
pub async fn select_runtime_auth_for(
    binding: &Arc<RuntimeAuthBinding>,
    auth: &CodexAuth,
    headers: &HeaderMap,
) -> Result<RuntimeAuth, AuthError> {
    binding
        .auth_for(auth, &resource_selection(headers)?)
        .await
        .map_err(AuthError::from)
}

/// Select fresh request auth without allowing a reusable client's source to change.
pub(crate) async fn select_runtime_auth_for_source(
    manager: &Arc<RuntimeAuthBinding>,
    source_generation: u64,
    headers: &HeaderMap,
) -> Result<RuntimeAuth, AuthError> {
    let manager = Arc::clone(manager);
    let selection = resource_selection(headers)?;
    finish_runtime_selection(
        async move { manager.auth_for_source(source_generation, &selection).await },
    )
    .await
}

// Once token rotation begins, let it finish persisting even if the request is cancelled.
pub(crate) async fn finish_runtime_selection(
    selection: impl Future<Output = Result<RuntimeAuth, RuntimeAuthError>> + Send + 'static,
) -> Result<RuntimeAuth, AuthError> {
    tokio::spawn(selection)
        .await
        .map_err(|error| AuthError::Build(format!("runtime auth task failed: {error}")))?
        .map_err(AuthError::from)
}

pub(crate) fn resource_selection(
    headers: &HeaderMap,
) -> Result<PlatformResourceSelection, AuthError> {
    let selector = |name| {
        headers
            .get(name)
            .map(|value| {
                value
                    .to_str()
                    .map(str::to_owned)
                    .map_err(|_| AuthError::Build(format!("invalid {name} resource selector")))
            })
            .transpose()
    };
    Ok(PlatformResourceSelection {
        organization_id: selector(OPENAI_ORGANIZATION_HEADER)?,
        project_id: selector(OPENAI_PROJECT_HEADER)?,
    })
}

/// Use the credential selected by provider configuration, never an ambient actor's proof.
pub fn select_runtime_provider(
    info: &ModelProviderInfo,
    credentials: Option<&Arc<AuthManager>>,
    runtime: &Arc<RuntimeAuthManager>,
) -> CodexResult<Arc<RuntimeAuthBinding>> {
    let unsupported = || {
        CodexErr::UnsupportedOperation(
            "runtime auth requires the model provider's selected OpenAI credential".to_string(),
        )
    };
    if info.aws.is_some() {
        return Err(unsupported());
    }
    let credentials = credentials.ok_or_else(unsupported)?;
    if let Some(bearer) = crate::auth::bearer_auth_for_provider(info)? {
        let token = bearer.token.ok_or_else(unsupported)?;
        return Ok(runtime.bind_api_key(&token, &credentials.runtime_config()));
    }
    if !info.requires_openai_auth && info.auth.is_none() {
        return Err(unsupported());
    }
    Ok(runtime.bind(Arc::clone(credentials)))
}

/// An unrecognized OpenAI origin must not silently fall back to legacy auth.
pub fn validate_runtime_destination(destination: Option<&str>) -> CodexResult<()> {
    if destination.is_some_and(|url| classify_openai_api_url(url) == OpenAiDestination::Unsupported)
    {
        return Err(CodexErr::UnsupportedOperation(
            "runtime auth does not support this OpenAI endpoint".to_string(),
        ));
    }
    Ok(())
}

/// Do not send a provider's credentials to registration in another OpenAI environment.
pub fn validate_runtime_environment(
    runtime: &RuntimeAuthManager,
    destination: Option<&str>,
) -> CodexResult<()> {
    let registration_url = runtime
        .registration_url()
        .map_err(|err| CodexErr::InvalidRequest(err.to_string()))?;
    let registration_is_staging = match registration_url
        .host_str()
        .map(|host| host.trim_end_matches('.'))
    {
        Some("auth.openai.com") => false,
        Some("auth.api.openai.org") => true,
        // Explicit registration targets also support local test servers.
        _ => return Ok(()),
    };
    let destination_is_staging = match destination {
        Some(destination) => {
            let destination = url::Url::parse(destination)
                .map_err(|_| CodexErr::InvalidRequest("invalid runtime auth destination".into()))?;
            let host = destination
                .host_str()
                .unwrap_or_default()
                .trim_end_matches('.');
            matches!(
                host,
                "api.openai.org" | "api.openai-staging.com" | "chatgpt-staging.com"
            ) || host.ends_with(".chatgpt-staging.com")
        }
        // Both implicit model-provider defaults use production.
        None => false,
    };
    if registration_is_staging != destination_is_staging {
        return Err(CodexErr::InvalidRequest(
            "Requests must use the same production or staging environment as runtime registration"
                .into(),
        ));
    }
    Ok(())
}

/// Select runtime auth for supported OpenAI URLs; reject unsupported OpenAI URLs.
pub fn runtime_auth_for_url<'a>(
    runtime: Option<&'a (Arc<RuntimeAuthManager>, RuntimeAuthMode)>,
    url: &str,
) -> CodexResult<Option<&'a (Arc<RuntimeAuthManager>, RuntimeAuthMode)>> {
    if runtime.is_some() {
        validate_runtime_destination(Some(url))?;
    }
    let runtime = runtime.filter(|_| classify_openai_api_url(url) == OpenAiDestination::Supported);
    if let Some((manager, _)) = runtime {
        validate_runtime_environment(manager, Some(url))?;
    }
    Ok(runtime)
}

/// Account and organization selectors must agree with the registered actor.
/// API project access is authorized by the backend for each request.
pub fn validate_runtime_selectors(headers: &HeaderMap, actor: &ActorInfo) -> CodexResult<()> {
    let validate = |name: &str, expected: Option<&str>| {
        if let Some(value) = headers.get(name)
            && value.to_str().ok() != expected
        {
            return Err(CodexErr::UnsupportedOperation(format!(
                "configured {name} does not match the registered runtime"
            )));
        }
        Ok(())
    };
    match actor {
        ActorInfo::Chatgpt { account_id, .. } => {
            validate(CHATGPT_ACCOUNT_ID_HEADER, Some(account_id))?;
            validate(OPENAI_ORGANIZATION_HEADER, None)?;
            validate(OPENAI_PROJECT_HEADER, None)
        }
        ActorInfo::Platform {
            organization_id, ..
        } => {
            validate(CHATGPT_ACCOUNT_ID_HEADER, None)?;
            validate(OPENAI_ORGANIZATION_HEADER, Some(organization_id))
        }
    }
}

#[cfg(test)]
#[path = "runtime_auth_tests.rs"]
mod tests;
