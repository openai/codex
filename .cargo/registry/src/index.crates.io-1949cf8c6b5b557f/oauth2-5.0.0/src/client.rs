use crate::{
    AccessToken, AuthType, AuthUrl, AuthorizationCode, AuthorizationRequest,
    ClientCredentialsTokenRequest, ClientId, ClientSecret, CodeTokenRequest, ConfigurationError,
    CsrfToken, DeviceAccessTokenRequest, DeviceAuthorizationRequest, DeviceAuthorizationResponse,
    DeviceAuthorizationUrl, ErrorResponse, ExtraDeviceAuthorizationFields, IntrospectionRequest,
    IntrospectionUrl, PasswordTokenRequest, RedirectUrl, RefreshToken, RefreshTokenRequest,
    ResourceOwnerPassword, ResourceOwnerUsername, RevocableToken, RevocationRequest, RevocationUrl,
    TokenIntrospectionResponse, TokenResponse, TokenUrl,
};

use std::marker::PhantomData;

mod private {
    /// Private trait to make `EndpointState` a sealed trait.
    pub trait EndpointStateSealed {}
}

/// [Typestate](https://cliffle.com/blog/rust-typestate/) base trait indicating whether an endpoint
/// has been configured via its corresponding setter.
pub trait EndpointState: private::EndpointStateSealed {}

/// [Typestate](https://cliffle.com/blog/rust-typestate/) indicating that an endpoint has not been
/// set and cannot be used.
#[derive(Clone, Debug)]
pub struct EndpointNotSet;
impl EndpointState for EndpointNotSet {}
impl private::EndpointStateSealed for EndpointNotSet {}

/// [Typestate](https://cliffle.com/blog/rust-typestate/) indicating that an endpoint has been set
/// and is ready to be used.
#[derive(Clone, Debug)]
pub struct EndpointSet;
impl EndpointState for EndpointSet {}
impl private::EndpointStateSealed for EndpointSet {}

/// [Typestate](https://cliffle.com/blog/rust-typestate/) indicating that an endpoint may have been
/// set and can be used via fallible methods.
#[derive(Clone, Debug)]
pub struct EndpointMaybeSet;
impl EndpointState for EndpointMaybeSet {}
impl private::EndpointStateSealed for EndpointMaybeSet {}

/// Stores the configuration for an OAuth2 client.
///
/// This type implements the
/// [Builder Pattern](https://doc.rust-lang.org/1.0.0/style/ownership/builders.html) together with
/// [typestates](https://cliffle.com/blog/rust-typestate/#what-are-typestates) to encode whether
/// certain fields have been set that are prerequisites to certain authentication flows. For
/// example, the authorization endpoint must be set via [`set_auth_uri()`](Client::set_auth_uri)
/// before [`authorize_url()`](Client::authorize_url) can be called. Each endpoint has a
/// corresponding generic type
/// parameter (e.g., `HasAuthUrl`) used to statically enforce these dependencies. These generics
/// are set automatically by the corresponding setter functions, and in most cases user code should
/// not need to deal with them directly.
///
/// In addition to unconditional setters (e.g., [`set_auth_uri()`](Client::set_auth_uri)), each
/// endpoint has a corresponding conditional setter (e.g.,
/// [`set_auth_uri_option()`](Client::set_auth_uri_option)) that sets a
/// conditional typestate ([`EndpointMaybeSet`]). When the conditional typestate is set, endpoints
/// can be used via fallible methods that return [`ConfigurationError::MissingUrl`] if an
/// endpoint has not been set. This is useful in dynamic scenarios such as
/// [OpenID Connect Discovery](https://openid.net/specs/openid-connect-discovery-1_0.html), in which
/// it cannot be determined until runtime whether an endpoint is configured.
///
/// # Error Types
///
/// To enable compile time verification that only the correct and complete set of errors for the `Client` function being
/// invoked are exposed to the caller, the `Client` type is specialized on multiple implementations of the
/// [`ErrorResponse`] trait. The exact [`ErrorResponse`] implementation returned varies by the RFC that the invoked
/// `Client` function implements:
///
///   - Generic type `TE` (aka Token Error) for errors defined by [RFC 6749 OAuth 2.0 Authorization Framework](https://tools.ietf.org/html/rfc6749).
///   - Generic type `TRE` (aka Token Revocation Error) for errors defined by [RFC 7009 OAuth 2.0 Token Revocation](https://tools.ietf.org/html/rfc7009).
///
/// For example when revoking a token, error code `unsupported_token_type` (from RFC 7009) may be returned:
/// ```rust
/// # use thiserror::Error;
/// # use http::status::StatusCode;
/// # use http::header::{HeaderValue, CONTENT_TYPE};
/// # use http::Response;
/// # use oauth2::{*, basic::*};
/// #
/// # let client = BasicClient::new(ClientId::new("aaa".to_string()))
/// #     .set_client_secret(ClientSecret::new("bbb".to_string()))
/// #     .set_auth_uri(AuthUrl::new("https://example.com/auth".to_string()).unwrap())
/// #     .set_token_uri(TokenUrl::new("https://example.com/token".to_string()).unwrap())
/// #     .set_revocation_url(RevocationUrl::new("https://revocation/url".to_string()).unwrap());
/// #
/// # #[derive(Debug, Error)]
/// # enum FakeError {
/// #     #[error("error")]
/// #     Err,
/// # }
/// #
/// # let http_client = |_| -> Result<HttpResponse, FakeError> {
/// #     Ok(Response::builder()
/// #         .status(StatusCode::BAD_REQUEST)
/// #         .header(CONTENT_TYPE, HeaderValue::from_str("application/json").unwrap())
/// #         .body(
/// #             r#"{"error": "unsupported_token_type",
/// #                 "error_description": "stuff happened",
/// #                 "error_uri": "https://errors"}"#
/// #             .to_string()
/// #             .into_bytes(),
/// #         )
/// #         .unwrap())
/// # };
/// #
/// let res = client
///     .revoke_token(AccessToken::new("some token".to_string()).into())
///     .unwrap()
///     .request(&http_client);
///
/// assert!(matches!(res, Err(
///     RequestTokenError::ServerResponse(err)) if matches!(err.error(),
///         RevocationErrorResponseType::UnsupportedTokenType)));
/// ```
///
/// # Examples
///
/// See the [crate] root documentation for usage examples.
#[derive(Clone, Debug)]
pub struct Client<
    TE,
    TR,
    TIR,
    RT,
    TRE,
    HasAuthUrl = EndpointNotSet,
    HasDeviceAuthUrl = EndpointNotSet,
    HasIntrospectionUrl = EndpointNotSet,
    HasRevocationUrl = EndpointNotSet,
    HasTokenUrl = EndpointNotSet,
