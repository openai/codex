use crate::basic::BasicErrorResponseType;
use crate::endpoint::{endpoint_request, endpoint_response_status_only};
use crate::{
    AccessToken, AsyncHttpClient, AuthType, Client, ClientId, ClientSecret, ConfigurationError,
    EndpointState, ErrorResponse, ErrorResponseType, HttpRequest, RefreshToken, RequestTokenError,
    RevocationUrl, SyncHttpClient, TokenIntrospectionResponse, TokenResponse,
};

use serde::{Deserialize, Serialize};

use std::borrow::Cow;
use std::error::Error;
use std::fmt::Error as FormatterError;
use std::fmt::{Debug, Display, Formatter};
use std::future::Future;
use std::marker::PhantomData;

impl<
        TE,
        TR,
        TIR,
        RT,
        TRE,
        HasAuthUrl,
        HasDeviceAuthUrl,
        HasIntrospectionUrl,
        HasRevocationUrl,
        HasTokenUrl,
    >
    Client<
        TE,
        TR,
        TIR,
        RT,
        TRE,
        HasAuthUrl,
        HasDeviceAuthUrl,
        HasIntrospectionUrl,
        HasRevocationUrl,
        HasTokenUrl,
    >
where
    TE: ErrorResponse + 'static,
    TR: TokenResponse,
    TIR: TokenIntrospectionResponse,
    RT: RevocableToken,
    TRE: ErrorResponse + 'static,
    HasAuthUrl: EndpointState,
    HasDeviceAuthUrl: EndpointState,
    HasIntrospectionUrl: EndpointState,
    HasRevocationUrl: EndpointState,
    HasTokenUrl: EndpointState,
{
    pub(crate) fn revoke_token_impl<'a>(
        &'a self,
        revocation_url: &'a RevocationUrl,
        token: RT,
    ) -> Result<RevocationRequest<'a, RT, TRE>, ConfigurationError> {
        // https://tools.ietf.org/html/rfc7009#section-2 states:
        //   "The client requests the revocation of a particular token by making an
        //    HTTP POST request to the token revocation endpoint URL.  This URL
        //    MUST conform to the rules given in [RFC6749], Section 3.1.  Clients
        //    MUST verify that the URL is an HTTPS URL."
        if revocation_url.url().scheme() != "https" {
            return Err(ConfigurationError::InsecureUrl("revocation"));
        }

        Ok(RevocationRequest {
            auth_type: &self.auth_type,
            client_id: &self.client_id,
            client_secret: self.client_secret.as_ref(),
            extra_params: Vec::new(),
            revocation_url,
            token,
            _phantom: PhantomData,
        })
    }
}

/// A revocable token.
///
/// Implement this trait to indicate support for token revocation per [RFC 7009 OAuth 2.0 Token Revocation](https://tools.ietf.org/html/rfc7009#section-2.2).
pub trait RevocableToken {
    /// The actual token value to be revoked.
    fn secret(&self) -> &str;

    /// Indicates the type of the token being revoked, as defined by [RFC 7009, Section 2.1](https://tools.ietf.org/html/rfc7009#section-2.1).
    ///
    /// Implementations should return `Some(...)` values for token types that the target authorization servers are
    /// expected to know (e.g. because they are registered in the [OAuth Token Type Hints Registry](https://tools.ietf.org/html/rfc7009#section-4.1.2))
    /// so that they can potentially optimize their search for the token to be revoked.
    fn type_hint(&self) -> Option<&str>;
}

