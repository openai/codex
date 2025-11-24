use crate::endpoint::{endpoint_request, endpoint_response};
use crate::{
    AccessToken, AsyncHttpClient, AuthType, AuthorizationCode, Client, ClientId, ClientSecret,
    EndpointState, ErrorResponse, HttpRequest, PkceCodeVerifier, RedirectUrl, RefreshToken,
    RequestTokenError, ResourceOwnerPassword, ResourceOwnerUsername, RevocableToken, Scope,
    SyncHttpClient, TokenIntrospectionResponse, TokenUrl,
};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use std::borrow::Cow;
use std::error::Error;
use std::fmt::Debug;
use std::future::Future;
use std::marker::PhantomData;
use std::time::Duration;

#[cfg(test)]
mod tests;

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
    pub(crate) fn exchange_client_credentials_impl<'a>(
        &'a self,
        token_url: &'a TokenUrl,
    ) -> ClientCredentialsTokenRequest<'a, TE, TR> {
        ClientCredentialsTokenRequest {
            auth_type: &self.auth_type,
            client_id: &self.client_id,
            client_secret: self.client_secret.as_ref(),
            extra_params: Vec::new(),
            scopes: Vec::new(),
            token_url,
            _phantom: PhantomData,
        }
    }

    pub(crate) fn exchange_code_impl<'a>(
        &'a self,
        token_url: &'a TokenUrl,
        code: AuthorizationCode,
    ) -> CodeTokenRequest<'a, TE, TR> {
        CodeTokenRequest {
            auth_type: &self.auth_type,
            client_id: &self.client_id,
            client_secret: self.client_secret.as_ref(),
            code,
            extra_params: Vec::new(),
            pkce_verifier: None,
            token_url,
            redirect_url: self.redirect_url.as_ref().map(Cow::Borrowed),
            _phantom: PhantomData,
        }
    }

    pub(crate) fn exchange_password_impl<'a>(
        &'a self,
        token_url: &'a TokenUrl,
        username: &'a ResourceOwnerUsername,
        password: &'a ResourceOwnerPassword,
    ) -> PasswordTokenRequest<'a, TE, TR> {
        PasswordTokenRequest {
            auth_type: &self.auth_type,
            client_id: &self.client_id,
            client_secret: self.client_secret.as_ref(),
            username,
            password,
            extra_params: Vec::new(),
            scopes: Vec::new(),
            token_url,
            _phantom: PhantomData,
        }
    }

    pub(crate) fn exchange_refresh_token_impl<'a>(
        &'a self,
        token_url: &'a TokenUrl,
        refresh_token: &'a RefreshToken,
    ) -> RefreshTokenRequest<'a, TE, TR> {
        RefreshTokenRequest {
            auth_type: &self.auth_type,
            client_id: &self.client_id,
            client_secret: self.client_secret.as_ref(),
            extra_params: Vec::new(),
            refresh_token,
            scopes: Vec::new(),
            token_url,
            _phantom: PhantomData,
        }
    }
}

