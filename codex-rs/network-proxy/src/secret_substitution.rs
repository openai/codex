//! Bounded byte-exact secret substitution after destination authorization, before signing.
//! Placeholder bindings and resolved secret values are private to the trusted proxy.

use crate::aws_sigv4::ProxySecretSource;
use anyhow::Result;
use anyhow::anyhow;
use codex_utils_absolute_path::AbsolutePathBuf;
use rama_core::extensions::ExtensionsMut;
use rama_core::extensions::ExtensionsRef;
use rama_http::Body;
use rama_http::HeaderMap;
use rama_http::HeaderValue;
use rama_http::Request;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use std::time::Duration;

// Bound the worst-case number of bytes compared by the placeholder window scans.
const MAX_SUBSTITUTION_SEARCH_BYTES: usize = 256 * 1024 * 1024;

#[derive(Clone, Copy, Debug)]
struct OriginalBodyHeadersSupported(bool);

pub(crate) fn preserve_body_substitution_headers(req: &mut Request) {
    if req
        .extensions()
        .get::<OriginalBodyHeadersSupported>()
        .is_none()
    {
        let supported = body_substitution_headers_supported(req.headers());
        req.extensions_mut()
            .insert(OriginalBodyHeadersSupported(supported));
    }
}

fn body_substitution_headers_supported(headers: &HeaderMap) -> bool {
    let identity_encoding = headers.get_all("content-encoding").iter().all(|value| {
        value.to_str().is_ok_and(|value| {
            value
                .split(',')
                .all(|coding| coding.trim().eq_ignore_ascii_case("identity"))
        })
    });
    identity_encoding
        && crate::body_budget::supported_transfer_framing(headers)
        && !crate::body_budget::is_amazon_event_stream(headers)
        && !headers.contains_key("trailer")
        && !headers.contains_key("x-amz-trailer")
        && !headers.contains_key("x-amz-decoded-content-length")
        && !headers.keys().any(|name| {
            matches!(
                name.as_str(),
                "content-md5"
                    | "digest"
                    | "content-digest"
                    | "repr-digest"
                    | "x-amz-content-sha256"
            ) || name.as_str().starts_with("x-amz-checksum-")
        })
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BodySecretReplacement {
    pub placeholder: String,
    pub secret: ProxySecretSource,
}

/// Exact opaque placeholders scoped by their enclosing host/method/path MITM hook.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BodySecretSubstitutionConfig {
    pub replacements: Vec<BodySecretReplacement>,
    #[serde(default = "https_port")]
    pub port: u16,
    #[serde(default = "default_max_body_bytes")]
    pub max_body_bytes: usize,
}