> where
    TE: ErrorResponse,
    TR: TokenResponse,
    TIR: TokenIntrospectionResponse,
    RT: RevocableToken,
    TRE: ErrorResponse,
    HasAuthUrl: EndpointState,
    HasDeviceAuthUrl: EndpointState,
    HasIntrospectionUrl: EndpointState,
    HasRevocationUrl: EndpointState,
    HasTokenUrl: EndpointState,
{
    pub(crate) client_id: ClientId,
    pub(crate) client_secret: Option<ClientSecret>,
    pub(crate) auth_url: Option<AuthUrl>,
    pub(crate) auth_type: AuthType,
    pub(crate) token_url: Option<TokenUrl>,
    pub(crate) redirect_url: Option<RedirectUrl>,
    pub(crate) introspection_url: Option<IntrospectionUrl>,
    pub(crate) revocation_url: Option<RevocationUrl>,
    pub(crate) device_authorization_url: Option<DeviceAuthorizationUrl>,
    #[allow(clippy::type_complexity)]
    pub(crate) phantom: PhantomData<(
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
    )>,
}
impl<TE, TR, TIR, RT, TRE>
    Client<
        TE,
        TR,
        TIR,
        RT,
        TRE,
        EndpointNotSet,
        EndpointNotSet,
        EndpointNotSet,
        EndpointNotSet,
        EndpointNotSet,
    >