/// A request to exchange an authorization code for an access token.
///
/// See <https://tools.ietf.org/html/rfc6749#section-4.1.3>.
#[derive(Debug)]
pub struct CodeTokenRequest<'a, TE, TR>
where
    TE: ErrorResponse,
    TR: TokenResponse,
{
    pub(crate) auth_type: &'a AuthType,
    pub(crate) client_id: &'a ClientId,
    pub(crate) client_secret: Option<&'a ClientSecret>,
    pub(crate) code: AuthorizationCode,
    pub(crate) extra_params: Vec<(Cow<'a, str>, Cow<'a, str>)>,
    pub(crate) pkce_verifier: Option<PkceCodeVerifier>,
    pub(crate) token_url: &'a TokenUrl,
    pub(crate) redirect_url: Option<Cow<'a, RedirectUrl>>,
    pub(crate) _phantom: PhantomData<(TE, TR)>,
}
impl<'a, TE, TR> CodeTokenRequest<'a, TE, TR>
where
    TE: ErrorResponse + 'static,
    TR: TokenResponse,
{
    /// Appends an extra param to the token request.
    ///
    /// This method allows extensions to be used without direct support from
    /// this crate. If `name` conflicts with a parameter managed by this crate, the
    /// behavior is undefined. In particular, do not set parameters defined by
    /// [RFC 6749](https://tools.ietf.org/html/rfc6749) or
    /// [RFC 7636](https://tools.ietf.org/html/rfc7636).
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

    /// Completes the [Proof Key for Code Exchange](https://tools.ietf.org/html/rfc7636)
    /// (PKCE) protocol flow.
    ///
    /// This method must be called if [`crate::AuthorizationRequest::set_pkce_challenge`] was used
    /// during the authorization request.
    pub fn set_pkce_verifier(mut self, pkce_verifier: PkceCodeVerifier) -> Self {
        self.pkce_verifier = Some(pkce_verifier);
        self
    }

    /// Overrides the `redirect_url` to the one specified.
    pub fn set_redirect_uri(mut self, redirect_url: Cow<'a, RedirectUrl>) -> Self {
        self.redirect_url = Some(redirect_url);
        self
    }

    fn prepare_request<RE>(self) -> Result<HttpRequest, RequestTokenError<RE, TE>>
    where
        RE: Error + 'static,
    {
        let mut params = vec![
            ("grant_type", "authorization_code"),
            ("code", self.code.secret()),
        ];
        if let Some(ref pkce_verifier) = self.pkce_verifier {
            params.push(("code_verifier", pkce_verifier.secret()));
        }

        endpoint_request(
            self.auth_type,
            self.client_id,
            self.client_secret,
            &self.extra_params,
            self.redirect_url,
            None,
            self.token_url.url(),
            params,
        )
        .map_err(|err| RequestTokenError::Other(format!("failed to prepare request: {err}")))
    }

    /// Synchronously sends the request to the authorization server and awaits a response.
    pub fn request<C>(
        self,
        http_client: &C,
    ) -> Result<TR, RequestTokenError<<C as SyncHttpClient>::Error, TE>>
    where
        C: SyncHttpClient,
    {
        endpoint_response(http_client.call(self.prepare_request()?)?)
    }

    /// Asynchronously sends the request to the authorization server and returns a Future.
    pub fn request_async<'c, C>(
        self,
        http_client: &'c C,
    ) -> impl Future<Output = Result<TR, RequestTokenError<<C as AsyncHttpClient<'c>>::Error, TE>>> + 'c
    where
        Self: 'c,
        C: AsyncHttpClient<'c>,
    {
        Box::pin(async move { endpoint_response(http_client.call(self.prepare_request()?).await?) })
    }
}

/// A request to exchange a refresh token for an access token.
///
/// See <https://tools.ietf.org/html/rfc6749#section-6>.
#[derive(Debug)]
pub struct RefreshTokenRequest<'a, TE, TR>
where
    TE: ErrorResponse,
    TR: TokenResponse,
{
    pub(crate) auth_type: &'a AuthType,
    pub(crate) client_id: &'a ClientId,
    pub(crate) client_secret: Option<&'a ClientSecret>,
    pub(crate) extra_params: Vec<(Cow<'a, str>, Cow<'a, str>)>,
    pub(crate) refresh_token: &'a RefreshToken,
    pub(crate) scopes: Vec<Cow<'a, Scope>>,
    pub(crate) token_url: &'a TokenUrl,
    pub(crate) _phantom: PhantomData<(TE, TR)>,
}
impl<'a, TE, TR> RefreshTokenRequest<'a, TE, TR>
where
    TE: ErrorResponse + 'static,
    TR: TokenResponse,
{
    /// Appends an extra param to the token request.
    ///
    /// This method allows extensions to be used without direct support from
    /// this crate. If `name` conflicts with a parameter managed by this crate, the
    /// behavior is undefined. In particular, do not set parameters defined by
    /// [RFC 6749](https://tools.ietf.org/html/rfc6749) or
    /// [RFC 7636](https://tools.ietf.org/html/rfc7636).
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

    /// Appends a new scope to the token request.
    pub fn add_scope(mut self, scope: Scope) -> Self {
        self.scopes.push(Cow::Owned(scope));
        self
    }

    /// Appends a collection of scopes to the token request.
    pub fn add_scopes<I>(mut self, scopes: I) -> Self
    where
        I: IntoIterator<Item = Scope>,
    {
        self.scopes.extend(scopes.into_iter().map(Cow::Owned));
        self
    }

    /// Synchronously sends the request to the authorization server and awaits a response.
    pub fn request<C>(
        self,
        http_client: &C,
    ) -> Result<TR, RequestTokenError<<C as SyncHttpClient>::Error, TE>>
    where
        C: SyncHttpClient,
    {
        endpoint_response(http_client.call(self.prepare_request()?)?)
    }
    /// Asynchronously sends the request to the authorization server and awaits a response.
    pub fn request_async<'c, C>(
        self,
        http_client: &'c C,
    ) -> impl Future<Output = Result<TR, RequestTokenError<<C as AsyncHttpClient<'c>>::Error, TE>>> + 'c
    where
        Self: 'c,
        C: AsyncHttpClient<'c>,
    {
        Box::pin(async move { endpoint_response(http_client.call(self.prepare_request()?).await?) })
    }

    fn prepare_request<RE>(&self) -> Result<HttpRequest, RequestTokenError<RE, TE>>
    where
        RE: Error + 'static,
    {
        endpoint_request(
            self.auth_type,
            self.client_id,
            self.client_secret,
            &self.extra_params,
            None,
            Some(&self.scopes),
            self.token_url.url(),
            vec![
                ("grant_type", "refresh_token"),
                ("refresh_token", self.refresh_token.secret()),
            ],
        )
        .map_err(|err| RequestTokenError::Other(format!("failed to prepare request: {err}")))
    }
}

