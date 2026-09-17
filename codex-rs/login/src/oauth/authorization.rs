//! Builds authorization requests and validates callback state before consuming codes or errors.

use base64::Engine;
use rand::RngCore;
use url::Url;

use crate::oauth::PkceCodes;

/// Standard authorization parameters plus issuer-specific extensions supplied by the caller.
pub(crate) struct AuthorizationRequest<'a> {
    pub endpoint: &'a str,
    pub client_id: &'a str,
    pub redirect_uri: &'a str,
    pub scope: Option<&'a str>,
    pub resource: Option<&'a str>,
    pub pkce: &'a PkceCodes,
    pub state: &'a str,
    pub extra_parameters: &'a [(&'a str, &'a str)],
}

pub(crate) fn build_authorization_url(
    request: AuthorizationRequest<'_>,
) -> Result<Url, url::ParseError> {
    let mut url = Url::parse(request.endpoint)?;
    {
        let mut query = url.query_pairs_mut();
        query.append_pair("response_type", "code");
        query.append_pair("client_id", request.client_id);
        query.append_pair("redirect_uri", request.redirect_uri);
        query.append_pair("code_challenge", &request.pkce.code_challenge);
        query.append_pair("code_challenge_method", "S256");
        query.append_pair("state", request.state);
        if let Some(scope) = request.scope {
            query.append_pair("scope", scope);
        }
        if let Some(resource) = request.resource {
            query.append_pair("resource", resource);
        }
        query.extend_pairs(request.extra_parameters.iter().copied());
    }
    Ok(url)
}

pub(crate) fn generate_state() -> String {
    let mut bytes = [0_u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

/// Callback parameters are kept out of Debug output because code and state are credentials.
#[derive(Default)]
pub(crate) struct CallbackParameters {
    pub code: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
    pub error_description: Option<String>,
}

pub(crate) enum CallbackError<'a> {
    StateMismatch,
    Provider {
        code: &'a str,
        description: Option<&'a str>,
    },
    MissingCode,
}

impl CallbackParameters {
    pub(crate) fn from_url(url: &Url) -> Self {
        let mut params = Self::default();
        for (name, value) in url.query_pairs() {
            match name.as_ref() {
                "code" => params.code = Some(value.into_owned()),
                "state" => params.state = Some(value.into_owned()),
                "error" => params.error = Some(value.into_owned()),
                "error_description" => params.error_description = Some(value.into_owned()),
                _ => {}
            }
        }
        params
    }

    /// Checks state before accepting an authorization code or provider error.
    pub(crate) fn validate(&self, expected_state: &str) -> Result<&str, CallbackError<'_>> {
        if self.state.as_deref() != Some(expected_state) {
            return Err(CallbackError::StateMismatch);
        }
        if let Some(code) = self.error.as_deref() {
            return Err(CallbackError::Provider {
                code,
                description: self.error_description.as_deref(),
            });
        }
        let code = self
            .code
            .as_deref()
            .filter(|code| !code.is_empty())
            .ok_or(CallbackError::MissingCode)?;
        Ok(code)
    }
}

#[cfg(test)]
#[path = "authorization_tests.rs"]
mod tests;