where
    TE: ErrorResponse + 'static,
    TR: TokenResponse,
    TIR: TokenIntrospectionResponse,
    RT: RevocableToken,
    TRE: ErrorResponse + 'static,
{
    /// Initializes an OAuth2 client with the specified client ID.
    pub fn new(client_id: ClientId) -> Self {
        Self {
            client_id,
            client_secret: None,
            auth_url: None,
            auth_type: AuthType::BasicAuth,
            token_url: None,
            redirect_url: None,
            introspection_url: None,
            revocation_url: None,
            device_authorization_url: None,
            phantom: PhantomData,
        }
    }
}
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
    /// Set the type of client authentication used for communicating with the authorization
    /// server.
    ///
    /// The default is to use HTTP Basic authentication, as recommended in
    /// [Section 2.3.1 of RFC 6749](https://tools.ietf.org/html/rfc6749#section-2.3.1). Note that
    /// if a client secret is omitted (i.e., [`set_client_secret()`](Self::set_client_secret) is not
    /// called), [`AuthType::RequestBody`] is used regardless of the `auth_type` passed to
    /// this function.
    pub fn set_auth_type(mut self, auth_type: AuthType) -> Self {
        self.auth_type = auth_type;

        self
    }

    /// Set the authorization endpoint.
    ///
    /// The client uses the authorization endpoint to obtain authorization from the resource owner
    /// via user-agent redirection. This URL is used in all standard OAuth2 flows except the
    /// [Resource Owner Password Credentials Grant](https://tools.ietf.org/html/rfc6749#section-4.3)
    /// and the [Client Credentials Grant](https://tools.ietf.org/html/rfc6749#section-4.4).
    pub fn set_auth_uri(
        self,
        auth_url: AuthUrl,
    ) -> Client<
        TE,
        TR,
        TIR,
        RT,
        TRE,
        EndpointSet,
        HasDeviceAuthUrl,
        HasIntrospectionUrl,
        HasRevocationUrl,
        HasTokenUrl,
    > {
        Client {
            client_id: self.client_id,
            client_secret: self.client_secret,
            auth_url: Some(auth_url),
            auth_type: self.auth_type,
            token_url: self.token_url,
            redirect_url: self.redirect_url,
            introspection_url: self.introspection_url,
            revocation_url: self.revocation_url,
            device_authorization_url: self.device_authorization_url,
            phantom: PhantomData,
        }
    }

    /// Conditionally set the authorization endpoint.
    ///
    /// The client uses the authorization endpoint to obtain authorization from the resource owner
    /// via user-agent redirection. This URL is used in all standard OAuth2 flows except the
    /// [Resource Owner Password Credentials Grant](https://tools.ietf.org/html/rfc6749#section-4.3)
    /// and the [Client Credentials Grant](https://tools.ietf.org/html/rfc6749#section-4.4).
    pub fn set_auth_uri_option(
        self,
        auth_url: Option<AuthUrl>,
    ) -> Client<
        TE,
        TR,
        TIR,
        RT,
        TRE,
        EndpointMaybeSet,
        HasDeviceAuthUrl,
        HasIntrospectionUrl,
        HasRevocationUrl,
        HasTokenUrl,
    > {
        Client {
            client_id: self.client_id,
            client_secret: self.client_secret,
            auth_url,
            auth_type: self.auth_type,
            token_url: self.token_url,
            redirect_url: self.redirect_url,
            introspection_url: self.introspection_url,
            revocation_url: self.revocation_url,
            device_authorization_url: self.device_authorization_url,
            phantom: PhantomData,
        }
    }

    /// Set the client secret.
    ///
    /// A client secret is generally used for confidential (i.e., server-side) OAuth2 clients and
    /// omitted from public (browser or native app) OAuth2 clients (see
    /// [RFC 8252](https://tools.ietf.org/html/rfc8252)).
    pub fn set_client_secret(mut self, client_secret: ClientSecret) -> Self {
        self.client_secret = Some(client_secret);

        self
    }

    /// Set the [RFC 8628](https://tools.ietf.org/html/rfc8628) device authorization endpoint used
    /// for the Device Authorization Flow.
    ///
    /// See [`exchange_device_code()`](Self::exchange_device_code).
    pub fn set_device_authorization_url(
        self,
        device_authorization_url: DeviceAuthorizationUrl,
    ) -> Client<
        TE,
        TR,
        TIR,
        RT,
        TRE,
        HasAuthUrl,
        EndpointSet,
        HasIntrospectionUrl,
        HasRevocationUrl,
        HasTokenUrl,
    > {
        Client {
            client_id: self.client_id,
            client_secret: self.client_secret,
            auth_url: self.auth_url,
            auth_type: self.auth_type,
            token_url: self.token_url,
            redirect_url: self.redirect_url,
            introspection_url: self.introspection_url,
            revocation_url: self.revocation_url,
            device_authorization_url: Some(device_authorization_url),
            phantom: PhantomData,
        }
    }

    /// Conditionally set the [RFC 8628](https://tools.ietf.org/html/rfc8628) device authorization
    /// endpoint used for the Device Authorization Flow.
    ///
    /// See [`exchange_device_code()`](Self::exchange_device_code).
    pub fn set_device_authorization_url_option(
        self,
        device_authorization_url: Option<DeviceAuthorizationUrl>,
    ) -> Client<
        TE,
        TR,
        TIR,
        RT,
        TRE,
        HasAuthUrl,
        EndpointMaybeSet,
        HasIntrospectionUrl,
        HasRevocationUrl,
        HasTokenUrl,
    > {
        Client {
            client_id: self.client_id,
            client_secret: self.client_secret,
            auth_url: self.auth_url,
            auth_type: self.auth_type,
            token_url: self.token_url,
            redirect_url: self.redirect_url,
            introspection_url: self.introspection_url,
            revocation_url: self.revocation_url,
            device_authorization_url,
            phantom: PhantomData,
        }
    }

    /// Set the [RFC 7662](https://tools.ietf.org/html/rfc7662) introspection endpoint.
    ///
    /// See [`introspect()`](Self::introspect).
    pub fn set_introspection_url(
        self,
        introspection_url: IntrospectionUrl,
    ) -> Client<
        TE,
        TR,
        TIR,
        RT,
        TRE,
        HasAuthUrl,
        HasDeviceAuthUrl,
        EndpointSet,
        HasRevocationUrl,
        HasTokenUrl,
    > {
        Client {
            client_id: self.client_id,
            client_secret: self.client_secret,
            auth_url: self.auth_url,
            auth_type: self.auth_type,
            token_url: self.token_url,
            redirect_url: self.redirect_url,
            introspection_url: Some(introspection_url),
            revocation_url: self.revocation_url,
            device_authorization_url: self.device_authorization_url,
            phantom: PhantomData,
        }
    }

    /// Conditionally set the [RFC 7662](https://tools.ietf.org/html/rfc7662) introspection
    /// endpoint.
    ///
    /// See [`introspect()`](Self::introspect).
    pub fn set_introspection_url_option(
        self,
        introspection_url: Option<IntrospectionUrl>,
    ) -> Client<
        TE,
        TR,
        TIR,
        RT,
        TRE,
        HasAuthUrl,
        HasDeviceAuthUrl,
        EndpointMaybeSet,
        HasRevocationUrl,
        HasTokenUrl,
    > {
        Client {
            client_id: self.client_id,
            client_secret: self.client_secret,
            auth_url: self.auth_url,
            auth_type: self.auth_type,
            token_url: self.token_url,
            redirect_url: self.redirect_url,
            introspection_url,
            revocation_url: self.revocation_url,
            device_authorization_url: self.device_authorization_url,
            phantom: PhantomData,
        }
    }

    /// Set the redirect URL used by the authorization endpoint.
    pub fn set_redirect_uri(mut self, redirect_url: RedirectUrl) -> Self {
        self.redirect_url = Some(redirect_url);

        self
    }

    /// Set the [RFC 7009](https://tools.ietf.org/html/rfc7009) revocation endpoint.
    ///
    /// See [`revoke_token()`](Self::revoke_token()).
    pub fn set_revocation_url(
        self,
        revocation_url: RevocationUrl,
    ) -> Client<
        TE,
        TR,
        TIR,
        RT,
        TRE,
        HasAuthUrl,
        HasDeviceAuthUrl,
        HasIntrospectionUrl,
        EndpointSet,
        HasTokenUrl,
    > {
        Client {
            client_id: self.client_id,
            client_secret: self.client_secret,
            auth_url: self.auth_url,
            auth_type: self.auth_type,
            token_url: self.token_url,
            redirect_url: self.redirect_url,
            introspection_url: self.introspection_url,
            revocation_url: Some(revocation_url),
            device_authorization_url: self.device_authorization_url,
            phantom: PhantomData,
        }
    }

    /// Conditionally set the [RFC 7009](https://tools.ietf.org/html/rfc7009) revocation
    /// endpoint.
    ///
    /// See [`revoke_token()`](Self::revoke_token()).
    pub fn set_revocation_url_option(
        self,
        revocation_url: Option<RevocationUrl>,
    ) -> Client<
        TE,
        TR,
        TIR,
        RT,
        TRE,
        HasAuthUrl,
        HasDeviceAuthUrl,
        HasIntrospectionUrl,
        EndpointMaybeSet,
        HasTokenUrl,
    > {
        Client {
            client_id: self.client_id,
            client_secret: self.client_secret,
            auth_url: self.auth_url,
            auth_type: self.auth_type,
            token_url: self.token_url,
            redirect_url: self.redirect_url,
            introspection_url: self.introspection_url,
            revocation_url,
            device_authorization_url: self.device_authorization_url,
            phantom: PhantomData,
        }
    }

    /// Set the token endpoint.
    ///
    /// The client uses the token endpoint to exchange an authorization code for an access token,
    /// typically with client authentication. This URL is used in
    /// all standard OAuth2 flows except the
    /// [Implicit Grant](https://tools.ietf.org/html/rfc6749#section-4.2).
    pub fn set_token_uri(
        self,
        token_url: TokenUrl,
    ) -> Client<
        TE,
        TR,
        TIR,
        RT,
        TRE,
        HasAuthUrl,
        HasDeviceAuthUrl,
        HasIntrospectionUrl,
        HasRevocationUrl,
        EndpointSet,
    > {
        Client {
            client_id: self.client_id,
            client_secret: self.client_secret,
            auth_url: self.auth_url,
            auth_type: self.auth_type,
            token_url: Some(token_url),
            redirect_url: self.redirect_url,
            introspection_url: self.introspection_url,
            revocation_url: self.revocation_url,
            device_authorization_url: self.device_authorization_url,
            phantom: PhantomData,
        }
    }

    /// Conditionally set the token endpoint.
    ///
    /// The client uses the token endpoint to exchange an authorization code for an access token,
    /// typically with client authentication. This URL is used in
    /// all standard OAuth2 flows except the
    /// [Implicit Grant](https://tools.ietf.org/html/rfc6749#section-4.2).
    pub fn set_token_uri_option(
        self,
        token_url: Option<TokenUrl>,
    ) -> Client<
        TE,
        TR,
        TIR,
        RT,
        TRE,
        HasAuthUrl,
        HasDeviceAuthUrl,
        HasIntrospectionUrl,
        HasRevocationUrl,
        EndpointMaybeSet,
    > {
        Client {
            client_id: self.client_id,
            client_secret: self.client_secret,
            auth_url: self.auth_url,
            auth_type: self.auth_type,
            token_url,
            redirect_url: self.redirect_url,
            introspection_url: self.introspection_url,
            revocation_url: self.revocation_url,
            device_authorization_url: self.device_authorization_url,
            phantom: PhantomData,
        }
    }

    /// Return the Client ID.
    pub fn client_id(&self) -> &ClientId {
        &self.client_id
    }

    /// Return the type of client authentication used for communicating with the authorization
    /// server.
    pub fn auth_type(&self) -> &AuthType {
        &self.auth_type
    }

    /// Return the redirect URL used by the authorization endpoint.
    pub fn redirect_uri(&self) -> Option<&RedirectUrl> {
        self.redirect_url.as_ref()
    }
}