/// A request to exchange resource owner credentials for an access token.
///
/// See <https://tools.ietf.org/html/rfc6749#section-4.3>.
#[derive(Debug)]
pub struct PasswordTokenRequest<'a, TE, TR>
where
    TE: ErrorResponse,
    TR: TokenResponse,
{
    pub(crate) auth_type: &'a AuthType,
    pub(crate) client_id: &'a ClientId,
    pub(crate) client_secret: Option<&'a ClientSecret>,
    pub(crate) extra_params: Vec<(Cow<'a, str>, Cow<'a, str>)>,
    pub(crate) username: &'a ResourceOwnerUsername,
    pub(crate) password: &'a ResourceOwnerPassword,
    pub(crate) scopes: Vec<Cow<'a, Scope>>,
    pub(crate) token_url: &'a TokenUrl,
    pub(crate) _phantom: PhantomData<(TE, TR)>,
}
impl<'a, TE, TR> PasswordTokenRequest<'a, TE, TR>
where
    TE: ErrorResponse + 'static,
    TR: TokenResponse,
{
    /// Appends an extra param to the token request.
    ///
    /// This method allows extensions to be used without direct support from
    /// this crate. If `name` conflicts with a parameter managed by this crate, the
    /// behavior is undefined. In particular, do not set parameters defined by
    /// [RFC 6749](https://tools.ietf.org/html/rfc6749) or
    /// [RFC 7636](https://tools.ietf.org/html/rfc7636).
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

    /// Appends a new scope to the token request.
    pub fn add_scope(mut self, scope: Scope) -> Self {
        self.scopes.push(Cow::Owned(scope));
        self
    }

    /// Appends a collection of scopes to the token request.
    pub fn add_scopes<I>(mut self, scopes: I) -> Self
    where
        I: IntoIterator<Item = Scope>,
    {
        self.scopes.extend(scopes.into_iter().map(Cow::Owned));
        self
    }

    /// Synchronously sends the request to the authorization server and awaits a response.
    pub fn request<C>(
        self,
        http_client: &C,
    ) -> Result<TR, RequestTokenError<<C as SyncHttpClient>::Error, TE>>
    where
        C: SyncHttpClient,
    {
        endpoint_response(http_client.call(self.prepare_request()?)?)
    }

    /// Asynchronously sends the request to the authorization server and awaits a response.
    pub fn request_async<'c, C>(
        self,
        http_client: &'c C,
    ) -> impl Future<Output = Result<TR, RequestTokenError<<C as AsyncHttpClient<'c>>::Error, TE>>> + 'c
    where
        Self: 'c,
        C: AsyncHttpClient<'c>,
    {
        Box::pin(async move { endpoint_response(http_client.call(self.prepare_request()?).await?) })
    }

    fn prepare_request<RE>(&self) -> Result<HttpRequest, RequestTokenError<RE, TE>>
    where
        RE: Error + 'static,
    {
        endpoint_request(
            self.auth_type,
            self.client_id,
            self.client_secret,
            &self.extra_params,
            None,
            Some(&self.scopes),
            self.token_url.url(),
            vec![
                ("grant_type", "password"),
                ("username", self.username),
                ("password", self.password.secret()),
            ],
        )
        .map_err(|err| RequestTokenError::Other(format!("failed to prepare request: {err}")))
    }
}

