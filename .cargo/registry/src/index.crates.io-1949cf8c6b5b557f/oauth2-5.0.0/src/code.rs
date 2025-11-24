use crate::{
    AuthUrl, Client, ClientId, CsrfToken, EndpointState, ErrorResponse, PkceCodeChallenge,
    RedirectUrl, ResponseType, RevocableToken, Scope, TokenIntrospectionResponse, TokenResponse,
};

use url::Url;

use std::borrow::Cow;

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
    pub(crate) fn authorize_url_impl<'a, S>(
        &'a self,
        auth_url: &'a AuthUrl,
        state_fn: S,
    ) -> AuthorizationRequest<'a>
    where
        S: FnOnce() -> CsrfToken,
    {
        AuthorizationRequest {
            auth_url,
            client_id: &self.client_id,
            extra_params: Vec::new(),
            pkce_challenge: None,
            redirect_url: self.redirect_url.as_ref().map(Cow::Borrowed),
            response_type: "code".into(),
            scopes: Vec::new(),
            state: state_fn(),
        }
    }
}

/// A request to the authorization endpoint
#[derive(Debug)]
pub struct AuthorizationRequest<'a> {
    pub(crate) auth_url: &'a AuthUrl,
    pub(crate) client_id: &'a ClientId,
    pub(crate) extra_params: Vec<(Cow<'a, str>, Cow<'a, str>)>,
    pub(crate) pkce_challenge: Option<PkceCodeChallenge>,
    pub(crate) redirect_url: Option<Cow<'a, RedirectUrl>>,
    pub(crate) response_type: Cow<'a, str>,
    pub(crate) scopes: Vec<Cow<'a, Scope>>,
    pub(crate) state: CsrfToken,
}
impl<'a> AuthorizationRequest<'a> {
    /// Appends a new scope to the authorization URL.
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

    /// Appends an extra param to the authorization URL.
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

    /// Enables the [Implicit Grant](https://tools.ietf.org/html/rfc6749#section-4.2) flow.
    pub fn use_implicit_flow(mut self) -> Self {
        self.response_type = "token".into();
        self
    }

    /// Enables custom flows other than the `code` and `token` (implicit flow) grant.
    pub fn set_response_type(mut self, response_type: &ResponseType) -> Self {
        self.response_type = (**response_type).to_owned().into();
        self
    }

    /// Enables the use of [Proof Key for Code Exchange](https://tools.ietf.org/html/rfc7636)
    /// (PKCE).
    ///
    /// PKCE is *highly recommended* for all public clients (i.e., those for which there
    /// is no client secret or for which the client secret is distributed with the client,
    /// such as in a native, mobile app, or browser app).
    pub fn set_pkce_challenge(mut self, pkce_code_challenge: PkceCodeChallenge) -> Self {
        self.pkce_challenge = Some(pkce_code_challenge);
        self
    }

    /// Overrides the `redirect_url` to the one specified.
    pub fn set_redirect_uri(mut self, redirect_url: Cow<'a, RedirectUrl>) -> Self {
        self.redirect_url = Some(redirect_url);
        self
    }

    /// Returns the full authorization URL and CSRF state for this authorization
    /// request.
    pub fn url(self) -> (Url, CsrfToken) {
        let scopes = self
            .scopes
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>()
            .join(" ");

        let url = {
            let mut pairs: Vec<(&str, &str)> = vec![
                ("response_type", self.response_type.as_ref()),
                ("client_id", self.client_id),
                ("state", self.state.secret()),
            ];

            if let Some(ref pkce_challenge) = self.pkce_challenge {
                pairs.push(("code_challenge", pkce_challenge.as_str()));
                pairs.push(("code_challenge_method", pkce_challenge.method().as_str()));
            }

            if let Some(ref redirect_url) = self.redirect_url {
                pairs.push(("redirect_uri", redirect_url.as_str()));
            }

            if !scopes.is_empty() {
                pairs.push(("scope", &scopes));
            }

            let mut url: Url = self.auth_url.url().to_owned();

            url.query_pairs_mut()
                .extend_pairs(pairs.iter().map(|&(k, v)| (k, v)));

            url.query_pairs_mut()
                .extend_pairs(self.extra_params.iter().cloned());
            url
        };

        (url, self.state)
    }
}

#[cfg(test)]
mod tests {
    use crate::basic::BasicClient;
    use crate::tests::new_client;
    use crate::{
        AuthUrl, ClientId, ClientSecret, CsrfToken, PkceCodeChallenge, PkceCodeVerifier,
        RedirectUrl, ResponseType, Scope, TokenUrl,
    };

    use url::form_urlencoded::byte_serialize;
    use url::Url;

    use std::borrow::Cow;

    #[test]
    fn test_authorize_url() {
        let client = new_client();
        let (url, _) = client
            .authorize_url(|| CsrfToken::new("csrf_token".to_string()))
            .url();

        assert_eq!(
            Url::parse(
                "https://example.com/auth?response_type=code&client_id=aaa&state=csrf_token"
            )
            .unwrap(),
            url
        );
    }