/// Methods requiring an authorization endpoint.
impl<
        TE,
        TR,
        TIR,
        RT,
        TRE,
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
        EndpointSet,
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
    HasDeviceAuthUrl: EndpointState,
    HasIntrospectionUrl: EndpointState,
    HasRevocationUrl: EndpointState,
    HasTokenUrl: EndpointState,
{
    /// Return the authorization endpoint.
    pub fn auth_uri(&self) -> &AuthUrl {
        // This is enforced statically via the HasAuthUrl generic type.
        self.auth_url.as_ref().expect("should have auth_url")
    }

    /// Generate an authorization URL for a new authorization request.
    ///
    /// Requires [`set_auth_uri()`](Self::set_auth_uri) to have been previously
    /// called to set the authorization endpoint.
    ///
    /// # Arguments
    ///
    /// * `state_fn` - A function that returns an opaque value used by the client to maintain state
    ///   between the request and callback. The authorization server includes this value when
    ///   redirecting the user-agent back to the client.
    ///
    /// # Security Warning
    ///
    /// Callers should use a fresh, unpredictable `state` for each authorization request and verify
    /// that this value matches the `state` parameter passed by the authorization server to the
    /// redirect URI. Doing so mitigates
    /// [Cross-Site Request Forgery](https://tools.ietf.org/html/rfc6749#section-10.12)
    ///  attacks. To disable CSRF protections (NOT recommended), use `insecure::authorize_url`
    ///  instead.
    pub fn authorize_url<S>(&self, state_fn: S) -> AuthorizationRequest
    where
        S: FnOnce() -> CsrfToken,
    {
        self.authorize_url_impl(self.auth_uri(), state_fn)
    }
}