/// A request to exchange client credentials for an access token.
///
/// See <https://tools.ietf.org/html/rfc6749#section-4.4>.
#[derive(Debug)]
pub struct ClientCredentialsTokenRequest<'a, TE, TR>
where
    TE: ErrorResponse,
    TR: TokenResponse,
{
    pub(crate) auth_type: &'a AuthType,
    pub(crate) client_id: &'a ClientId,
    pub(crate) client_secret: Option<&'a ClientSecret>,
    pub(crate) extra_params: Vec<(Cow<'a, str>, Cow<'a, str>)>,
    pub(crate) scopes: Vec<Cow<'a, Scope>>,
    pub(crate) token_url: &'a TokenUrl,
    pub(crate) _phantom: PhantomData<(TE, TR)>,
}
impl<'a, TE, TR> ClientCredentialsTokenRequest<'a, TE, TR>
where
    TE: ErrorResponse + 'static,
    TR: TokenResponse,
{
    /// Appends an extra param to the token request.
    ///
    /// This method allows extensions to be used without direct support from
    /// this crate. If `name` conflicts with a parameter managed by this crate, the
    /// behavior is undefined. In particular, do not set parameters defined by
    /// [RFC 6749](https://tools.ietf.org/html/rfc6749) or
    /// [RFC 7636](https://tools.ietf.org/html/rfc7636).
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

    /// Appends a new scope to the token request.
    pub fn add_scope(mut self, scope: Scope) -> Self {
        self.scopes.push(Cow::Owned(scope));
        self
    }

    /// Appends a collection of scopes to the token request.
    pub fn add_scopes<I>(mut self, scopes: I) -> Self
    where
        I: IntoIterator<Item = Scope>,
    {
        self.scopes.extend(scopes.into_iter().map(Cow::Owned));
        self
    }

    /// Synchronously sends the request to the authorization server and awaits a response.
    pub fn request<C>(
        self,
        http_client: &C,
    ) -> Result<TR, RequestTokenError<<C as SyncHttpClient>::Error, TE>>
    where
        C: SyncHttpClient,
    {
        endpoint_response(http_client.call(self.prepare_request()?)?)
    }

    /// Asynchronously sends the request to the authorization server and awaits a response.
    pub fn request_async<'c, C>(
        self,
        http_client: &'c C,
    ) -> impl Future<Output = Result<TR, RequestTokenError<<C as AsyncHttpClient<'c>>::Error, TE>>> + 'c
    where
        Self: 'c,
        C: AsyncHttpClient<'c>,
    {
        Box::pin(async move { endpoint_response(http_client.call(self.prepare_request()?).await?) })
    }

    fn prepare_request<RE>(&self) -> Result<HttpRequest, RequestTokenError<RE, TE>>
    where
        RE: Error + 'static,
    {
        endpoint_request(
            self.auth_type,
            self.client_id,
            self.client_secret,
            &self.extra_params,
            None,
            Some(&self.scopes),
            self.token_url.url(),
            vec![("grant_type", "client_credentials")],
        )
        .map_err(|err| RequestTokenError::Other(format!("failed to prepare request: {err}")))
    }
}

/// Type of OAuth2 access token.
pub trait TokenType: Clone + DeserializeOwned + Debug + PartialEq + Serialize {}

/// Trait for adding extra fields to the `TokenResponse`.
pub trait ExtraTokenFields: DeserializeOwned + Debug + Serialize {}

/// Empty (default) extra token fields.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct EmptyExtraTokenFields {}
impl ExtraTokenFields for EmptyExtraTokenFields {}

/// Common methods shared by all OAuth2 token implementations.
///
/// The methods in this trait are defined in
/// [Section 5.1 of RFC 6749](https://tools.ietf.org/html/rfc6749#section-5.1). This trait exists
/// separately from the `StandardTokenResponse` struct to support customization by clients,
/// such as supporting interoperability with non-standards-complaint OAuth2 providers.
pub trait TokenResponse: Debug + DeserializeOwned + Serialize {
    /// Type of OAuth2 access token included in this response.
    type TokenType: TokenType;

    /// REQUIRED. The access token issued by the authorization server.
    fn access_token(&self) -> &AccessToken;
    /// REQUIRED. The type of the token issued as described in
    /// [Section 7.1](https://tools.ietf.org/html/rfc6749#section-7.1).
    /// Value is case insensitive and deserialized to the generic `TokenType` parameter.
    fn token_type(&self) -> &Self::TokenType;
    /// RECOMMENDED. The lifetime in seconds of the access token. For example, the value 3600
    /// denotes that the access token will expire in one hour from the time the response was
    /// generated. If omitted, the authorization server SHOULD provide the expiration time via
    /// other means or document the default value.
    fn expires_in(&self) -> Option<Duration>;
    /// OPTIONAL. The refresh token, which can be used to obtain new access tokens using the same
    /// authorization grant as described in
    /// [Section 6](https://tools.ietf.org/html/rfc6749#section-6).
    fn refresh_token(&self) -> Option<&RefreshToken>;
    /// OPTIONAL, if identical to the scope requested by the client; otherwise, REQUIRED. The
    /// scope of the access token as described by
    /// [Section 3.3](https://tools.ietf.org/html/rfc6749#section-3.3). If included in the response,
    /// this space-delimited field is parsed into a `Vec` of individual scopes. If omitted from
    /// the response, this field is `None`.
    fn scopes(&self) -> Option<&Vec<Scope>>;
}

