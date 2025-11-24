use crate::{
    revocation::{RevocationErrorResponseType, StandardRevocableToken},
    Client, EmptyExtraTokenFields, EndpointNotSet, ErrorResponseType, RequestTokenError,
    StandardErrorResponse, StandardTokenIntrospectionResponse, StandardTokenResponse, TokenType,
};

use std::fmt::Error as FormatterError;
use std::fmt::{Debug, Display, Formatter};

/// Basic OAuth2 client specialization, suitable for most applications.
pub type BasicClient<
    HasAuthUrl = EndpointNotSet,
    HasDeviceAuthUrl = EndpointNotSet,
    HasIntrospectionUrl = EndpointNotSet,
    HasRevocationUrl = EndpointNotSet,
    HasTokenUrl = EndpointNotSet,
> = Client<
    BasicErrorResponse,
    BasicTokenResponse,
    BasicTokenIntrospectionResponse,
    StandardRevocableToken,
    BasicRevocationErrorResponse,
    HasAuthUrl,
    HasDeviceAuthUrl,
    HasIntrospectionUrl,
    HasRevocationUrl,
    HasTokenUrl,
>;

/// Basic OAuth2 authorization token types.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BasicTokenType {
    /// Bearer token
    /// ([OAuth 2.0 Bearer Tokens - RFC 6750](https://tools.ietf.org/html/rfc6750)).
    Bearer,
    /// MAC ([OAuth 2.0 Message Authentication Code (MAC)
    /// Tokens](https://tools.ietf.org/html/draft-ietf-oauth-v2-http-mac-05)).
    Mac,
    /// An extension not defined by RFC 6749.
    Extension(String),
}
impl BasicTokenType {
    fn from_str(s: &str) -> Self {
        match s {
            "bearer" => BasicTokenType::Bearer,
            "mac" => BasicTokenType::Mac,
            ext => BasicTokenType::Extension(ext.to_string()),
        }
    }
}
impl AsRef<str> for BasicTokenType {
    fn as_ref(&self) -> &str {
        match *self {
            BasicTokenType::Bearer => "bearer",
            BasicTokenType::Mac => "mac",
            BasicTokenType::Extension(ref ext) => ext.as_str(),
        }
    }
}
impl<'de> serde::Deserialize<'de> for BasicTokenType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        let variant_str = String::deserialize(deserializer)?;
        Ok(Self::from_str(&variant_str))
    }
}
impl serde::ser::Serialize for BasicTokenType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::ser::Serializer,
    {
        serializer.serialize_str(self.as_ref())
    }
}
impl TokenType for BasicTokenType {}

/// Basic OAuth2 token response.
pub type BasicTokenResponse = StandardTokenResponse<EmptyExtraTokenFields, BasicTokenType>;

/// Basic OAuth2 token introspection response.
pub type BasicTokenIntrospectionResponse =
    StandardTokenIntrospectionResponse<EmptyExtraTokenFields, BasicTokenType>;

/// Basic access token error types.
///
/// These error types are defined in
/// [Section 5.2 of RFC 6749](https://tools.ietf.org/html/rfc6749#section-5.2).
#[derive(Clone, PartialEq, Eq)]
pub enum BasicErrorResponseType {
    /// Client authentication failed (e.g., unknown client, no client authentication included,
    /// or unsupported authentication method).
    InvalidClient,
    /// The provided authorization grant (e.g., authorization code, resource owner credentials)
    /// or refresh token is invalid, expired, revoked, does not match the redirection URI used
    /// in the authorization request, or was issued to another client.
    InvalidGrant,
    /// The request is missing a required parameter, includes an unsupported parameter value
    /// (other than grant type), repeats a parameter, includes multiple credentials, utilizes
    /// more than one mechanism for authenticating the client, or is otherwise malformed.
    InvalidRequest,
    /// The requested scope is invalid, unknown, malformed, or exceeds the scope granted by the
    /// resource owner.
    InvalidScope,
    /// The authenticated client is not authorized to use this authorization grant type.
    UnauthorizedClient,
    /// The authorization grant type is not supported by the authorization server.
    UnsupportedGrantType,
    /// An extension not defined by RFC 6749.
    Extension(String),
}
impl BasicErrorResponseType {
    pub(crate) fn from_str(s: &str) -> Self {
        match s {
            "invalid_client" => BasicErrorResponseType::InvalidClient,
            "invalid_grant" => BasicErrorResponseType::InvalidGrant,
            "invalid_request" => BasicErrorResponseType::InvalidRequest,
            "invalid_scope" => BasicErrorResponseType::InvalidScope,
            "unauthorized_client" => BasicErrorResponseType::UnauthorizedClient,
            "unsupported_grant_type" => BasicErrorResponseType::UnsupportedGrantType,
            ext => BasicErrorResponseType::Extension(ext.to_string()),
        }
    }
}
impl AsRef<str> for BasicErrorResponseType {
    fn as_ref(&self) -> &str {
        match *self {
            BasicErrorResponseType::InvalidClient => "invalid_client",
            BasicErrorResponseType::InvalidGrant => "invalid_grant",
            BasicErrorResponseType::InvalidRequest => "invalid_request",
            BasicErrorResponseType::InvalidScope => "invalid_scope",
            BasicErrorResponseType::UnauthorizedClient => "unauthorized_client",
            BasicErrorResponseType::UnsupportedGrantType => "unsupported_grant_type",
            BasicErrorResponseType::Extension(ref ext) => ext.as_str(),
        }
    }
}
impl<'de> serde::Deserialize<'de> for BasicErrorResponseType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        let variant_str = String::deserialize(deserializer)?;
        Ok(Self::from_str(&variant_str))
    }
}
impl serde::ser::Serialize for BasicErrorResponseType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::ser::Serializer,
    {
        serializer.serialize_str(self.as_ref())
    }
}
impl ErrorResponseType for BasicErrorResponseType {}
impl Debug for BasicErrorResponseType {
    fn fmt(&self, f: &mut Formatter) -> Result<(), FormatterError> {
        Display::fmt(self, f)
    }
}

impl Display for BasicErrorResponseType {
    fn fmt(&self, f: &mut Formatter) -> Result<(), FormatterError> {
        write!(f, "{}", self.as_ref())
    }
}

/// Error response specialization for basic OAuth2 implementation.
pub type BasicErrorResponse = StandardErrorResponse<BasicErrorResponseType>;

/// Token error specialization for basic OAuth2 implementation.
pub type BasicRequestTokenError<RE> = RequestTokenError<RE, BasicErrorResponse>;

/// Revocation error response specialization for basic OAuth2 implementation.
pub type BasicRevocationErrorResponse = StandardErrorResponse<RevocationErrorResponseType>;