/// Methods with a possibly-set authorization endpoint.
impl<
        TE,
        TR,
        TIR,
        RT,
        TRE,
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
        EndpointMaybeSet,
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
    HasDeviceAuthUrl: EndpointState,
    HasIntrospectionUrl: EndpointState,
    HasRevocationUrl: EndpointState,
    HasTokenUrl: EndpointState,
{
    /// Return the authorization endpoint.
    pub fn auth_uri(&self) -> Option<&AuthUrl> {
        self.auth_url.as_ref()
    }

    /// Generate an authorization URL for a new authorization request.
    ///
    /// Requires [`set_auth_uri_option()`](Self::set_auth_uri_option) to have been previously
    /// called to set the authorization endpoint.
    ///
    /// # Arguments
    ///
    /// * `state_fn` - A function that returns an opaque value used by the client to maintain state
    ///   between the request and callback. The authorization server includes this value when
    ///   redirecting the user-agent back to the client.
    ///
    /// # Security Warning
    ///
    /// Callers should use a fresh, unpredictable `state` for each authorization request and verify
    /// that this value matches the `state` parameter passed by the authorization server to the
    /// redirect URI. Doing so mitigates
    /// [Cross-Site Request Forgery](https://tools.ietf.org/html/rfc6749#section-10.12)
    ///  attacks. To disable CSRF protections (NOT recommended), use `insecure::authorize_url`
    ///  instead.
    pub fn authorize_url<S>(&self, state_fn: S) -> Result<AuthorizationRequest, ConfigurationError>
    where
        S: FnOnce() -> CsrfToken,
    {
        Ok(self.authorize_url_impl(
            self.auth_uri()
                .ok_or(ConfigurationError::MissingUrl("authorization"))?,
            state_fn,
        ))
    }
}

