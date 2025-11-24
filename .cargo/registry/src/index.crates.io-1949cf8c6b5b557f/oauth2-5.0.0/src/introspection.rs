use crate::endpoint::{endpoint_request, endpoint_response};
use crate::{
    AccessToken, AsyncHttpClient, AuthType, Client, ClientId, ClientSecret, EndpointState,
    ErrorResponse, ExtraTokenFields, HttpRequest, IntrospectionUrl, RequestTokenError,
    RevocableToken, Scope, SyncHttpClient, TokenResponse, TokenType,
};

use chrono::serde::ts_seconds_option;
use chrono::{DateTime, Utc};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use std::borrow::Cow;
use std::error::Error;
use std::fmt::Debug;
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
    pub(crate) fn introspect_impl<'a>(
        &'a self,
        introspection_url: &'a IntrospectionUrl,
        token: &'a AccessToken,
    ) -> IntrospectionRequest<'a, TE, TIR> {
        IntrospectionRequest {
            auth_type: &self.auth_type,
            client_id: &self.client_id,
            client_secret: self.client_secret.as_ref(),
            extra_params: Vec::new(),
            introspection_url,
            token,
            token_type_hint: None,
            _phantom: PhantomData,
        }
    }
}

/// A request to introspect an access token.
///
/// See <https://tools.ietf.org/html/rfc7662#section-2.1>.
#[derive(Debug)]
pub struct IntrospectionRequest<'a, TE, TIR>
where
    TE: ErrorResponse,
    TIR: TokenIntrospectionResponse,
{
    pub(crate) token: &'a AccessToken,
    pub(crate) token_type_hint: Option<Cow<'a, str>>,
    pub(crate) auth_type: &'a AuthType,
    pub(crate) client_id: &'a ClientId,
    pub(crate) client_secret: Option<&'a ClientSecret>,
    pub(crate) extra_params: Vec<(Cow<'a, str>, Cow<'a, str>)>,
    pub(crate) introspection_url: &'a IntrospectionUrl,
    pub(crate) _phantom: PhantomData<(TE, TIR)>,
}

impl<'a, TE, TIR> IntrospectionRequest<'a, TE, TIR>
where
    TE: ErrorResponse + 'static,
    TIR: TokenIntrospectionResponse,
{
    /// Sets the optional token_type_hint parameter.
    ///
    /// See <https://tools.ietf.org/html/rfc7662#section-2.1>.
    ///
    /// OPTIONAL.  A hint about the type of the token submitted for
    ///       introspection.  The protected resource MAY pass this parameter to
    ///       help the authorization server optimize the token lookup.  If the
    ///       server is unable to locate the token using the given hint, it MUST
    ///      extend its search across all of its supported token types.  An
    ///      authorization server MAY ignore this parameter, particularly if it
    ///      is able to detect the token type automatically.  Values for this
    ///      field are defined in the "OAuth Token Type Hints" registry defined
    ///      in OAuth Token Revocation [RFC7009](https://tools.ietf.org/html/rfc7009).
    pub fn set_token_type_hint<V>(mut self, value: V) -> Self
    where
        V: Into<Cow<'a, str>>,
    {
        self.token_type_hint = Some(value.into());

        self
    }

    /// Appends an extra param to the token introspection request.
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
        if let Some(ref token_type_hint) = self.token_type_hint {
            params.push(("token_type_hint", token_type_hint));
        }

        endpoint_request(
            self.auth_type,
            self.client_id,
            self.client_secret,
            &self.extra_params,
            None,
            None,
            self.introspection_url.url(),
            params,
        )
        .map_err(|err| RequestTokenError::Other(format!("failed to prepare request: {err}")))
    }

    /// Synchronously sends the request to the authorization server and awaits a response.
    pub fn request<C>(
        self,
        http_client: &C,
    ) -> Result<TIR, RequestTokenError<<C as SyncHttpClient>::Error, TE>>
    where
        C: SyncHttpClient,
    {
        endpoint_response(http_client.call(self.prepare_request()?)?)
    }

    /// Asynchronously sends the request to the authorization server and returns a Future.
    pub fn request_async<'c, C>(
        self,
        http_client: &'c C,
    ) -> impl Future<Output = Result<TIR, RequestTokenError<<C as AsyncHttpClient<'c>>::Error, TE>>> + 'c
    where
        Self: 'c,
        C: AsyncHttpClient<'c>,
    {
        Box::pin(async move { endpoint_response(http_client.call(self.prepare_request()?).await?) })
    }
}