/// Standard OAuth2 token response.
///
/// This struct includes the fields defined in
/// [Section 5.1 of RFC 6749](https://tools.ietf.org/html/rfc6749#section-5.1), as well as
/// extensions defined by the `EF` type parameter.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StandardTokenResponse<EF, TT>
where
    EF: ExtraTokenFields,
    TT: TokenType,
{
    access_token: AccessToken,
    #[serde(bound = "TT: TokenType")]
    #[serde(deserialize_with = "crate::helpers::deserialize_untagged_enum_case_insensitive")]
    token_type: TT,
    #[serde(skip_serializing_if = "Option::is_none")]
    expires_in: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    refresh_token: Option<RefreshToken>,
    #[serde(rename = "scope")]
    #[serde(deserialize_with = "crate::helpers::deserialize_space_delimited_vec")]
    #[serde(serialize_with = "crate::helpers::serialize_space_delimited_vec")]
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    scopes: Option<Vec<Scope>>,

    #[serde(bound = "EF: ExtraTokenFields")]
    #[serde(flatten)]
    extra_fields: EF,
}
impl<EF, TT> StandardTokenResponse<EF, TT>
where
    EF: ExtraTokenFields,
    TT: TokenType,
{
    /// Instantiate a new OAuth2 token response.
    pub fn new(access_token: AccessToken, token_type: TT, extra_fields: EF) -> Self {
        Self {
            access_token,
            token_type,
            expires_in: None,
            refresh_token: None,
            scopes: None,
            extra_fields,
        }
    }

    /// Set the `access_token` field.
    pub fn set_access_token(&mut self, access_token: AccessToken) {
        self.access_token = access_token;
    }

    /// Set the `token_type` field.
    pub fn set_token_type(&mut self, token_type: TT) {
        self.token_type = token_type;
    }

    /// Set the `expires_in` field.
    pub fn set_expires_in(&mut self, expires_in: Option<&Duration>) {
        self.expires_in = expires_in.map(Duration::as_secs);
    }

    /// Set the `refresh_token` field.
    pub fn set_refresh_token(&mut self, refresh_token: Option<RefreshToken>) {
        self.refresh_token = refresh_token;
    }

    /// Set the `scopes` field.
    pub fn set_scopes(&mut self, scopes: Option<Vec<Scope>>) {
        self.scopes = scopes;
    }

    /// Extra fields defined by the client application.
    pub fn extra_fields(&self) -> &EF {
        &self.extra_fields
    }

    /// Set the extra fields defined by the client application.
    pub fn set_extra_fields(&mut self, extra_fields: EF) {
        self.extra_fields = extra_fields;
    }
}
impl<EF, TT> TokenResponse for StandardTokenResponse<EF, TT>
where
    EF: ExtraTokenFields,
    TT: TokenType,
{
    type TokenType = TT;

    /// REQUIRED. The access token issued by the authorization server.
    fn access_token(&self) -> &AccessToken {
        &self.access_token
    }
    /// REQUIRED. The type of the token issued as described in
    /// [Section 7.1](https://tools.ietf.org/html/rfc6749#section-7.1).
    /// Value is case insensitive and deserialized to the generic `TokenType` parameter.
    fn token_type(&self) -> &TT {
        &self.token_type
    }
    /// RECOMMENDED. The lifetime in seconds of the access token. For example, the value 3600
    /// denotes that the access token will expire in one hour from the time the response was
    /// generated. If omitted, the authorization server SHOULD provide the expiration time via
    /// other means or document the default value.
    fn expires_in(&self) -> Option<Duration> {
        self.expires_in.map(Duration::from_secs)
    }
    /// OPTIONAL. The refresh token, which can be used to obtain new access tokens using the same
    /// authorization grant as described in
    /// [Section 6](https://tools.ietf.org/html/rfc6749#section-6).
    fn refresh_token(&self) -> Option<&RefreshToken> {
        self.refresh_token.as_ref()
    }
    /// OPTIONAL, if identical to the scope requested by the client; otherwise, REQUIRED. The
    /// scope of the access token as described by
    /// [Section 3.3](https://tools.ietf.org/html/rfc6749#section-3.3). If included in the response,
    /// this space-delimited field is parsed into a `Vec` of individual scopes. If omitted from
    /// the response, this field is `None`.
    fn scopes(&self) -> Option<&Vec<Scope>> {
        self.scopes.as_ref()
    }
}