/// Methods requiring a token endpoint.
impl<TE, TR, TIR, RT, TRE, HasAuthUrl, HasDeviceAuthUrl, HasIntrospectionUrl, HasRevocationUrl>
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
        EndpointSet,
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
{
    /// Request an access token using the
    /// [Client Credentials Flow](https://datatracker.ietf.org/doc/html/rfc6749#section-4.4).
    ///
    /// Requires [`set_token_uri()`](Self::set_token_uri) to have been previously
    /// called to set the token endpoint.
    pub fn exchange_client_credentials(&self) -> ClientCredentialsTokenRequest<TE, TR> {
        self.exchange_client_credentials_impl(self.token_uri())
    }

    /// Exchange a code returned during the
    /// [Authorization Code Flow](https://datatracker.ietf.org/doc/html/rfc6749#section-4.1)
    /// for an access token.
    ///
    /// Acquires ownership of the `code` because authorization codes may only be used once to
    /// retrieve an access token from the authorization server.
    ///
    /// Requires [`set_token_uri()`](Self::set_token_uri) to have been previously
    /// called to set the token endpoint.
    pub fn exchange_code(&self, code: AuthorizationCode) -> CodeTokenRequest<TE, TR> {
        self.exchange_code_impl(self.token_uri(), code)
    }

    /// Exchange an [RFC 8628](https://tools.ietf.org/html/rfc8628#section-3.2) Device Authorization
    /// Response returned by [`exchange_device_code()`](Self::exchange_device_code) for an access
    /// token.
    ///
    /// Requires [`set_token_uri()`](Self::set_token_uri) to have been previously
    /// called to set the token endpoint.
    pub fn exchange_device_access_token<'a, EF>(
        &'a self,
        auth_response: &'a DeviceAuthorizationResponse<EF>,
    ) -> DeviceAccessTokenRequest<'a, 'static, TR, EF>
    where
        EF: ExtraDeviceAuthorizationFields,
    {
        self.exchange_device_access_token_impl(self.token_uri(), auth_response)
    }

    /// Request an access token using the
    /// [Resource Owner Password Credentials Flow](https://datatracker.ietf.org/doc/html/rfc6749#section-4.3).
    ///
    /// Requires
    /// [`set_token_uri()`](Self::set_token_uri) to have
    /// been previously called to set the token endpoint.
    pub fn exchange_password<'a>(
        &'a self,
        username: &'a ResourceOwnerUsername,
        password: &'a ResourceOwnerPassword,
    ) -> PasswordTokenRequest<'a, TE, TR> {
        self.exchange_password_impl(self.token_uri(), username, password)
    }

    /// Exchange a refresh token for an access token.
    ///
    /// See <https://tools.ietf.org/html/rfc6749#section-6>.
    ///
    /// Requires
    /// [`set_token_uri()`](Self::set_token_uri) to have
    /// been previously called to set the token endpoint.
    pub fn exchange_refresh_token<'a>(
        &'a self,
        refresh_token: &'a RefreshToken,
    ) -> RefreshTokenRequest<'a, TE, TR> {
        self.exchange_refresh_token_impl(self.token_uri(), refresh_token)
    }

    /// Return the token endpoint.
    pub fn token_uri(&self) -> &TokenUrl {
        // This is enforced statically via the HasTokenUrl generic type.
        self.token_url.as_ref().expect("should have token_url")
    }
}

/// Methods with a possibly-set token endpoint.
impl<TE, TR, TIR, RT, TRE, HasAuthUrl, HasDeviceAuthUrl, HasIntrospectionUrl, HasRevocationUrl>
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
        EndpointMaybeSet,
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
{
    /// Request an access token using the
    /// [Client Credentials Flow](https://datatracker.ietf.org/doc/html/rfc6749#section-4.4).
    ///
    /// Requires [`set_token_uri_option()`](Self::set_token_uri_option) to have been previously
    /// called to set the token endpoint.
    pub fn exchange_client_credentials(
        &self,
    ) -> Result<ClientCredentialsTokenRequest<TE, TR>, ConfigurationError> {
        Ok(self.exchange_client_credentials_impl(
            self.token_url
                .as_ref()
                .ok_or(ConfigurationError::MissingUrl("token"))?,
        ))
    }

    /// Exchange a code returned during the
    /// [Authorization Code Flow](https://datatracker.ietf.org/doc/html/rfc6749#section-4.1)
    /// for an access token.
    ///
    /// Acquires ownership of the `code` because authorization codes may only be used once to
    /// retrieve an access token from the authorization server.
    ///
    /// Requires [`set_token_uri_option()`](Self::set_token_uri_option) to have been previously
    /// called to set the token endpoint.
    pub fn exchange_code(
        &self,
        code: AuthorizationCode,
    ) -> Result<CodeTokenRequest<TE, TR>, ConfigurationError> {
        Ok(self.exchange_code_impl(
            self.token_url
                .as_ref()
                .ok_or(ConfigurationError::MissingUrl("token"))?,
            code,
        ))
    }

    /// Exchange an [RFC 8628](https://tools.ietf.org/html/rfc8628#section-3.2) Device Authorization
    /// Response returned by [`exchange_device_code()`](Self::exchange_device_code) for an access
    /// token.
    ///
    /// Requires [`set_token_uri_option()`](Self::set_token_uri_option) to have been previously
    /// called to set the token endpoint.
    pub fn exchange_device_access_token<'a, EF>(
        &'a self,
        auth_response: &'a DeviceAuthorizationResponse<EF>,
    ) -> Result<DeviceAccessTokenRequest<'a, 'static, TR, EF>, ConfigurationError>
    where
        EF: ExtraDeviceAuthorizationFields,
    {
        Ok(self.exchange_device_access_token_impl(
            self.token_url
                .as_ref()
                .ok_or(ConfigurationError::MissingUrl("token"))?,
            auth_response,
        ))
    }

    /// Request an access token using the
    /// [Resource Owner Password Credentials Flow](https://datatracker.ietf.org/doc/html/rfc6749#section-4.3).
    ///
    /// Requires
    /// [`set_token_uri_option()`](Self::set_token_uri_option) to have
    /// been previously called to set the token endpoint.
    pub fn exchange_password<'a>(
        &'a self,
        username: &'a ResourceOwnerUsername,
        password: &'a ResourceOwnerPassword,
    ) -> Result<PasswordTokenRequest<'a, TE, TR>, ConfigurationError> {
        Ok(self.exchange_password_impl(
            self.token_url
                .as_ref()
                .ok_or(ConfigurationError::MissingUrl("token"))?,
            username,
            password,
        ))
    }

    /// Exchange a refresh token for an access token.
    ///
    /// See <https://tools.ietf.org/html/rfc6749#section-6>.
    ///
    /// Requires
    /// [`set_token_uri_option()`](Self::set_token_uri_option) to have
    /// been previously called to set the token endpoint.
    pub fn exchange_refresh_token<'a>(
        &'a self,
        refresh_token: &'a RefreshToken,
    ) -> Result<RefreshTokenRequest<'a, TE, TR>, ConfigurationError> {
        Ok(self.exchange_refresh_token_impl(
            self.token_url
                .as_ref()
                .ok_or(ConfigurationError::MissingUrl("token"))?,
            refresh_token,
        ))
    }

    /// Return the token endpoint.
    pub fn token_uri(&self) -> Option<&TokenUrl> {
        self.token_url.as_ref()
    }
}