/// Common methods shared by all OAuth2 token introspection implementations.
///
/// The methods in this trait are defined in
/// [Section 2.2 of RFC 7662](https://tools.ietf.org/html/rfc7662#section-2.2). This trait exists
/// separately from the `StandardTokenIntrospectionResponse` struct to support customization by
/// clients, such as supporting interoperability with non-standards-complaint OAuth2 providers.
pub trait TokenIntrospectionResponse: Debug + DeserializeOwned + Serialize {
    /// Type of OAuth2 access token included in this response.
    type TokenType: TokenType;

    /// REQUIRED.  Boolean indicator of whether or not the presented token
    /// is currently active.  The specifics of a token's "active" state
    /// will vary depending on the implementation of the authorization
    /// server and the information it keeps about its tokens, but a "true"
    /// value return for the "active" property will generally indicate
    /// that a given token has been issued by this authorization server,
    /// has not been revoked by the resource owner, and is within its
    /// given time window of validity (e.g., after its issuance time and
    /// before its expiration time).
    fn active(&self) -> bool;
    /// OPTIONAL.  A JSON string containing a space-separated list of
    /// scopes associated with this token, in the format described in
    /// [Section 3.3 of RFC 7662](https://tools.ietf.org/html/rfc7662#section-3.3).
    /// If included in the response,
    /// this space-delimited field is parsed into a `Vec` of individual scopes. If omitted from
    /// the response, this field is `None`.
    fn scopes(&self) -> Option<&Vec<Scope>>;
    /// OPTIONAL.  Client identifier for the OAuth 2.0 client that
    /// requested this token.
    fn client_id(&self) -> Option<&ClientId>;
    /// OPTIONAL.  Human-readable identifier for the resource owner who
    /// authorized this token.
    fn username(&self) -> Option<&str>;
    /// OPTIONAL.  Type of the token as defined in
    /// [Section 5.1 of RFC 7662](https://tools.ietf.org/html/rfc7662#section-5.1).
    /// Value is case insensitive and deserialized to the generic `TokenType` parameter.
    fn token_type(&self) -> Option<&Self::TokenType>;
    /// OPTIONAL.  Integer timestamp, measured in the number of seconds
    /// since January 1 1970 UTC, indicating when this token will expire,
    /// as defined in JWT [RFC7519](https://tools.ietf.org/html/rfc7519).
    fn exp(&self) -> Option<DateTime<Utc>>;
    /// OPTIONAL.  Integer timestamp, measured in the number of seconds
    /// since January 1 1970 UTC, indicating when this token was
    /// originally issued, as defined in JWT [RFC7519](https://tools.ietf.org/html/rfc7519).
    fn iat(&self) -> Option<DateTime<Utc>>;
    /// OPTIONAL.  Integer timestamp, measured in the number of seconds
    /// since January 1 1970 UTC, indicating when this token is not to be
    /// used before, as defined in JWT [RFC7519](https://tools.ietf.org/html/rfc7519).
    fn nbf(&self) -> Option<DateTime<Utc>>;
    /// OPTIONAL.  Subject of the token, as defined in JWT [RFC7519](https://tools.ietf.org/html/rfc7519).
    /// Usually a machine-readable identifier of the resource owner who
    /// authorized this token.
    fn sub(&self) -> Option<&str>;
    /// OPTIONAL.  Service-specific string identifier or list of string
    /// identifiers representing the intended audience for this token, as
    /// defined in JWT [RFC7519](https://tools.ietf.org/html/rfc7519).
    fn aud(&self) -> Option<&Vec<String>>;
    /// OPTIONAL.  String representing the issuer of this token, as
    /// defined in JWT [RFC7519](https://tools.ietf.org/html/rfc7519).
    fn iss(&self) -> Option<&str>;
    /// OPTIONAL.  String identifier for the token, as defined in JWT
    /// [RFC7519](https://tools.ietf.org/html/rfc7519).
    fn jti(&self) -> Option<&str>;
}

/// Standard OAuth2 token introspection response.
///
/// This struct includes the fields defined in
/// [Section 2.2 of RFC 7662](https://tools.ietf.org/html/rfc7662#section-2.2), as well as
/// extensions defined by the `EF` type parameter.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StandardTokenIntrospectionResponse<EF, TT>
where
    EF: ExtraTokenFields,
    TT: TokenType + 'static,
{
    active: bool,
    #[serde(rename = "scope")]
    #[serde(deserialize_with = "crate::helpers::deserialize_space_delimited_vec")]
    #[serde(serialize_with = "crate::helpers::serialize_space_delimited_vec")]
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    scopes: Option<Vec<Scope>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    client_id: Option<ClientId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    username: Option<String>,
    #[serde(
        bound = "TT: TokenType",
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::helpers::deserialize_untagged_enum_case_insensitive",
        default = "none_field"
    )]
    token_type: Option<TT>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(with = "ts_seconds_option")]
    #[serde(default)]
    exp: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(with = "ts_seconds_option")]
    #[serde(default)]
    iat: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(with = "ts_seconds_option")]
    #[serde(default)]
    nbf: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sub: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    #[serde(deserialize_with = "crate::helpers::deserialize_optional_string_or_vec_string")]
    aud: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    iss: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    jti: Option<String>,

    #[serde(bound = "EF: ExtraTokenFields")]
    #[serde(flatten)]
    extra_fields: EF,
}