pub(crate) fn https_port() -> u16 {
    443
}
pub(crate) fn default_max_body_bytes() -> usize {
    8 * 1024 * 1024
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct BodySecretSubstitution {
    config: BodySecretSubstitutionConfig,
    secrets: Vec<Vec<u8>>,
}

impl std::fmt::Debug for BodySecretSubstitution {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BodySecretSubstitution")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl BodySecretSubstitutionConfig {
    pub(crate) fn validate(&self) -> Result<()> {
        if self.port == 0
            || self.max_body_bytes > 16 * 1024 * 1024
            || self.replacements.is_empty()
            || self.replacements.len() > 32
        {
            return Err(anyhow!(
                "invalid body substitution port, bindings, or limit (maximum 16 MiB)"
            ));
        }
        for (index, replacement) in self.replacements.iter().enumerate() {
            if !(16..=256).contains(&replacement.placeholder.len())
                || !replacement.placeholder.is_ascii()
            {
                return Err(anyhow!(
                    "body placeholders must be opaque ASCII markers of 16 to 256 bytes"
                ));
            }
            replacement.secret.validate()?;
            if self.replacements[..index].iter().any(|previous| {
                previous.placeholder.contains(&replacement.placeholder)
                    || replacement.placeholder.contains(&previous.placeholder)
            }) {
                return Err(anyhow!("body substitution bindings overlap"));
            }
        }
        Ok(())
    }

    pub(crate) fn compile(
        &self,
        env: &impl Fn(&str) -> Option<String>,
        file: &impl Fn(&AbsolutePathBuf) -> Result<String>,
    ) -> Result<BodySecretSubstitution> {
        self.validate()?;
        let secrets = self
            .replacements
            .iter()
            .map(|replacement| {
                let secret = replacement.secret.resolve(env, file)?;
                if secret.len() > self.max_body_bytes
                    || self
                        .replacements
                        .iter()
                        .any(|binding| secret.contains(&binding.placeholder))
                {
                    return Err(anyhow!(
                        "body substitution secret exceeds limit or contains a placeholder"
                    ));
                }
                Ok(secret.into_bytes())
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(BodySecretSubstitution {
            config: self.clone(),
            secrets,
        })
    }
}

impl BodySecretSubstitution {
    pub(crate) async fn apply(&self, mut req: Request) -> Result<Request> {
        if req.uri().scheme_str() != Some("https")
            || req.uri().port_u16().unwrap_or(443) != self.config.port
        {
            return Err(anyhow!(
                "body substitution requires its configured HTTPS port"
            ));
        }
        if req
            .extensions()
            .get::<OriginalBodyHeadersSupported>()
            .is_some_and(|original| !original.0)
            || !body_substitution_headers_supported(req.headers())
        {
            return Err(anyhow!(
                "body substitution does not support encoded, streaming-signed, or integrity-protected payloads"
            ));
        }
        let _permit = crate::body_budget::reserve(&mut req).await?;
        let (mut parts, body) = req.into_parts();
        let (body, trailers) = tokio::time::timeout(
            Duration::from_secs(30),
            crate::body_budget::collect_body(body, self.config.max_body_bytes),
        )
        .await
        .map_err(|_| anyhow!("body substitution read timed out"))?
        .map_err(|_| anyhow!("body substitution exceeds limit or cannot read payload"))?;
        if trailers.is_some() {
            return Err(anyhow!(
                "body substitution does not support request trailers"
            ));
        }
        if !crate::body_budget::content_length_matches(&parts.headers, body.len()) {
            return Err(anyhow!(
                "body substitution content length does not match the request body"
            ));
        }
        // Charge every window for its full length before searching, including failed matches.
        let search_bytes = self
            .config
            .replacements
            .iter()
            .try_fold(0usize, |work, binding| {
                let windows = body.len().saturating_sub(binding.placeholder.len() - 1);
                work.checked_add(windows.checked_mul(binding.placeholder.len())?)
            });
        if search_bytes.is_none_or(|work| work > MAX_SUBSTITUTION_SEARCH_BYTES) {
            return Err(anyhow!("body substitution search exceeds work limit"));
        }
        // Search the original bytes only. Replacements cannot recursively expand other bindings.
        let mut matches = Vec::new();
        for (index, binding) in self.config.replacements.iter().enumerate() {
            for (offset, window) in body.windows(binding.placeholder.len()).enumerate() {
                if window == binding.placeholder.as_bytes() {
                    if matches.len() >= 65_536 {
                        return Err(anyhow!("too many body substitution occurrences"));
                    }
                    matches.push((offset, offset + binding.placeholder.len(), index));
                }
            }
        }
        matches.sort_unstable();
        let mut cursor = 0;
        let mut size = body.len();
        for &(start, end, index) in &matches {
            if start < cursor {
                return Err(anyhow!("body substitution matches overlap"));
            }
            size = size
                .checked_sub(end - start)
                .and_then(|size| size.checked_add(self.secrets[index].len()))
                .ok_or_else(|| anyhow!("expanded body substitution exceeds limit"))?;
            cursor = end;
        }
        if size > self.config.max_body_bytes {
            return Err(anyhow!("expanded body substitution exceeds limit"));
        }
        let mut output = Vec::with_capacity(size);
        cursor = 0;
        for (start, end, index) in matches {
            output.extend_from_slice(&body[cursor..start]);
            output.extend_from_slice(&self.secrets[index]);
            cursor = end;
        }
        output.extend_from_slice(&body[cursor..]);
        parts.headers.remove("transfer-encoding");
        parts.headers.remove("trailer");
        parts.headers.insert(
            "content-length",
            HeaderValue::from_str(&output.len().to_string())?,
        );
        Ok(crate::body_budget::retain(Request::from_parts(
            parts,
            Body::from(output),
        )))
    }
}

#[cfg(test)]
#[path = "secret_substitution_tests.rs"]
mod tests;