/// Methods requiring a device authorization endpoint.
impl<TE, TR, TIR, RT, TRE, HasAuthUrl, HasIntrospectionUrl, HasRevocationUrl, HasTokenUrl>
    Client<
        TE,
        TR,
        TIR,
        RT,
        TRE,
        HasAuthUrl,
        EndpointSet,
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
    HasIntrospectionUrl: EndpointState,
    HasRevocationUrl: EndpointState,
    HasTokenUrl: EndpointState,
{
    /// Begin the [RFC 8628](https://tools.ietf.org/html/rfc8628) Device Authorization Flow and
    /// retrieve a Device Authorization Response.
    ///
    /// Requires
    /// [`set_device_authorization_url()`](Self::set_device_authorization_url) to have
    /// been previously called to set the device authorization endpoint.
    ///
    /// See [`exchange_device_access_token()`](Self::exchange_device_access_token).
    pub fn exchange_device_code(&self) -> DeviceAuthorizationRequest<TE> {
        self.exchange_device_code_impl(self.device_authorization_url())
    }

    /// Return the [RFC 8628](https://tools.ietf.org/html/rfc8628) device authorization endpoint
    /// used for the Device Authorization Flow.
    ///
    /// See [`exchange_device_code()`](Self::exchange_device_code).
    pub fn device_authorization_url(&self) -> &DeviceAuthorizationUrl {
        // This is enforced statically via the HasDeviceAuthUrl generic type.
        self.device_authorization_url
            .as_ref()
            .expect("should have device_authorization_url")
    }
}

/// Methods with a possibly-set device authorization endpoint.
impl<TE, TR, TIR, RT, TRE, HasAuthUrl, HasIntrospectionUrl, HasRevocationUrl, HasTokenUrl>
    Client<
        TE,
        TR,
        TIR,
        RT,
        TRE,
        HasAuthUrl,
        EndpointMaybeSet,
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
    HasIntrospectionUrl: EndpointState,
    HasRevocationUrl: EndpointState,
    HasTokenUrl: EndpointState,
{
    /// Begin the [RFC 8628](https://tools.ietf.org/html/rfc8628) Device Authorization Flow.
    ///
    /// Requires
    /// [`set_device_authorization_url_option()`](Self::set_device_authorization_url_option) to have
    /// been previously called to set the device authorization endpoint.
    ///
    /// See [`exchange_device_access_token()`](Self::exchange_device_access_token).
    pub fn exchange_device_code(
        &self,
    ) -> Result<DeviceAuthorizationRequest<TE>, ConfigurationError> {
        Ok(self.exchange_device_code_impl(
            self.device_authorization_url
                .as_ref()
                .ok_or(ConfigurationError::MissingUrl("device authorization"))?,
        ))
    }

    /// Return the [RFC 8628](https://tools.ietf.org/html/rfc8628) device authorization endpoint
    /// used for the Device Authorization Flow.
    ///
    /// See [`exchange_device_code()`](Self::exchange_device_code).
    pub fn device_authorization_url(&self) -> Option<&DeviceAuthorizationUrl> {
        self.device_authorization_url.as_ref()
    }
}

