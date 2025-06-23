//! Helper for fetching the list of models that are available for a given
//! [`ModelProviderInfo`] instance.
//!
//! The implementation is intentionally lightweight and only covers the subset
//! of the OpenAI-compatible REST API that is required to discover available
//! model *identifiers*.  At the time of writing all providers supported by
//! Codex expose a `GET /models` endpoint that returns a JSON payload in the
//! following canonical form:
//!
//! ```json
//! {
//!   "object": "list",
//!   "data": [
//!     { "id": "o3", "object": "model" },
//!     { "id": "o4-mini", "object": "model" }
//!   ]
//! }
//! ```
//!
//! We purposefully parse *only* the `id` fields that callers care about and
//! ignore any additional metadata so that the function keeps working even if
//! upstream providers add new attributes.

use codex_core::ModelProviderInfo;
use codex_core::error::CodexErr;
use codex_core::error::Result;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct ModelsResponse {
    data: Vec<ModelId>,
}

#[derive(Debug, Deserialize)]
struct ModelId {
    id: String,
}

/// Fetch the list of available model identifiers from the given provider.
///
/// The caller must ensure that the provider's API key can be resolved via
/// [`ModelProviderInfo::api_key`] – if this fails the function returns a
/// [`CodexErr::EnvVar`].  Any network or JSON parsing failures are forwarded
/// to the caller.
#[allow(clippy::needless_pass_by_value)]
pub async fn fetch_available_models(provider: ModelProviderInfo) -> Result<Vec<String>> {
    let api_key = provider.api_key()?;

    let base_url = provider.base_url.trim_end_matches('/');
    let url = format!("{base_url}/models");

    // Build the request.  For providers that require authentication we send
    // the token via the standard Bearer mechanism.  Providers like Ollama do
    // not require a token – in that case we just omit the header.
    let client = reqwest::Client::new();
    let mut req = client.get(&url);
    if let Some(token) = api_key {
        req = req.bearer_auth(token);
    }

    let resp = req.send().await?;

    match resp.error_for_status() {
        Ok(ok_resp) => {
            // Guaranteed 2xx
            let json: ModelsResponse = ok_resp.json().await?;
            let mut models: Vec<String> = json.data.into_iter().map(|m| m.id).collect();
            models.sort();
            Ok(models)
        }
        Err(err) => Err(CodexErr::Reqwest(err)),
    }
}