/// A token representation usable with authorization servers that support [RFC 7009](https://tools.ietf.org/html/rfc7009) token revocation.
///
/// For use with [`revoke_token()`].
///
/// Automatically reports the correct RFC 7009 [`token_type_hint`](https://tools.ietf.org/html/rfc7009#section-2.1) value corresponding to the token type variant used, i.e.
/// `access_token` for [`AccessToken`] and `secret_token` for [`RefreshToken`].
///
/// # Example
///
/// Per [RFC 7009, Section 2](https://tools.ietf.org/html/rfc7009#section-2) prefer revocation by refresh token which,
/// if issued to the client, must be supported by the server, otherwise fallback to access token (which may or may not
/// be supported by the server).
///
/// ```rust
/// # use http::{Response, StatusCode};
/// # use oauth2::{
/// #     AccessToken, AuthUrl, ClientId, EmptyExtraTokenFields, HttpResponse, RequestTokenError,
/// #     RevocationUrl, StandardRevocableToken, StandardTokenResponse, TokenResponse, TokenUrl,
/// # };
/// # use oauth2::basic::{BasicClient, BasicRequestTokenError, BasicTokenResponse, BasicTokenType};
/// #
/// # fn err_wrapper() -> Result<(), anyhow::Error> {
/// #
/// # let token_response = BasicTokenResponse::new(
/// #   AccessToken::new("access".to_string()),
/// #   BasicTokenType::Bearer,
/// #   EmptyExtraTokenFields {},
/// # );
/// #
/// # #[derive(Debug, thiserror::Error)]
/// # enum FakeError {}
/// #
/// # let http_client = |_| -> Result<HttpResponse, BasicRequestTokenError<FakeError>> {
/// #     Ok(Response::builder()
/// #         .status(StatusCode::OK)
/// #         .body(Vec::new())
/// #         .unwrap())
/// # };
/// #
/// let client = BasicClient::new(ClientId::new("aaa".to_string()))
///     .set_auth_uri(AuthUrl::new("https://example.com/auth".to_string()).unwrap())
///     .set_token_uri(TokenUrl::new("https://example.com/token".to_string()).unwrap())
///     // Be sure to set a revocation URL.
///     .set_revocation_url(RevocationUrl::new("https://revocation/url".to_string()).unwrap());
///
/// // ...
///
/// let token_to_revoke: StandardRevocableToken = match token_response.refresh_token() {
///     Some(token) => token.into(),
///     None => token_response.access_token().into(),
/// };
///
/// client
///     .revoke_token(token_to_revoke)?
///     .request(&http_client)
/// #   .unwrap();
/// # Ok(())
/// # }
/// ```
///
/// [`revoke_token()`]: crate::Client::revoke_token()
#[derive(Clone, Debug, Deserialize, Serialize)]
#[non_exhaustive]
pub enum StandardRevocableToken {
    /// A representation of an [`AccessToken`] suitable for use with [`revoke_token()`](crate::Client::revoke_token()).
    AccessToken(AccessToken),
    /// A representation of an [`RefreshToken`] suitable for use with [`revoke_token()`](crate::Client::revoke_token()).
    RefreshToken(RefreshToken),
}
impl RevocableToken for StandardRevocableToken {
    fn secret(&self) -> &str {
        match self {
            Self::AccessToken(token) => token.secret(),
            Self::RefreshToken(token) => token.secret(),
        }
    }

    /// Indicates the type of the token to be revoked, as defined by [RFC 7009, Section 2.1](https://tools.ietf.org/html/rfc7009#section-2.1), i.e.:
    ///
    /// * `access_token`: An access token as defined in [RFC 6749,
    ///   Section 1.4](https://tools.ietf.org/html/rfc6749#section-1.4)
    ///
    /// * `refresh_token`: A refresh token as defined in [RFC 6749,
    ///   Section 1.5](https://tools.ietf.org/html/rfc6749#section-1.5)
    fn type_hint(&self) -> Option<&str> {
        match self {
            StandardRevocableToken::AccessToken(_) => Some("access_token"),
            StandardRevocableToken::RefreshToken(_) => Some("refresh_token"),
        }
    }
}

impl From<AccessToken> for StandardRevocableToken {
    fn from(token: AccessToken) -> Self {
        Self::AccessToken(token)
    }
}

impl From<&AccessToken> for StandardRevocableToken {
    fn from(token: &AccessToken) -> Self {
        Self::AccessToken(token.clone())
    }
}

impl From<RefreshToken> for StandardRevocableToken {
    fn from(token: RefreshToken) -> Self {
        Self::RefreshToken(token)
    }
}

impl From<&RefreshToken> for StandardRevocableToken {
    fn from(token: &RefreshToken) -> Self {
        Self::RefreshToken(token.clone())
    }
}

/// A request to revoke a token via an [`RFC 7009`](https://tools.ietf.org/html/rfc7009#section-2.1) compatible
/// endpoint.
#[derive(Debug)]
pub struct RevocationRequest<'a, RT, TE>
where
    RT: RevocableToken,
    TE: ErrorResponse,
{
    pub(crate) token: RT,
    pub(crate) auth_type: &'a AuthType,
    pub(crate) client_id: &'a ClientId,
    pub(crate) client_secret: Option<&'a ClientSecret>,
    pub(crate) extra_params: Vec<(Cow<'a, str>, Cow<'a, str>)>,
    pub(crate) revocation_url: &'a RevocationUrl,
    pub(crate) _phantom: PhantomData<(RT, TE)>,
}