/// Methods requiring an introspection endpoint.
impl<TE, TR, TIR, RT, TRE, HasAuthUrl, HasDeviceAuthUrl, HasRevocationUrl, HasTokenUrl>
    Client<
        TE,
        TR,
        TIR,
        RT,
        TRE,
        HasAuthUrl,
        HasDeviceAuthUrl,
        EndpointSet,
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
    HasRevocationUrl: EndpointState,
    HasTokenUrl: EndpointState,
{
    /// Retrieve metadata for an access token using the
    /// [`RFC 7662`](https://tools.ietf.org/html/rfc7662) introspection endpoint.
    ///
    /// Requires [`set_introspection_url()`](Self::set_introspection_url) to have been previously
    /// called to set the introspection endpoint.
    pub fn introspect<'a>(&'a self, token: &'a AccessToken) -> IntrospectionRequest<'a, TE, TIR> {
        self.introspect_impl(self.introspection_url(), token)
    }

    /// Return the [RFC 7662](https://tools.ietf.org/html/rfc7662) introspection endpoint.
    pub fn introspection_url(&self) -> &IntrospectionUrl {
        // This is enforced statically via the HasIntrospectionUrl generic type.
        self.introspection_url
            .as_ref()
            .expect("should have introspection_url")
    }
}

/// Methods with a possibly-set introspection endpoint.
impl<TE, TR, TIR, RT, TRE, HasAuthUrl, HasDeviceAuthUrl, HasRevocationUrl, HasTokenUrl>
    Client<
        TE,
        TR,
        TIR,
        RT,
        TRE,
        HasAuthUrl,
        HasDeviceAuthUrl,
        EndpointMaybeSet,
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
    HasRevocationUrl: EndpointState,
    HasTokenUrl: EndpointState,
{
    /// Retrieve metadata for an access token using the
    /// [`RFC 7662`](https://tools.ietf.org/html/rfc7662) introspection endpoint.
    ///
    /// Requires [`set_introspection_url_option()`](Self::set_introspection_url_option) to have been
    /// previously called to set the introspection endpoint.
    pub fn introspect<'a>(
        &'a self,
        token: &'a AccessToken,
    ) -> Result<IntrospectionRequest<'a, TE, TIR>, ConfigurationError> {
        Ok(self.introspect_impl(
            self.introspection_url
                .as_ref()
                .ok_or(ConfigurationError::MissingUrl("introspection"))?,
            token,
        ))
    }

    /// Return the [RFC 7662](https://tools.ietf.org/html/rfc7662) introspection endpoint.
    pub fn introspection_url(&self) -> Option<&IntrospectionUrl> {
        self.introspection_url.as_ref()
    }
}

/// Methods requiring a revocation endpoint.
impl<TE, TR, TIR, RT, TRE, HasAuthUrl, HasDeviceAuthUrl, HasIntrospectionUrl, HasTokenUrl>
    Client<
        TE,
        TR,
        TIR,
        RT,
        TRE,
        HasAuthUrl,
        HasDeviceAuthUrl,
        HasIntrospectionUrl,
        EndpointSet,
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
    HasTokenUrl: EndpointState,
{
    /// Revoke an access or refresh token using the [RFC 7009](https://tools.ietf.org/html/rfc7009)
    /// revocation endpoint.
    ///
    /// Requires [`set_revocation_url()`](Self::set_revocation_url) to have been previously
    /// called to set the revocation endpoint.
    pub fn revoke_token(
        &self,
        token: RT,
    ) -> Result<RevocationRequest<RT, TRE>, ConfigurationError> {
        self.revoke_token_impl(self.revocation_url(), token)
    }

    /// Return the [RFC 7009](https://tools.ietf.org/html/rfc7009) revocation endpoint.
    ///
    /// See [`revoke_token()`](Self::revoke_token()).
    pub fn revocation_url(&self) -> &RevocationUrl {
        // This is enforced statically via the HasRevocationUrl generic type.
        self.revocation_url
            .as_ref()
            .expect("should have revocation_url")
    }
}

/// Methods with a possible-set revocation endpoint.
impl<TE, TR, TIR, RT, TRE, HasAuthUrl, HasDeviceAuthUrl, HasIntrospectionUrl, HasTokenUrl>
    Client<
        TE,
        TR,
        TIR,
        RT,
        TRE,
        HasAuthUrl,
        HasDeviceAuthUrl,
        HasIntrospectionUrl,
        EndpointMaybeSet,
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
    HasTokenUrl: EndpointState,
{
    /// Revoke an access or refresh token using the [RFC 7009](https://tools.ietf.org/html/rfc7009)
    /// revocation endpoint.
    ///
    /// Requires [`set_revocation_url_option()`](Self::set_revocation_url_option) to have been
    /// previously called to set the revocation endpoint.
    pub fn revoke_token(
        &self,
        token: RT,
    ) -> Result<RevocationRequest<RT, TRE>, ConfigurationError> {
        self.revoke_token_impl(
            self.revocation_url
                .as_ref()
                .ok_or(ConfigurationError::MissingUrl("revocation"))?,
            token,
        )
    }

    /// Return the [RFC 7009](https://tools.ietf.org/html/rfc7009) revocation endpoint.
    ///
    /// See [`revoke_token()`](Self::revoke_token()).
    pub fn revocation_url(&self) -> Option<&RevocationUrl> {
        self.revocation_url.as_ref()
    }
}
