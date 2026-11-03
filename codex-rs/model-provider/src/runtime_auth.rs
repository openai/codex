//! Keep model requests and catalog refreshes bound to the same registered actor.

use codex_api::AuthError;
use codex_features::RuntimeAuthMode;
use codex_http_client::CHATGPT_ACCOUNT_ID_HEADER;
use codex_http_client::OPENAI_ORGANIZATION_HEADER;
use codex_http_client::OPENAI_PROJECT_HEADER;
use codex_http_client::OpenAiDestination;
use codex_http_client::classify_openai_api_url;
use codex_login::AuthManager;
use codex_model_provider_info::ModelProviderInfo;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result as CodexResult;
use codex_runtime_agent_instance::ActorInfo;
use codex_runtime_agent_instance::PlatformResourceSelection;
use codex_runtime_agent_instance::RuntimeAuth;
use codex_runtime_agent_instance::RuntimeAuthError;
use codex_runtime_agent_instance::RuntimeAuthManager;
use http::HeaderMap;
use std::sync::Arc;

/// Select the runtime for the resource scope carried by the model request.
pub(crate) async fn select_runtime_auth(
    manager: &RuntimeAuthManager,
    headers: &HeaderMap,
) -> Result<RuntimeAuth, AuthError> {
    manager
        .auth_with_selection(&resource_selection(headers)?)
        .await
        .map_err(AuthError::from)
}

/// Select fresh request auth without allowing a reusable client's source to change.
pub async fn select_runtime_auth_for_source(
    manager: &Arc<RuntimeAuthManager>,
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

/// A runtime may not replace an independently selected provider credential.
pub fn validate_runtime_provider(
    info: &ModelProviderInfo,
    credentials: Option<&Arc<AuthManager>>,
    runtime: &RuntimeAuthManager,
) -> CodexResult<()> {
    if !info.requires_openai_auth
        || info.env_key.is_some()
        || info.experimental_bearer_token.is_some()
        || info.auth.is_some()
        || info.aws.is_some()
        || !credentials.is_some_and(|auth| runtime.uses_credentials(auth))
    {
        return Err(CodexErr::UnsupportedOperation(
            "runtime auth requires the model provider's selected OpenAI credential".to_string(),
        ));
    }
    Ok(())
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

/// Select runtime auth for supported OpenAI URLs; reject unsupported OpenAI URLs.
pub fn runtime_auth_for_url<'a>(
    runtime: Option<&'a (Arc<RuntimeAuthManager>, RuntimeAuthMode)>,
    url: &str,
) -> CodexResult<Option<&'a (Arc<RuntimeAuthManager>, RuntimeAuthMode)>> {
    if runtime.is_some() {
        validate_runtime_destination(Some(url))?;
    }
    Ok(runtime.filter(|_| classify_openai_api_url(url) == OpenAiDestination::Supported))
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