impl<'a, RT, TE> RevocationRequest<'a, RT, TE>
where
    RT: RevocableToken,
    TE: ErrorResponse + 'static,
{
    /// Appends an extra param to the token revocation request.
    ///
    /// This method allows extensions to be used without direct support from
    /// this crate. If `name` conflicts with a parameter managed by this crate, the
    /// behavior is undefined. In particular, do not set parameters defined by
    /// [RFC 6749](https://tools.ietf.org/html/rfc6749) or
    /// [RFC 7662](https://tools.ietf.org/html/rfc7662).
    ///
    /// # Security Warning
    ///
    /// Callers should follow the security recommendations for any OAuth2 extensions used with
    /// this function, which are beyond the scope of
    /// [RFC 6749](https://tools.ietf.org/html/rfc6749).
    pub fn add_extra_param<N, V>(mut self, name: N, value: V) -> Self
    where
        N: Into<Cow<'a, str>>,
        V: Into<Cow<'a, str>>,
    {
        self.extra_params.push((name.into(), value.into()));
        self
    }

    fn prepare_request<RE>(self) -> Result<HttpRequest, RequestTokenError<RE, TE>>
    where
        RE: Error + 'static,
    {
        let mut params: Vec<(&str, &str)> = vec![("token", self.token.secret())];
        if let Some(type_hint) = self.token.type_hint() {
            params.push(("token_type_hint", type_hint));
        }

        endpoint_request(
            self.auth_type,
            self.client_id,
            self.client_secret,
            &self.extra_params,
            None,
            None,
            self.revocation_url.url(),
            params,
        )
        .map_err(|err| RequestTokenError::Other(format!("failed to prepare request: {err}")))
    }

    /// Synchronously sends the request to the authorization server and awaits a response.
    ///
    /// A successful response indicates that the server either revoked the token or the token was not known to the
    /// server.
    ///
    /// Error [`UnsupportedTokenType`](RevocationErrorResponseType::UnsupportedTokenType) will be returned if the
    /// type of token type given is not supported by the server.
    pub fn request<C>(
        self,
        http_client: &C,
    ) -> Result<(), RequestTokenError<<C as SyncHttpClient>::Error, TE>>
    where
        C: SyncHttpClient,
    {
        // From https://tools.ietf.org/html/rfc7009#section-2.2:
        //   "The content of the response body is ignored by the client as all
        //    necessary information is conveyed in the response code."
        endpoint_response_status_only(http_client.call(self.prepare_request()?)?)
    }

    /// Asynchronously sends the request to the authorization server and returns a Future.
    pub fn request_async<'c, C>(
        self,
        http_client: &'c C,
    ) -> impl Future<Output = Result<(), RequestTokenError<<C as AsyncHttpClient<'c>>::Error, TE>>> + 'c
    where
        Self: 'c,
        C: AsyncHttpClient<'c>,
    {
        Box::pin(async move {
            endpoint_response_status_only(http_client.call(self.prepare_request()?).await?)
        })
    }
}

/// OAuth 2.0 Token Revocation error response types.
///
/// These error types are defined in
/// [Section 2.2.1 of RFC 7009](https://tools.ietf.org/html/rfc7009#section-2.2.1) and
/// [Section 5.2 of RFC 6749](https://tools.ietf.org/html/rfc8628#section-5.2)
#[derive(Clone, PartialEq, Eq)]
pub enum RevocationErrorResponseType {
    /// The authorization server does not support the revocation of the presented token type.
    UnsupportedTokenType,
    /// The authorization server responded with some other error as defined [RFC 6749](https://tools.ietf.org/html/rfc6749) error.
    Basic(BasicErrorResponseType),
}
impl RevocationErrorResponseType {
    fn from_str(s: &str) -> Self {
        match BasicErrorResponseType::from_str(s) {
            BasicErrorResponseType::Extension(ext) => match ext.as_str() {
                "unsupported_token_type" => RevocationErrorResponseType::UnsupportedTokenType,
                _ => RevocationErrorResponseType::Basic(BasicErrorResponseType::Extension(ext)),
            },
            basic => RevocationErrorResponseType::Basic(basic),
        }
    }
}
impl AsRef<str> for RevocationErrorResponseType {
    fn as_ref(&self) -> &str {
        match self {
            RevocationErrorResponseType::UnsupportedTokenType => "unsupported_token_type",
            RevocationErrorResponseType::Basic(basic) => basic.as_ref(),
        }
    }
}
impl<'de> serde::Deserialize<'de> for RevocationErrorResponseType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        let variant_str = String::deserialize(deserializer)?;
        Ok(Self::from_str(&variant_str))
    }
}
impl serde::ser::Serialize for RevocationErrorResponseType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::ser::Serializer,
    {
        serializer.serialize_str(self.as_ref())
    }
}
impl ErrorResponseType for RevocationErrorResponseType {}
impl Debug for RevocationErrorResponseType {
    fn fmt(&self, f: &mut Formatter) -> Result<(), FormatterError> {
        Display::fmt(self, f)
    }
}