fn none_field<T>() -> Option<T> {
    None
}

impl<EF, TT> StandardTokenIntrospectionResponse<EF, TT>
where
    EF: ExtraTokenFields,
    TT: TokenType,
{
    /// Instantiate a new OAuth2 token introspection response.
    pub fn new(active: bool, extra_fields: EF) -> Self {
        Self {
            active,

            scopes: None,
            client_id: None,
            username: None,
            token_type: None,
            exp: None,
            iat: None,
            nbf: None,
            sub: None,
            aud: None,
            iss: None,
            jti: None,
            extra_fields,
        }
    }

    /// Sets the `set_active` field.
    pub fn set_active(&mut self, active: bool) {
        self.active = active;
    }
    /// Sets the `set_scopes` field.
    pub fn set_scopes(&mut self, scopes: Option<Vec<Scope>>) {
        self.scopes = scopes;
    }
    /// Sets the `set_client_id` field.
    pub fn set_client_id(&mut self, client_id: Option<ClientId>) {
        self.client_id = client_id;
    }
    /// Sets the `set_username` field.
    pub fn set_username(&mut self, username: Option<String>) {
        self.username = username;
    }
    /// Sets the `set_token_type` field.
    pub fn set_token_type(&mut self, token_type: Option<TT>) {
        self.token_type = token_type;
    }
    /// Sets the `set_exp` field.
    pub fn set_exp(&mut self, exp: Option<DateTime<Utc>>) {
        self.exp = exp;
    }
    /// Sets the `set_iat` field.
    pub fn set_iat(&mut self, iat: Option<DateTime<Utc>>) {
        self.iat = iat;
    }
    /// Sets the `set_nbf` field.
    pub fn set_nbf(&mut self, nbf: Option<DateTime<Utc>>) {
        self.nbf = nbf;
    }
    /// Sets the `set_sub` field.
    pub fn set_sub(&mut self, sub: Option<String>) {
        self.sub = sub;
    }
    /// Sets the `set_aud` field.
    pub fn set_aud(&mut self, aud: Option<Vec<String>>) {
        self.aud = aud;
    }
    /// Sets the `set_iss` field.
    pub fn set_iss(&mut self, iss: Option<String>) {
        self.iss = iss;
    }
    /// Sets the `set_jti` field.
    pub fn set_jti(&mut self, jti: Option<String>) {
        self.jti = jti;
    }
    /// Extra fields defined by the client application.
    pub fn extra_fields(&self) -> &EF {
        &self.extra_fields
    }
    /// Sets the `set_extra_fields` field.
    pub fn set_extra_fields(&mut self, extra_fields: EF) {
        self.extra_fields = extra_fields;
    }
}
impl<EF, TT> TokenIntrospectionResponse for StandardTokenIntrospectionResponse<EF, TT>
where
    EF: ExtraTokenFields,
    TT: TokenType,
{
    type TokenType = TT;

    fn active(&self) -> bool {
        self.active
    }

    fn scopes(&self) -> Option<&Vec<Scope>> {
        self.scopes.as_ref()
    }

    fn client_id(&self) -> Option<&ClientId> {
        self.client_id.as_ref()
    }

    fn username(&self) -> Option<&str> {
        self.username.as_deref()
    }

    fn token_type(&self) -> Option<&TT> {
        self.token_type.as_ref()
    }

    fn exp(&self) -> Option<DateTime<Utc>> {
        self.exp
    }

    fn iat(&self) -> Option<DateTime<Utc>> {
        self.iat
    }

    fn nbf(&self) -> Option<DateTime<Utc>> {
        self.nbf
    }

    fn sub(&self) -> Option<&str> {
        self.sub.as_deref()
    }

    fn aud(&self) -> Option<&Vec<String>> {
        self.aud.as_ref()
    }

    fn iss(&self) -> Option<&str> {
        self.iss.as_deref()
    }

    fn jti(&self) -> Option<&str> {
        self.jti.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use crate::basic::BasicTokenType;
    use crate::tests::{mock_http_client, new_client};
    use crate::{AccessToken, AuthType, ClientId, IntrospectionUrl, RedirectUrl, Scope};

    use chrono::DateTime;
    use http::header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE};
    use http::{HeaderValue, Response, StatusCode};

    #[test]
    fn test_token_introspection_successful_with_basic_auth_minimal_response() {
        let client = new_client()
            .set_auth_type(AuthType::BasicAuth)
            .set_redirect_uri(RedirectUrl::new("https://redirect/here".to_string()).unwrap())
            .set_introspection_url(
                IntrospectionUrl::new("https://introspection/url".to_string()).unwrap(),
            );

        let introspection_response = client
            .introspect(&AccessToken::new("access_token_123".to_string()))
            .request(&mock_http_client(
                vec![
                    (ACCEPT, "application/json"),
                    (CONTENT_TYPE, "application/x-www-form-urlencoded"),
                    (AUTHORIZATION, "Basic YWFhOmJiYg=="),
                ],
                "token=access_token_123",
                Some("https://introspection/url".parse().unwrap()),
                Response::builder()
                    .status(StatusCode::OK)
                    .header(
                        CONTENT_TYPE,
                        HeaderValue::from_str("application/json").unwrap(),
                    )
                    .body(
                        "{\
                       \"active\": true\
                       }"
                        .to_string()
                        .into_bytes(),
                    )
                    .unwrap(),
            ))
            .unwrap();

        assert!(introspection_response.active);
        assert_eq!(None, introspection_response.scopes);
        assert_eq!(None, introspection_response.client_id);
        assert_eq!(None, introspection_response.username);
        assert_eq!(None, introspection_response.token_type);
        assert_eq!(None, introspection_response.exp);
        assert_eq!(None, introspection_response.iat);
        assert_eq!(None, introspection_response.nbf);
        assert_eq!(None, introspection_response.sub);
        assert_eq!(None, introspection_response.aud);
        assert_eq!(None, introspection_response.iss);
        assert_eq!(None, introspection_response.jti);
    }

    #[test]
    fn test_token_introspection_successful_with_basic_auth_full_response() {
        let client = new_client()
            .set_auth_type(AuthType::BasicAuth)
            .set_redirect_uri(RedirectUrl::new("https://redirect/here".to_string()).unwrap())
            .set_introspection_url(
                IntrospectionUrl::new("https://introspection/url".to_string()).unwrap(),
            );

        let introspection_response = client
            .introspect(&AccessToken::new("access_token_123".to_string()))
            .set_token_type_hint("access_token")
            .request(&mock_http_client(
                vec![
                    (ACCEPT, "application/json"),
                    (CONTENT_TYPE, "application/x-www-form-urlencoded"),
                    (AUTHORIZATION, "Basic YWFhOmJiYg=="),
                ],
                "token=access_token_123&token_type_hint=access_token",
                Some("https://introspection/url".parse().unwrap()),
                Response::builder()
                    .status(StatusCode::OK)
                    .header(
                        CONTENT_TYPE,
                        HeaderValue::from_str("application/json").unwrap(),
                    )
                    .body(
                        r#"{
                            "active": true,
                            "scope": "email profile",
                            "client_id": "aaa",
                            "username": "demo",
                            "token_type": "bearer",
                            "exp": 1604073517,
                            "iat": 1604073217,
                            "nbf": 1604073317,
                            "sub": "demo",
                            "aud": "demo",
                            "iss": "http://127.0.0.1:8080/auth/realms/test-realm",
                            "jti": "be1b7da2-fc18-47b3-bdf1-7a4f50bcf53f"
                        }"#
                        .to_string()
                        .into_bytes(),
                    )
                    .unwrap(),
            ))
            .unwrap();

        assert!(introspection_response.active);
        assert_eq!(
            Some(vec![
                Scope::new("email".to_string()),
                Scope::new("profile".to_string())
            ]),
            introspection_response.scopes
        );
        assert_eq!(
            Some(ClientId::new("aaa".to_string())),
            introspection_response.client_id
        );
        assert_eq!(Some("demo".to_string()), introspection_response.username);
        assert_eq!(
            Some(BasicTokenType::Bearer),
            introspection_response.token_type
        );
        assert_eq!(
            Some(DateTime::from_timestamp(1604073517, 0).unwrap()),
            introspection_response.exp
        );
        assert_eq!(
            Some(DateTime::from_timestamp(1604073217, 0).unwrap()),
            introspection_response.iat
        );
        assert_eq!(
            Some(DateTime::from_timestamp(1604073317, 0).unwrap()),
            introspection_response.nbf
        );
        assert_eq!(Some("demo".to_string()), introspection_response.sub);
        assert_eq!(Some(vec!["demo".to_string()]), introspection_response.aud);
        assert_eq!(
            Some("http://127.0.0.1:8080/auth/realms/test-realm".to_string()),
            introspection_response.iss
        );
        assert_eq!(
            Some("be1b7da2-fc18-47b3-bdf1-7a4f50bcf53f".to_string()),
            introspection_response.jti
        );
    }
}