    #[test]
    fn test_authorize_random() {
        let client = new_client();
        let (url, csrf_state) = client.authorize_url(CsrfToken::new_random).url();

        assert_eq!(
            Url::parse(&format!(
                "https://example.com/auth?response_type=code&client_id=aaa&state={}",
                byte_serialize(csrf_state.secret().clone().into_bytes().as_slice())
                    .collect::<Vec<_>>()
                    .join("")
            ))
            .unwrap(),
            url
        );
    }

    #[test]
    fn test_authorize_url_pkce() {
        // Example from https://tools.ietf.org/html/rfc7636#appendix-B
        let client = new_client();

        let (url, _) = client
            .authorize_url(|| CsrfToken::new("csrf_token".to_string()))
            .set_pkce_challenge(PkceCodeChallenge::from_code_verifier_sha256(
                &PkceCodeVerifier::new("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk".to_string()),
            ))
            .url();
        assert_eq!(
            Url::parse(concat!(
                "https://example.com/auth",
                "?response_type=code&client_id=aaa",
                "&state=csrf_token",
                "&code_challenge=E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM",
                "&code_challenge_method=S256",
            ))
            .unwrap(),
            url
        );
    }

    #[test]
    fn test_authorize_url_implicit() {
        let client = new_client();

        let (url, _) = client
            .authorize_url(|| CsrfToken::new("csrf_token".to_string()))
            .use_implicit_flow()
            .url();

        assert_eq!(
            Url::parse(
                "https://example.com/auth?response_type=token&client_id=aaa&state=csrf_token"
            )
            .unwrap(),
            url
        );
    }

    #[test]
    fn test_authorize_url_with_param() {
        let client = BasicClient::new(ClientId::new("aaa".to_string()))
            .set_client_secret(ClientSecret::new("bbb".to_string()))
            .set_auth_uri(AuthUrl::new("https://example.com/auth?foo=bar".to_string()).unwrap())
            .set_token_uri(TokenUrl::new("https://example.com/token".to_string()).unwrap());

        let (url, _) = client
            .authorize_url(|| CsrfToken::new("csrf_token".to_string()))
            .url();

        assert_eq!(
            Url::parse(
                "https://example.com/auth?foo=bar&response_type=code&client_id=aaa&state=csrf_token"
            )
              .unwrap(),
            url
        );
    }

    #[test]
    fn test_authorize_url_with_scopes() {
        let scopes = vec![
            Scope::new("read".to_string()),
            Scope::new("write".to_string()),
        ];
        let (url, _) = new_client()
            .authorize_url(|| CsrfToken::new("csrf_token".to_string()))
            .add_scopes(scopes)
            .url();

        assert_eq!(
            Url::parse(
                "https://example.com/auth\
             ?response_type=code\
             &client_id=aaa\
             &state=csrf_token\
             &scope=read+write"
            )
            .unwrap(),
            url
        );
    }

    #[test]
    fn test_authorize_url_with_one_scope() {
        let (url, _) = new_client()
            .authorize_url(|| CsrfToken::new("csrf_token".to_string()))
            .add_scope(Scope::new("read".to_string()))
            .url();

        assert_eq!(
            Url::parse(
                "https://example.com/auth\
             ?response_type=code\
             &client_id=aaa\
             &state=csrf_token\
             &scope=read"
            )
            .unwrap(),
            url
        );
    }

    #[test]
    fn test_authorize_url_with_extension_response_type() {
        let client = new_client();

        let (url, _) = client
            .authorize_url(|| CsrfToken::new("csrf_token".to_string()))
            .set_response_type(&ResponseType::new("code token".to_string()))
            .add_extra_param("foo", "bar")
            .url();

        assert_eq!(
            Url::parse(
                "https://example.com/auth?response_type=code+token&client_id=aaa&state=csrf_token\
             &foo=bar"
            )
            .unwrap(),
            url
        );
    }

    #[test]
    fn test_authorize_url_with_redirect_url() {
        let client = new_client()
            .set_redirect_uri(RedirectUrl::new("https://localhost/redirect".to_string()).unwrap());

        let (url, _) = client
            .authorize_url(|| CsrfToken::new("csrf_token".to_string()))
            .url();

        assert_eq!(
            Url::parse(
                "https://example.com/auth?response_type=code\
             &client_id=aaa\
             &state=csrf_token\
             &redirect_uri=https%3A%2F%2Flocalhost%2Fredirect"
            )
            .unwrap(),
            url
        );
    }

    #[test]
    fn test_authorize_url_with_redirect_url_override() {
        let client = new_client()
            .set_redirect_uri(RedirectUrl::new("https://localhost/redirect".to_string()).unwrap());

        let (url, _) = client
            .authorize_url(|| CsrfToken::new("csrf_token".to_string()))
            .set_redirect_uri(Cow::Owned(
                RedirectUrl::new("https://localhost/alternative".to_string()).unwrap(),
            ))
            .url();

        assert_eq!(
            Url::parse(
                "https://example.com/auth?response_type=code\
             &client_id=aaa\
             &state=csrf_token\
             &redirect_uri=https%3A%2F%2Flocalhost%2Falternative"
            )
            .unwrap(),
            url
        );
    }
}