impl Display for RevocationErrorResponseType {
    fn fmt(&self, f: &mut Formatter) -> Result<(), FormatterError> {
        write!(f, "{}", self.as_ref())
    }
}

#[cfg(test)]
mod tests {
    use crate::basic::BasicRevocationErrorResponse;
    use crate::tests::colorful_extension::{ColorfulClient, ColorfulRevocableToken};
    use crate::tests::{mock_http_client, new_client};
    use crate::{
        AccessToken, AuthUrl, ClientId, ClientSecret, RefreshToken, RequestTokenError,
        RevocationErrorResponseType, RevocationUrl, TokenUrl,
    };

    use http::header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE};
    use http::{HeaderValue, Response, StatusCode};

    #[test]
    fn test_token_revocation_with_missing_url() {
        let client = new_client().set_revocation_url_option(None);

        let result = client
            .revoke_token(AccessToken::new("access_token_123".to_string()).into())
            .unwrap_err();

        assert_eq!(result.to_string(), "No revocation endpoint URL specified");
    }

    #[test]
    fn test_token_revocation_with_non_https_url() {
        let client = new_client();

        let result = client
            .set_revocation_url(RevocationUrl::new("http://revocation/url".to_string()).unwrap())
            .revoke_token(AccessToken::new("access_token_123".to_string()).into())
            .unwrap_err();

        assert_eq!(
            result.to_string(),
            "Scheme for revocation endpoint URL must be HTTPS"
        );
    }

    #[test]
    fn test_token_revocation_with_unsupported_token_type() {
        let client = new_client()
            .set_revocation_url(RevocationUrl::new("https://revocation/url".to_string()).unwrap());

        let revocation_response = client
          .revoke_token(AccessToken::new("access_token_123".to_string()).into()).unwrap()
          .request(&mock_http_client(
              vec![
                  (ACCEPT, "application/json"),
                  (CONTENT_TYPE, "application/x-www-form-urlencoded"),
                  (AUTHORIZATION, "Basic YWFhOmJiYg=="),
              ],
              "token=access_token_123&token_type_hint=access_token",
              Some("https://revocation/url".parse().unwrap()),
              Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .header(
                    CONTENT_TYPE,
                    HeaderValue::from_str("application/json").unwrap(),
                )
                .body(
                    "{\
                        \"error\": \"unsupported_token_type\", \"error_description\": \"stuff happened\", \
                        \"error_uri\": \"https://errors\"\
                    }"
                      .to_string()
                      .into_bytes(),
                )
                .unwrap(),
          ));

        assert!(matches!(
            revocation_response,
            Err(RequestTokenError::ServerResponse(
                BasicRevocationErrorResponse {
                    error: RevocationErrorResponseType::UnsupportedTokenType,
                    ..
                }
            ))
        ));
    }

    #[test]
    fn test_token_revocation_with_access_token_and_empty_json_response() {
        let client = new_client()
            .set_revocation_url(RevocationUrl::new("https://revocation/url".to_string()).unwrap());

        client
            .revoke_token(AccessToken::new("access_token_123".to_string()).into())
            .unwrap()
            .request(&mock_http_client(
                vec![
                    (ACCEPT, "application/json"),
                    (CONTENT_TYPE, "application/x-www-form-urlencoded"),
                    (AUTHORIZATION, "Basic YWFhOmJiYg=="),
                ],
                "token=access_token_123&token_type_hint=access_token",
                Some("https://revocation/url".parse().unwrap()),
                Response::builder()
                    .status(StatusCode::OK)
                    .header(
                        CONTENT_TYPE,
                        HeaderValue::from_str("application/json").unwrap(),
                    )
                    .body(b"{}".to_vec())
                    .unwrap(),
            ))
            .unwrap();
    }

    #[test]
    fn test_token_revocation_with_access_token_and_empty_response() {
        let client = new_client()
            .set_revocation_url(RevocationUrl::new("https://revocation/url".to_string()).unwrap());

        client
            .revoke_token(AccessToken::new("access_token_123".to_string()).into())
            .unwrap()
            .request(&mock_http_client(
                vec![
                    (ACCEPT, "application/json"),
                    (CONTENT_TYPE, "application/x-www-form-urlencoded"),
                    (AUTHORIZATION, "Basic YWFhOmJiYg=="),
                ],
                "token=access_token_123&token_type_hint=access_token",
                Some("https://revocation/url".parse().unwrap()),
                Response::builder()
                    .status(StatusCode::OK)
                    .body(vec![])
                    .unwrap(),
            ))
            .unwrap();
    }

    #[test]
    fn test_token_revocation_with_access_token_and_non_json_response() {
        let client = new_client()
            .set_revocation_url(RevocationUrl::new("https://revocation/url".to_string()).unwrap());

        client
            .revoke_token(AccessToken::new("access_token_123".to_string()).into())
            .unwrap()
            .request(&mock_http_client(
                vec![
                    (ACCEPT, "application/json"),
                    (CONTENT_TYPE, "application/x-www-form-urlencoded"),
                    (AUTHORIZATION, "Basic YWFhOmJiYg=="),
                ],
                "token=access_token_123&token_type_hint=access_token",
                Some("https://revocation/url".parse().unwrap()),
                Response::builder()
                    .status(StatusCode::OK)
                    .header(
                        CONTENT_TYPE,
                        HeaderValue::from_str("application/octet-stream").unwrap(),
                    )
                    .body(vec![1, 2, 3])
                    .unwrap(),
            ))
            .unwrap();
    }

    #[test]
    fn test_token_revocation_with_refresh_token() {
        let client = new_client()
            .set_revocation_url(RevocationUrl::new("https://revocation/url".to_string()).unwrap());

        client
            .revoke_token(RefreshToken::new("refresh_token_123".to_string()).into())
            .unwrap()
            .request(&mock_http_client(
                vec![
                    (ACCEPT, "application/json"),
                    (CONTENT_TYPE, "application/x-www-form-urlencoded"),
                    (AUTHORIZATION, "Basic YWFhOmJiYg=="),
                ],
                "token=refresh_token_123&token_type_hint=refresh_token",
                Some("https://revocation/url".parse().unwrap()),
                Response::builder()
                    .status(StatusCode::OK)
                    .header(
                        CONTENT_TYPE,
                        HeaderValue::from_str("application/json").unwrap(),
                    )
                    .body(b"{}".to_vec())
                    .unwrap(),
            ))
            .unwrap();
    }

    #[test]
    fn test_extension_token_revocation_successful() {
        let client = ColorfulClient::new(ClientId::new("aaa".to_string()))
            .set_client_secret(ClientSecret::new("bbb".to_string()))
            .set_auth_uri(AuthUrl::new("https://example.com/auth".to_string()).unwrap())
            .set_token_uri(TokenUrl::new("https://example.com/token".to_string()).unwrap())
            .set_revocation_url(RevocationUrl::new("https://revocation/url".to_string()).unwrap());

        client
            .revoke_token(ColorfulRevocableToken::Red(
                "colorful_token_123".to_string(),
            ))
            .unwrap()
            .request(&mock_http_client(
                vec![
                    (ACCEPT, "application/json"),
                    (CONTENT_TYPE, "application/x-www-form-urlencoded"),
                    (AUTHORIZATION, "Basic YWFhOmJiYg=="),
                ],
                "token=colorful_token_123&token_type_hint=red_token",
                Some("https://revocation/url".parse().unwrap()),
                Response::builder()
                    .status(StatusCode::OK)
                    .header(
                        CONTENT_TYPE,
                        HeaderValue::from_str("application/json").unwrap(),
                    )
                    .body(b"{}".to_vec())
                    .unwrap(),
            ))
            .unwrap();
    }
}
