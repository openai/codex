//! Gateway credentials and their delivery alongside the provider's primary authentication.

use codex_client::CHATGPT_ACCOUNT_ID_HEADER;
use std::fmt;

use codex_http_client::validate_endpoint_url;
use http::header::HeaderName;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;

use crate::CredentialExportConfig;
use crate::ModelProviderInfo;
use crate::OAuthCredentialsStore;

// Reserve authentication, routing/framing, and internal protocol headers.
const RESERVED_GATEWAY_OAUTH_HEADERS: &[&str] = &[
    "authorization",
    "proxy-authorization",
    "cookie",
    "host",
    "content-length",
    "transfer-encoding",
    "connection",
    "upgrade",
    CHATGPT_ACCOUNT_ID_HEADER,
];
const RESERVED_GATEWAY_OAUTH_HEADER_PREFIXES: &[&str] =
    &["x-codex-", "x-openai-", "sec-websocket-"];

#[derive(Clone, Deserialize, Serialize, PartialEq, Eq, JsonSchema)]
#[serde(
    untagged,
    deny_unknown_fields,
    expecting = "choose one gateway credential source: credential_export or OAuth settings (authorization_url, token_url, and client_id)"
)]
pub enum GatewayOAuthConfig {
    OAuth(GatewayOAuthSettings),
    Export {
        /// Command that prints a gateway access token.
        credential_export: CredentialExportConfig,
        #[serde(default)]
        delivery: GatewayOAuthDelivery,
    },
}

#[derive(Clone, Deserialize, Serialize, PartialEq, Eq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GatewayOAuthSettings {
    pub authorization_url: String,
    /// Optional device endpoint used when the terminal cannot open a browser for gateway sign-in.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_authorization_url: Option<String>,
    pub token_url: String,
    pub client_id: String,
    pub resource: Option<String>,
    #[serde(default)]
    pub scopes: Vec<String>,
    pub redirect_port: Option<u16>,
    /// Where Codex stores this gateway's OAuth credentials. Defaults to the keyring-backed store.
    #[serde(default, skip_serializing_if = "is_keyring")]
    pub credentials_store: OAuthCredentialsStore,
    /// Omit to send the gateway token via the Authorization header
    #[serde(default)]
    pub delivery: GatewayOAuthDelivery,
}

// Preserve the serialized provider and model cache identity for the default store.
#[expect(
    clippy::trivially_copy_pass_by_ref,
    reason = "Serde passes a reference to skip_serializing_if predicates"
)]
fn is_keyring(store: &OAuthCredentialsStore) -> bool {
    *store == OAuthCredentialsStore::Keyring
}

impl fmt::Debug for GatewayOAuthConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GatewayOAuthConfig")
            .field("delivery", self.delivery())
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum GatewayOAuthDelivery {
    Header {
        name: String,
        #[serde(default = "bearer_scheme")]
        scheme: String,
    },
    Cookie {
        name: String,
    },
}

impl Default for GatewayOAuthDelivery {
    fn default() -> Self {
        Self::Header {
            name: "Authorization".into(),
            scheme: bearer_scheme(),
        }
    }
}

impl GatewayOAuthDelivery {
    fn is_authorization_bearer(&self) -> bool {
        matches!(self, Self::Header { name, scheme } if name.eq_ignore_ascii_case("authorization") && scheme.eq_ignore_ascii_case("bearer"))
    }

    fn secondary_header(&self) -> Result<HeaderName, &'static str> {
        match self {
            Self::Header { name, scheme } => {
                let header = HeaderName::from_bytes(name.as_bytes())
                    .map_err(|_| "needs a valid HTTP header name, such as X-Gateway-Auth")?;
                if RESERVED_GATEWAY_OAUTH_HEADERS.contains(&header.as_str())
                    || RESERVED_GATEWAY_OAUTH_HEADER_PREFIXES
                        .iter()
                        .any(|prefix| header.as_str().starts_with(prefix))
                {
                    return Err(
                        "uses a reserved header. Choose a custom header or cookie accepted by your gateway",
                    );
                }
                if !is_token(scheme) {
                    return Err("needs an HTTP authentication scheme such as Bearer");
                }
                Ok(header)
            }
            Self::Cookie { name } if is_token(name) => Ok(http::header::COOKIE),
            Self::Cookie { .. } => Err("needs a valid cookie name, such as gateway_session"),
        }
    }
}

