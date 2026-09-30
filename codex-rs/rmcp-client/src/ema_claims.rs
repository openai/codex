//! Validate OIDC identity assertion bindings before storing enterprise credentials.

use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use anyhow::Result;
use anyhow::anyhow;
use anyhow::bail;
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use serde::Deserialize;
use serde::de::DeserializeOwned;

pub(crate) const ID_JAG_TOKEN_TYPE: &str = "urn:ietf:params:oauth:token-type:id-jag";

#[derive(Deserialize)]
struct JwtHeader {
    alg: String,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum OAuthResource {
    Single(String),
    Multiple(Vec<String>),
}

fn signed_jwt<T: DeserializeOwned>(token: &str) -> Result<T> {
    let mut parts = token.split('.');
    let (Some(header), Some(payload), Some(signature), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        bail!("identity assertion is not a compact signed JWT");
    };
    if header.is_empty() || payload.is_empty() || signature.is_empty() {
        bail!("identity assertion contains an empty JWT segment");
    }
    let header: JwtHeader = serde_json::from_slice(&URL_SAFE_NO_PAD.decode(header)?)
        .map_err(|_| anyhow!("invalid identity assertion JWT header"))?;
    if header.alg.trim().is_empty() || header.alg.eq_ignore_ascii_case("none") {
        bail!("identity assertion is unsigned");
    }
    let claims = serde_json::from_slice(&URL_SAFE_NO_PAD.decode(payload)?)
        .map_err(|_| anyhow!("invalid identity assertion JWT claims"))?;
    Ok(claims)
}

#[derive(Deserialize)]
pub(crate) struct OidcClaims {
    iss: String,
    sub: String,
    aud: OAuthResource,
    azp: Option<String>,
    exp: u64,
}

pub(crate) fn oidc_identity(
    assertion: &str,
    expected_issuer: &str,
    expected_audience: &str,
) -> Result<OidcClaims> {
    let claims: OidcClaims = signed_jwt(assertion)?;
    if claims.iss != expected_issuer || claims.sub.trim().is_empty() {
        bail!("OIDC identity assertion issuer or subject does not match the enterprise IdP");
    }
    let (audience_matches, multiple_audiences) = match &claims.aud {
        OAuthResource::Single(value) => (value == expected_audience, false),
        OAuthResource::Multiple(values) => (
            values.iter().any(|value| value == expected_audience),
            values.len() > 1,
        ),
    };
    if !audience_matches
        || claims
            .azp
            .as_deref()
            .is_some_and(|party| party != expected_audience)
        || multiple_audiences && claims.azp.as_deref() != Some(expected_audience)
    {
        bail!("OIDC identity assertion audience or authorized party does not match the IdP client");
    }
    Ok(claims)
}

pub(crate) fn validate_oidc_identity_assertion(
    assertion: &str,
    expected_issuer: &str,
    expected_audience: &str,
) -> Result<()> {
    let claims = oidc_identity(assertion, expected_issuer, expected_audience)?;
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    if claims.exp <= now {
        bail!("OIDC identity assertion is expired");
    }
    Ok(())
}