fn bearer_scheme() -> String {
    "Bearer".to_owned()
}

impl GatewayOAuthConfig {
    pub fn delivery(&self) -> &GatewayOAuthDelivery {
        match self {
            Self::OAuth(settings) => &settings.delivery,
            Self::Export { delivery, .. } => delivery,
        }
    }

    pub fn uses_credential_export(&self) -> bool {
        matches!(self, Self::Export { .. })
    }

    /// Send the gateway bearer only to the configured endpoint.
    pub fn requires_gateway_origin(&self) -> bool {
        self.delivery().is_authorization_bearer() || self.uses_credential_export()
    }

    pub(crate) fn validate(&self, provider: &ModelProviderInfo) -> Result<(), String> {
        if provider.aws.is_some() || provider.is_amazon_bedrock() {
            return Err("provider gateway_oauth cannot be combined with AWS authentication".into());
        }
        let base_url = provider
            .base_url
            .as_deref()
            .ok_or("gateway_oauth requires base_url")?;
        let gateway = validate_endpoint_url(base_url, "base_url")?;
        match self {
            Self::Export {
                credential_export, ..
            } => credential_export.validate("gateway_oauth.credential_export")?,
            Self::OAuth(settings) => {
                validate_endpoint_url(
                    &settings.authorization_url,
                    "gateway_oauth.authorization_url",
                )?;
                if let Some(url) = &settings.device_authorization_url {
                    validate_endpoint_url(url, "gateway_oauth.device_authorization_url")?;
                }
                validate_endpoint_url(&settings.token_url, "gateway_oauth.token_url")?;
                if settings.client_id.trim().is_empty() || settings.redirect_port == Some(0) {
                    return Err(
                        "gateway_oauth requires a nonempty client_id and a nonzero redirect_port"
                            .into(),
                    );
                }
            }
        }
        if self.requires_gateway_origin()
            && let Some(catalog) = provider.model_catalog_url.as_deref()
            && validate_endpoint_url(catalog, "model_catalog_url")?.origin() != gateway.origin()
        {
            return Err(
                "Codex would send gateway credentials to a different model catalog. Set model_catalog_url \
                 to a URL with the same scheme, host, and port as base_url, or remove model_catalog_url \
                 to use the gateway's catalog."
                    .into(),
            );
        }
        let gateway_header = if self.delivery().is_authorization_bearer() {
            if provider.is_gcp() {
                return Err("Google Cloud uses the Authorization header for its own token. Remove gateway_oauth to use the built-in Google Cloud provider".into());
            }
            if provider.requires_openai_auth
                || provider.env_key.is_some()
                || provider.experimental_bearer_token.is_some()
                || provider.has_command_auth()
            {
                return Err(
                    "Header conflict. Codex is configured to send both the model provider credential \
                     and the gateway token in the Authorization header. Set gateway_oauth.delivery \
                     to a different header or cookie that your gateway accepts for its token."
                        .into(),
                );
            }
            http::header::AUTHORIZATION
        } else {
            self.delivery()
                .secondary_header()
                .map_err(|reason| format!("gateway_oauth.delivery {reason}"))?
        };
        if provider
            .http_headers
            .iter()
            .flat_map(|headers| headers.keys())
            .chain(
                provider
                    .env_http_headers
                    .iter()
                    .flat_map(|headers| headers.keys()),
            )
            .any(|name| name.eq_ignore_ascii_case(gateway_header.as_str()))
        {
            return Err(format!(
                "The {gateway_header} header is set in both gateway_oauth.delivery and the provider's http_headers or env_http_headers. Remove the duplicate header from the provider or choose a different gateway delivery."
            ));
        }
        Ok(())
    }
}

fn is_token(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&byte))
}

#[cfg(test)]
#[path = "gateway_oauth_tests.rs"]
mod tests;
