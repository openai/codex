use crate::basic::BasicErrorResponseType;
use crate::endpoint::{endpoint_request, endpoint_response};
use crate::types::VerificationUriComplete;
use crate::{
    AsyncHttpClient, AuthType, Client, ClientId, ClientSecret, DeviceAuthorizationUrl, DeviceCode,
    EndUserVerificationUrl, EndpointState, ErrorResponse, ErrorResponseType, HttpRequest,
    HttpResponse, RequestTokenError, RevocableToken, Scope, StandardErrorResponse, SyncHttpClient,
    TokenIntrospectionResponse, TokenResponse, TokenUrl, UserCode,
};

use chrono::{DateTime, Utc};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use std::borrow::Cow;
use std::error::Error;
use std::fmt::Error as FormatterError;
use std::fmt::{Debug, Display, Formatter};
use std::future::Future;
use std::marker::PhantomData;
use std::sync::Arc;
use std::time::Duration;

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
    pub(crate) fn exchange_device_code_impl<'a>(
        &'a self,
        device_authorization_url: &'a DeviceAuthorizationUrl,
    ) -> DeviceAuthorizationRequest<'a, TE> {
        DeviceAuthorizationRequest {
            auth_type: &self.auth_type,
            client_id: &self.client_id,
            client_secret: self.client_secret.as_ref(),
            extra_params: Vec::new(),
            scopes: Vec::new(),
            device_authorization_url,
            _phantom: PhantomData,
        }
    }

    pub(crate) fn exchange_device_access_token_impl<'a, EF>(
        &'a self,
        token_url: &'a TokenUrl,
        auth_response: &'a DeviceAuthorizationResponse<EF>,
    ) -> DeviceAccessTokenRequest<'a, 'static, TR, EF>
    where
        EF: ExtraDeviceAuthorizationFields,
    {
        DeviceAccessTokenRequest {
            auth_type: &self.auth_type,
            client_id: &self.client_id,
            client_secret: self.client_secret.as_ref(),
            extra_params: Vec::new(),
            token_url,
            dev_auth_resp: auth_response,
            time_fn: Arc::new(Utc::now),
            max_backoff_interval: None,
            _phantom: PhantomData,
        }
    }
}

/// The request for a set of verification codes from the authorization server.
///
/// See <https://tools.ietf.org/html/rfc8628#section-3.1>.
#[derive(Debug)]
pub struct DeviceAuthorizationRequest<'a, TE>
where
    TE: ErrorResponse,
{
    pub(crate) auth_type: &'a AuthType,
    pub(crate) client_id: &'a ClientId,
    pub(crate) client_secret: Option<&'a ClientSecret>,
    pub(crate) extra_params: Vec<(Cow<'a, str>, Cow<'a, str>)>,
    pub(crate) scopes: Vec<Cow<'a, Scope>>,
    pub(crate) device_authorization_url: &'a DeviceAuthorizationUrl,
    pub(crate) _phantom: PhantomData<TE>,
}

impl<'a, TE> DeviceAuthorizationRequest<'a, TE>
where
    TE: ErrorResponse + 'static,
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

    fn prepare_request<RE>(self) -> Result<HttpRequest, RequestTokenError<RE, TE>>
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
            self.device_authorization_url.url(),
            vec![],
        )
        .map_err(|err| RequestTokenError::Other(format!("failed to prepare request: {err}")))
    }

    /// Synchronously sends the request to the authorization server and awaits a response.
    pub fn request<C, EF>(
        self,
        http_client: &C,
    ) -> Result<DeviceAuthorizationResponse<EF>, RequestTokenError<<C as SyncHttpClient>::Error, TE>>
    where
        C: SyncHttpClient,
        EF: ExtraDeviceAuthorizationFields,
    {
        endpoint_response(http_client.call(self.prepare_request()?)?)
    }

    /// Asynchronously sends the request to the authorization server and returns a Future.
    pub fn request_async<'c, C, EF>(
        self,
        http_client: &'c C,
    ) -> impl Future<
        Output = Result<
            DeviceAuthorizationResponse<EF>,
            RequestTokenError<<C as AsyncHttpClient<'c>>::Error, TE>,
        >,
    > + 'c
    where
        Self: 'c,
        C: AsyncHttpClient<'c>,
        EF: ExtraDeviceAuthorizationFields,
    {
        Box::pin(async move { endpoint_response(http_client.call(self.prepare_request()?).await?) })
    }
}

/// The request for a device access token from the authorization server.
///
/// See <https://tools.ietf.org/html/rfc8628#section-3.4>.
#[derive(Clone)]
pub struct DeviceAccessTokenRequest<'a, 'b, TR, EF>
where
    TR: TokenResponse,
    EF: ExtraDeviceAuthorizationFields,
{
    pub(crate) auth_type: &'a AuthType,
    pub(crate) client_id: &'a ClientId,
    pub(crate) client_secret: Option<&'a ClientSecret>,
    pub(crate) extra_params: Vec<(Cow<'a, str>, Cow<'a, str>)>,
    pub(crate) token_url: &'a TokenUrl,
    pub(crate) dev_auth_resp: &'a DeviceAuthorizationResponse<EF>,
    pub(crate) time_fn: Arc<dyn Fn() -> DateTime<Utc> + Send + Sync + 'b>,
    pub(crate) max_backoff_interval: Option<Duration>,
    pub(crate) _phantom: PhantomData<(TR, EF)>,
}

impl<'a, 'b, TR, EF> DeviceAccessTokenRequest<'a, 'b, TR, EF>
where
    TR: TokenResponse,
    EF: ExtraDeviceAuthorizationFields,
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

    /// Specifies a function for returning the current time.
    ///
    /// This function is used while polling the authorization server.
    pub fn set_time_fn<'t, T>(self, time_fn: T) -> DeviceAccessTokenRequest<'a, 't, TR, EF>
    where
        T: Fn() -> DateTime<Utc> + Send + Sync + 't,
    {
        DeviceAccessTokenRequest {
            auth_type: self.auth_type,
            client_id: self.client_id,
            client_secret: self.client_secret,
            extra_params: self.extra_params,
            token_url: self.token_url,
            dev_auth_resp: self.dev_auth_resp,
            time_fn: Arc::new(time_fn),
            max_backoff_interval: self.max_backoff_interval,
            _phantom: PhantomData,
        }
    }

    /// Sets the upper limit of the sleep interval to use for polling the token endpoint when the
    /// HTTP client returns an error (e.g., in case of connection timeout).
    pub fn set_max_backoff_interval(mut self, interval: Duration) -> Self {
        self.max_backoff_interval = Some(interval);
        self
    }

    /// Synchronously polls the authorization server for a response, waiting
    /// using a user defined sleep function.
    pub fn request<C, S>(
        self,
        http_client: &C,
        sleep_fn: S,
        timeout: Option<Duration>,
    ) -> Result<TR, RequestTokenError<<C as SyncHttpClient>::Error, DeviceCodeErrorResponse>>
    where
        C: SyncHttpClient,
        S: Fn(Duration),
    {
        // Get the request timeout and starting interval
        let timeout_dt = self.compute_timeout(timeout)?;
        let mut interval = self.dev_auth_resp.interval();

        // Loop while requesting a token.
        loop {
            let now = (*self.time_fn)();
            if now > timeout_dt {
                break Err(RequestTokenError::ServerResponse(
                    DeviceCodeErrorResponse::new(
                        DeviceCodeErrorResponseType::ExpiredToken,
                        Some(String::from("This device code has expired.")),
                        None,
                    ),
                ));
            }

            match self.process_response(http_client.call(self.prepare_request()?), interval) {
                DeviceAccessTokenPollResult::ContinueWithNewPollInterval(new_interval) => {
                    interval = new_interval
                }
                DeviceAccessTokenPollResult::Done(res) => break res,
            }

            // Sleep here using the provided sleep function.
            sleep_fn(interval);
        }
    }

    /// Asynchronously sends the request to the authorization server and awaits a response.
    pub fn request_async<'c, C, S, SF>(
        self,
        http_client: &'c C,
        sleep_fn: S,
        timeout: Option<Duration>,
    ) -> impl Future<
        Output = Result<
            TR,
            RequestTokenError<<C as AsyncHttpClient<'c>>::Error, DeviceCodeErrorResponse>,
        >,
    > + 'c
    where
        Self: 'c,
        C: AsyncHttpClient<'c>,
        S: Fn(Duration) -> SF + 'c,
        SF: Future<Output = ()>,
    {
        Box::pin(async move {
            // Get the request timeout and starting interval
            let timeout_dt = self.compute_timeout(timeout)?;
            let mut interval = self.dev_auth_resp.interval();

            // Loop while requesting a token.
            loop {
                let now = (*self.time_fn)();
                if now > timeout_dt {
                    break Err(RequestTokenError::ServerResponse(
                        DeviceCodeErrorResponse::new(
                            DeviceCodeErrorResponseType::ExpiredToken,
                            Some(String::from("This device code has expired.")),
                            None,
                        ),
                    ));
                }

                match self
                    .process_response(http_client.call(self.prepare_request()?).await, interval)
                {
                    DeviceAccessTokenPollResult::ContinueWithNewPollInterval(new_interval) => {
                        interval = new_interval
                    }
                    DeviceAccessTokenPollResult::Done(res) => break res,
                }

                // Sleep here using the provided sleep function.
                sleep_fn(interval).await;
            }
        })
    }

    fn prepare_request<RE, TE>(&self) -> Result<HttpRequest, RequestTokenError<RE, TE>>
    where
        RE: Error + 'static,
        TE: ErrorResponse + 'static,
    {
        endpoint_request(
            self.auth_type,
            self.client_id,
            self.client_secret,
            &self.extra_params,
            None,
            None,
            self.token_url.url(),
            vec![
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ("device_code", self.dev_auth_resp.device_code().secret()),
            ],
        )
        .map_err(|err| RequestTokenError::Other(format!("failed to prepare request: {err}")))
    }

    fn process_response<RE>(
        &self,
        res: Result<HttpResponse, RE>,
        current_interval: Duration,
    ) -> DeviceAccessTokenPollResult<TR, RE, DeviceCodeErrorResponse>
    where
        RE: Error + 'static,
    {
        let http_response = match res {
            Ok(inner) => inner,
            Err(_) => {
                // RFC 8628 requires a backoff in cases of connection timeout, but we can't
                // distinguish between connection timeouts and other HTTP client request errors
                // here. Set a maximum backoff so that the client doesn't effectively backoff
                // infinitely when there are network issues unrelated to server load.
                const DEFAULT_MAX_BACKOFF_INTERVAL: Duration = Duration::from_secs(10);
                let new_interval = std::cmp::min(
                    current_interval.checked_mul(2).unwrap_or(current_interval),
                    self.max_backoff_interval
                        .unwrap_or(DEFAULT_MAX_BACKOFF_INTERVAL),
                );
                return DeviceAccessTokenPollResult::ContinueWithNewPollInterval(new_interval);
            }
        };

        // Explicitly process the response with a DeviceCodeErrorResponse
        let res = endpoint_response::<RE, DeviceCodeErrorResponse, TR>(http_response);
        match res {
            // On a ServerResponse error, the error needs inspecting as a DeviceCodeErrorResponse
            // to work out whether a retry needs to happen.
            Err(RequestTokenError::ServerResponse(dcer)) => {
                match dcer.error() {
                    // On AuthorizationPending, a retry needs to happen with the same poll interval.
                    DeviceCodeErrorResponseType::AuthorizationPending => {
                        DeviceAccessTokenPollResult::ContinueWithNewPollInterval(current_interval)
                    }
                    // On SlowDown, a retry needs to happen with a larger poll interval.
                    DeviceCodeErrorResponseType::SlowDown => {
                        DeviceAccessTokenPollResult::ContinueWithNewPollInterval(
                            current_interval + Duration::from_secs(5),
                        )
                    }

                    // On any other error, just return the error.
                    _ => DeviceAccessTokenPollResult::Done(Err(RequestTokenError::ServerResponse(
                        dcer,
                    ))),
                }
            }

            // On any other success or failure, return the failure.
            res => DeviceAccessTokenPollResult::Done(res),
        }
    }

    fn compute_timeout<RE>(
        &self,
        timeout: Option<Duration>,
    ) -> Result<DateTime<Utc>, RequestTokenError<RE, DeviceCodeErrorResponse>>
    where
        RE: Error + 'static,
    {
        // Calculate the request timeout - if the user specified a timeout,
        // use that, otherwise use the value given by the device authorization
        // response.
        let timeout_dur = timeout.unwrap_or_else(|| self.dev_auth_resp.expires_in());
        let chrono_timeout = chrono::Duration::from_std(timeout_dur).map_err(|e| {
            RequestTokenError::Other(format!(
                "failed to convert `{timeout_dur:?}` to `chrono::Duration`: {e}"
            ))
        })?;

        // Calculate the DateTime at which the request times out.
        let timeout_dt = (*self.time_fn)()
            .checked_add_signed(chrono_timeout)
            .ok_or_else(|| RequestTokenError::Other("failed to calculate timeout".to_string()))?;

        Ok(timeout_dt)
    }
}

/// The minimum amount of time in seconds that the client SHOULD wait
/// between polling requests to the token endpoint.  If no value is
/// provided, clients MUST use 5 as the default.
fn default_devicecode_interval() -> u64 {
    5
}

fn deserialize_devicecode_interval<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: serde::de::Deserializer<'de>,
{
    struct NumOrNull;

    impl<'de> serde::de::Visitor<'de> for NumOrNull {
        type Value = u64;

        fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
            formatter.write_str("non-negative integer or null")
        }

        fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
        where
            E: Error,
        {
            Ok(v)
        }

        fn visit_unit<E>(self) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(default_devicecode_interval())
        }
    }

    deserializer.deserialize_any(NumOrNull)
}

/// Trait for adding extra fields to the `DeviceAuthorizationResponse`.
pub trait ExtraDeviceAuthorizationFields: DeserializeOwned + Debug + Serialize {}

#[derive(Clone, Debug, Deserialize, Serialize)]
/// Empty (default) extra token fields.
pub struct EmptyExtraDeviceAuthorizationFields {}
impl ExtraDeviceAuthorizationFields for EmptyExtraDeviceAuthorizationFields {}

/// Standard OAuth2 device authorization response.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DeviceAuthorizationResponse<EF>
where
    EF: ExtraDeviceAuthorizationFields,
{
    /// The device verification code.
    device_code: DeviceCode,

    /// The end-user verification code.
    user_code: UserCode,

    /// The end-user verification URI on the authorization The URI should be
    /// short and easy to remember as end users will be asked to manually type
    /// it into their user agent.
    ///
    /// The `verification_url` alias here is a deviation from the RFC, as
    /// implementations of device authorization flow predate RFC 8628.
    #[serde(alias = "verification_url")]
    verification_uri: EndUserVerificationUrl,

    /// A verification URI that includes the "user_code" (or other information
    /// with the same function as the "user_code"), which is designed for
    /// non-textual transmission.
    #[serde(skip_serializing_if = "Option::is_none")]
    verification_uri_complete: Option<VerificationUriComplete>,

    /// The lifetime in seconds of the "device_code" and "user_code".
    expires_in: u64,

    /// The minimum amount of time in seconds that the client SHOULD wait
    /// between polling requests to the token endpoint.  If no value is
    /// provided, clients MUST use 5 as the default.
    #[serde(
        default = "default_devicecode_interval",
        deserialize_with = "deserialize_devicecode_interval"
    )]
    interval: u64,

    #[serde(bound = "EF: ExtraDeviceAuthorizationFields", flatten)]
    extra_fields: EF,
}

impl<EF> DeviceAuthorizationResponse<EF>
where
    EF: ExtraDeviceAuthorizationFields,
{
    /// The device verification code.
    pub fn device_code(&self) -> &DeviceCode {
        &self.device_code
    }

    /// The end-user verification code.
    pub fn user_code(&self) -> &UserCode {
        &self.user_code
    }

    /// The end-user verification URI on the authorization The URI should be
    /// short and easy to remember as end users will be asked to manually type
    /// it into their user agent.
    pub fn verification_uri(&self) -> &EndUserVerificationUrl {
        &self.verification_uri
    }

    /// A verification URI that includes the "user_code" (or other information
    /// with the same function as the "user_code"), which is designed for
    /// non-textual transmission.
    pub fn verification_uri_complete(&self) -> Option<&VerificationUriComplete> {
        self.verification_uri_complete.as_ref()
    }

    /// The lifetime in seconds of the "device_code" and "user_code".
    pub fn expires_in(&self) -> Duration {
        Duration::from_secs(self.expires_in)
    }

    /// The minimum amount of time in seconds that the client SHOULD wait
    /// between polling requests to the token endpoint.  If no value is
    /// provided, clients MUST use 5 as the default.
    pub fn interval(&self) -> Duration {
        Duration::from_secs(self.interval)
    }

    /// Any extra fields returned on the response.
    pub fn extra_fields(&self) -> &EF {
        &self.extra_fields
    }
}

/// Standard implementation of DeviceAuthorizationResponse which throws away
/// extra received response fields.
pub type StandardDeviceAuthorizationResponse =
    DeviceAuthorizationResponse<EmptyExtraDeviceAuthorizationFields>;

/// Basic access token error types.
///
/// These error types are defined in
/// [Section 5.2 of RFC 6749](https://tools.ietf.org/html/rfc6749#section-5.2) and
/// [Section 3.5 of RFC 6749](https://tools.ietf.org/html/rfc8628#section-3.5)
#[derive(Clone, PartialEq, Eq)]
pub enum DeviceCodeErrorResponseType {
    /// The authorization request is still pending as the end user hasn't
    /// yet completed the user-interaction steps.  The client SHOULD repeat the
    /// access token request to the token endpoint.  Before each new request,
    /// the client MUST wait at least the number of seconds specified by the
    /// "interval" parameter of the device authorization response, or 5 seconds
    /// if none was provided, and respect any increase in the polling interval
    /// required by the "slow_down" error.
    AuthorizationPending,
    /// A variant of "authorization_pending", the authorization request is
    /// still pending and polling should continue, but the interval MUST be
    /// increased by 5 seconds for this and all subsequent requests.
    SlowDown,
    /// The authorization request was denied.
    AccessDenied,
    /// The "device_code" has expired, and the device authorization session has
    /// concluded.  The client MAY commence a new device authorization request
    /// but SHOULD wait for user interaction before restarting to avoid
    /// unnecessary polling.
    ExpiredToken,
    /// A Basic response type
    Basic(BasicErrorResponseType),
}
impl DeviceCodeErrorResponseType {
    fn from_str(s: &str) -> Self {
        match BasicErrorResponseType::from_str(s) {
            BasicErrorResponseType::Extension(ext) => match ext.as_str() {
                "authorization_pending" => DeviceCodeErrorResponseType::AuthorizationPending,
                "slow_down" => DeviceCodeErrorResponseType::SlowDown,
                "access_denied" => DeviceCodeErrorResponseType::AccessDenied,
                "expired_token" => DeviceCodeErrorResponseType::ExpiredToken,
                _ => DeviceCodeErrorResponseType::Basic(BasicErrorResponseType::Extension(ext)),
            },
            basic => DeviceCodeErrorResponseType::Basic(basic),
        }
    }
}
impl AsRef<str> for DeviceCodeErrorResponseType {
    fn as_ref(&self) -> &str {
        match self {
            DeviceCodeErrorResponseType::AuthorizationPending => "authorization_pending",
            DeviceCodeErrorResponseType::SlowDown => "slow_down",
            DeviceCodeErrorResponseType::AccessDenied => "access_denied",
            DeviceCodeErrorResponseType::ExpiredToken => "expired_token",
            DeviceCodeErrorResponseType::Basic(basic) => basic.as_ref(),
        }
    }
}
impl<'de> serde::Deserialize<'de> for DeviceCodeErrorResponseType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        let variant_str = String::deserialize(deserializer)?;
        Ok(Self::from_str(&variant_str))
    }
}
impl serde::ser::Serialize for DeviceCodeErrorResponseType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::ser::Serializer,
    {
        serializer.serialize_str(self.as_ref())
    }
}
impl ErrorResponseType for DeviceCodeErrorResponseType {}
impl Debug for DeviceCodeErrorResponseType {
    fn fmt(&self, f: &mut Formatter) -> Result<(), FormatterError> {
        Display::fmt(self, f)
    }
}

impl Display for DeviceCodeErrorResponseType {
    fn fmt(&self, f: &mut Formatter) -> Result<(), FormatterError> {
        write!(f, "{}", self.as_ref())
    }
}

/// Error response specialization for device code OAuth2 implementation.
pub type DeviceCodeErrorResponse = StandardErrorResponse<DeviceCodeErrorResponseType>;

pub(crate) enum DeviceAccessTokenPollResult<TR, RE, TE>
where
    TE: ErrorResponse + 'static,
    TR: TokenResponse,
    RE: Error + 'static,
{
    ContinueWithNewPollInterval(Duration),
    Done(Result<TR, RequestTokenError<RE, TE>>),
}

#[cfg(test)]
mod tests {
    use crate::basic::BasicTokenType;
    use crate::devicecode::default_devicecode_interval;
    use crate::tests::{mock_http_client, mock_http_client_success_fail, new_client};
    use crate::{
        DeviceAuthorizationResponse, DeviceAuthorizationUrl, DeviceCodeErrorResponse,
        DeviceCodeErrorResponseType, EmptyExtraDeviceAuthorizationFields, RequestTokenError, Scope,
        StandardDeviceAuthorizationResponse, TokenResponse,
    };

    use chrono::{DateTime, Utc};
    use http::header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE};
    use http::{HeaderValue, Response, StatusCode};

    use std::time::Duration;

    fn new_device_auth_details(expires_in: u32) -> StandardDeviceAuthorizationResponse {
        let body = format!(
            "{{\
        \"device_code\": \"12345\", \
        \"verification_uri\": \"https://verify/here\", \
        \"user_code\": \"abcde\", \
        \"verification_uri_complete\": \"https://verify/here?abcde\", \
        \"expires_in\": {expires_in}, \
        \"interval\": 1 \
        }}"
        );

        let device_auth_url =
            DeviceAuthorizationUrl::new("https://deviceauth/here".to_string()).unwrap();

        let client = new_client().set_device_authorization_url(device_auth_url.clone());
        client
            .exchange_device_code()
            .add_extra_param("foo", "bar")
            .add_scope(Scope::new("openid".to_string()))
            .request(&mock_http_client(
                vec![
                    (ACCEPT, "application/json"),
                    (CONTENT_TYPE, "application/x-www-form-urlencoded"),
                    (AUTHORIZATION, "Basic YWFhOmJiYg=="),
                ],
                "scope=openid&foo=bar",
                Some(device_auth_url.url().to_owned()),
                Response::builder()
                    .status(StatusCode::OK)
                    .header(
                        CONTENT_TYPE,
                        HeaderValue::from_str("application/json").unwrap(),
                    )
                    .body(body.into_bytes())
                    .unwrap(),
            ))
            .unwrap()
    }

    #[test]
    fn test_device_token_pending_then_success() {
        let details = new_device_auth_details(20);
        assert_eq!("12345", details.device_code().secret());
        assert_eq!("https://verify/here", details.verification_uri().as_str());
        assert_eq!("abcde", details.user_code().secret().as_str());
        assert_eq!(
            "https://verify/here?abcde",
            details
                .verification_uri_complete()
                .unwrap()
                .secret()
                .as_str()
        );
        assert_eq!(Duration::from_secs(20), details.expires_in());
        assert_eq!(Duration::from_secs(1), details.interval());

        let token = new_client()
          .exchange_device_access_token(&details)
          .set_time_fn(mock_time_fn())
          .request(
              &mock_http_client_success_fail(
                  None,
                  vec![
                      (ACCEPT, "application/json"),
                      (CONTENT_TYPE, "application/x-www-form-urlencoded"),
                      (AUTHORIZATION, "Basic YWFhOmJiYg=="),
                  ],
                  "grant_type=urn%3Aietf%3Aparams%3Aoauth%3Agrant-type%3Adevice_code&device_code=12345",
                  Response::builder()
                    .status(StatusCode::BAD_REQUEST)
                    .header(
                        CONTENT_TYPE,
                        HeaderValue::from_str("application/json").unwrap(),
                    )
                    .body("{\
                    \"error\": \"authorization_pending\", \
                    \"error_description\": \"Still waiting for user\"\
                    }"
                      .to_string()
                      .into_bytes())
                    .unwrap(),
                  5,
                  Response::builder()
                    .status(StatusCode::OK)
                    .header(
                        CONTENT_TYPE,
                        HeaderValue::from_str("application/json").unwrap(),
                    )
                    .body("{\
                    \"access_token\": \"12/34\", \
                    \"token_type\": \"bearer\", \
                    \"scope\": \"openid\"\
                    }"
                      .to_string()
                      .into_bytes())
                    .unwrap(),
              ),
              mock_sleep_fn,
              None)
          .unwrap();

        assert_eq!("12/34", token.access_token().secret());
        assert_eq!(BasicTokenType::Bearer, *token.token_type());
        assert_eq!(
            Some(&vec![Scope::new("openid".to_string()),]),
            token.scopes()
        );
        assert_eq!(None, token.expires_in());
        assert!(token.refresh_token().is_none());
    }

    #[test]
    fn test_device_token_slowdown_then_success() {
        let details = new_device_auth_details(3600);
        assert_eq!("12345", details.device_code().secret());
        assert_eq!("https://verify/here", details.verification_uri().as_str());
        assert_eq!("abcde", details.user_code().secret().as_str());
        assert_eq!(
            "https://verify/here?abcde",
            details
                .verification_uri_complete()
                .unwrap()
                .secret()
                .as_str()
        );
        assert_eq!(Duration::from_secs(3600), details.expires_in());
        assert_eq!(Duration::from_secs(1), details.interval());

        let token = new_client()
          .exchange_device_access_token(&details)
          .set_time_fn(mock_time_fn())
          .request(
              &mock_http_client_success_fail(
                  None,
                  vec![
                      (ACCEPT, "application/json"),
                      (CONTENT_TYPE, "application/x-www-form-urlencoded"),
                      (AUTHORIZATION, "Basic YWFhOmJiYg=="),
                  ],
                  "grant_type=urn%3Aietf%3Aparams%3Aoauth%3Agrant-type%3Adevice_code&device_code=12345",
                  Response::builder()
                    .status(StatusCode::BAD_REQUEST)
                    .header(
                        CONTENT_TYPE,
                        HeaderValue::from_str("application/json").unwrap(),
                    )
                    .body("{\
                    \"error\": \"slow_down\", \
                    \"error_description\": \"Woah there partner\"\
                    }"
                      .to_string()
                      .into_bytes())
                    .unwrap(),
                  5,
                  Response::builder()
                    .status(StatusCode::OK)
                    .header(
                        CONTENT_TYPE,
                        HeaderValue::from_str("application/json").unwrap(),
                    )
                    .body("{\
                    \"access_token\": \"12/34\", \
                    \"token_type\": \"bearer\", \
                    \"scope\": \"openid\"\
                    }"
                      .to_string()
                      .into_bytes())
                    .unwrap(),
              ),
              mock_sleep_fn,
              None)
          .unwrap();

        assert_eq!("12/34", token.access_token().secret());
        assert_eq!(BasicTokenType::Bearer, *token.token_type());
        assert_eq!(
            Some(&vec![Scope::new("openid".to_string()),]),
            token.scopes()
        );
        assert_eq!(None, token.expires_in());
        assert!(token.refresh_token().is_none());
    }

    struct IncreasingTime {
        times: std::ops::RangeFrom<i64>,
    }

    impl IncreasingTime {
        fn new() -> Self {
            Self { times: (0..) }
        }
        fn next(&mut self) -> DateTime<Utc> {
            let next_value = self.times.next().unwrap();
            DateTime::from_timestamp(next_value, 0).unwrap()
        }
    }

    /// Creates a time function that increments by one second each time.
    fn mock_time_fn() -> impl Fn() -> DateTime<Utc> + Send + Sync {
        let timer = std::sync::Mutex::new(IncreasingTime::new());
        move || timer.lock().unwrap().next()
    }

    /// Mock sleep function that doesn't actually sleep.
    fn mock_sleep_fn(_: Duration) {}

    #[test]
    fn test_exchange_device_code_and_token() {
        let details = new_device_auth_details(3600);
        assert_eq!("12345", details.device_code().secret());
        assert_eq!("https://verify/here", details.verification_uri().as_str());
        assert_eq!("abcde", details.user_code().secret().as_str());
        assert_eq!(
            "https://verify/here?abcde",
            details
                .verification_uri_complete()
                .unwrap()
                .secret()
                .as_str()
        );
        assert_eq!(Duration::from_secs(3600), details.expires_in());
        assert_eq!(Duration::from_secs(1), details.interval());

        let token = new_client()
          .exchange_device_access_token(&details)
          .set_time_fn(mock_time_fn())
          .request(
              &mock_http_client(
                  vec![
                      (ACCEPT, "application/json"),
                      (CONTENT_TYPE, "application/x-www-form-urlencoded"),
                      (AUTHORIZATION, "Basic YWFhOmJiYg=="),
                  ],
                  "grant_type=urn%3Aietf%3Aparams%3Aoauth%3Agrant-type%3Adevice_code&device_code=12345",
                  None,
                  Response::builder()
                    .status(StatusCode::OK)
                    .header(
                        CONTENT_TYPE,
                        HeaderValue::from_str("application/json").unwrap(),
                    )
                    .body("{\
                    \"access_token\": \"12/34\", \
                    \"token_type\": \"bearer\", \
                    \"scope\": \"openid\"\
                    }"
                      .to_string()
                      .into_bytes())
                    .unwrap(),
              ),
              mock_sleep_fn,
              None)
          .unwrap();

        assert_eq!("12/34", token.access_token().secret());
        assert_eq!(BasicTokenType::Bearer, *token.token_type());
        assert_eq!(
            Some(&vec![Scope::new("openid".to_string()),]),
            token.scopes()
        );
        assert_eq!(None, token.expires_in());
        assert!(token.refresh_token().is_none());
    }

    #[test]
    fn test_device_token_authorization_timeout() {
        let details = new_device_auth_details(2);
        assert_eq!("12345", details.device_code().secret());
        assert_eq!("https://verify/here", details.verification_uri().as_str());
        assert_eq!("abcde", details.user_code().secret().as_str());
        assert_eq!(
            "https://verify/here?abcde",
            details
                .verification_uri_complete()
                .unwrap()
                .secret()
                .as_str()
        );
        assert_eq!(Duration::from_secs(2), details.expires_in());
        assert_eq!(Duration::from_secs(1), details.interval());

        let token = new_client()
          .exchange_device_access_token(&details)
          .set_time_fn(mock_time_fn())
          .request(
              &mock_http_client(
                  vec![
                      (ACCEPT, "application/json"),
                      (CONTENT_TYPE, "application/x-www-form-urlencoded"),
                      (AUTHORIZATION, "Basic YWFhOmJiYg=="),
                  ],
                  "grant_type=urn%3Aietf%3Aparams%3Aoauth%3Agrant-type%3Adevice_code&device_code=12345",
                  None,
                  Response::builder()
                    .status(StatusCode::BAD_REQUEST)
                    .header(
                        CONTENT_TYPE,
                        HeaderValue::from_str("application/json").unwrap(),
                    )
                    .body("{\
                    \"error\": \"authorization_pending\", \
                    \"error_description\": \"Still waiting for user\"\
                    }"
                      .to_string()
                      .into_bytes())
                    .unwrap(),
              ),
              mock_sleep_fn,
              None)
          .err()
          .unwrap();
        match token {
            RequestTokenError::ServerResponse(msg) => assert_eq!(
                msg,
                DeviceCodeErrorResponse::new(
                    DeviceCodeErrorResponseType::ExpiredToken,
                    Some(String::from("This device code has expired.")),
                    None,
                )
            ),
            _ => unreachable!("Error should be an expiry"),
        }
    }

    #[test]
    fn test_device_token_access_denied() {
        let details = new_device_auth_details(2);
        assert_eq!("12345", details.device_code().secret());
        assert_eq!("https://verify/here", details.verification_uri().as_str());
        assert_eq!("abcde", details.user_code().secret().as_str());
        assert_eq!(
            "https://verify/here?abcde",
            details
                .verification_uri_complete()
                .unwrap()
                .secret()
                .as_str()
        );
        assert_eq!(Duration::from_secs(2), details.expires_in());
        assert_eq!(Duration::from_secs(1), details.interval());

        let token = new_client()
          .exchange_device_access_token(&details)
          .set_time_fn(mock_time_fn())
          .request(
              &mock_http_client(
                  vec![
                      (ACCEPT, "application/json"),
                      (CONTENT_TYPE, "application/x-www-form-urlencoded"),
                      (AUTHORIZATION, "Basic YWFhOmJiYg=="),
                  ],
                  "grant_type=urn%3Aietf%3Aparams%3Aoauth%3Agrant-type%3Adevice_code&device_code=12345",
                  None,
                  Response::builder()
                    .status(StatusCode::BAD_REQUEST)
                    .header(
                        CONTENT_TYPE,
                        HeaderValue::from_str("application/json").unwrap(),
                    )
                    .body("{\
                    \"error\": \"access_denied\", \
                    \"error_description\": \"Access Denied\"\
                    }"
                      .to_string()
                      .into_bytes())
                    .unwrap(),
              ),
              mock_sleep_fn,
              None)
          .err()
          .unwrap();
        match token {
            RequestTokenError::ServerResponse(msg) => {
                assert_eq!(msg.error(), &DeviceCodeErrorResponseType::AccessDenied)
            }
            _ => unreachable!("Error should be Access Denied"),
        }
    }

    #[test]
    fn test_device_token_expired() {
        let details = new_device_auth_details(2);
        assert_eq!("12345", details.device_code().secret());
        assert_eq!("https://verify/here", details.verification_uri().as_str());
        assert_eq!("abcde", details.user_code().secret().as_str());
        assert_eq!(
            "https://verify/here?abcde",
            details
                .verification_uri_complete()
                .unwrap()
                .secret()
                .as_str()
        );
        assert_eq!(Duration::from_secs(2), details.expires_in());
        assert_eq!(Duration::from_secs(1), details.interval());

        let token = new_client()
          .exchange_device_access_token(&details)
          .set_time_fn(mock_time_fn())
          .request(
              &mock_http_client(
                  vec![
                      (ACCEPT, "application/json"),
                      (CONTENT_TYPE, "application/x-www-form-urlencoded"),
                      (AUTHORIZATION, "Basic YWFhOmJiYg=="),
                  ],
                  "grant_type=urn%3Aietf%3Aparams%3Aoauth%3Agrant-type%3Adevice_code&device_code=12345",
                  None,
                  Response::builder()
                    .status(StatusCode::BAD_REQUEST)
                    .header(
                        CONTENT_TYPE,
                        HeaderValue::from_str("application/json").unwrap(),
                    )
                    .body("{\
                    \"error\": \"expired_token\", \
                    \"error_description\": \"Token has expired\"\
                    }"
                      .to_string()
                      .into_bytes())
                    .unwrap(),
              ),
              mock_sleep_fn,
              None)
          .err()
          .unwrap();
        match token {
            RequestTokenError::ServerResponse(msg) => {
                assert_eq!(msg.error(), &DeviceCodeErrorResponseType::ExpiredToken)
            }
            _ => unreachable!("Error should be ExpiredToken"),
        }
    }

    #[test]
    fn test_device_auth_response_default_interval() {
        let response: DeviceAuthorizationResponse<EmptyExtraDeviceAuthorizationFields> =
            serde_json::from_str(
                r#"{
                    "device_code": "12345",
                    "verification_uri": "https://verify/here",
                    "user_code": "abcde",
                    "expires_in": 300
                }"#,
            )
            .unwrap();

        assert_eq!(response.interval, default_devicecode_interval());
    }

    #[test]
    fn test_device_auth_response_non_default_interval() {
        let response: DeviceAuthorizationResponse<EmptyExtraDeviceAuthorizationFields> =
            serde_json::from_str(
                r#"{
                    "device_code": "12345",
                    "verification_uri": "https://verify/here",
                    "user_code": "abcde",
                    "expires_in": 300,
                    "interval": 10
                }"#,
            )
            .unwrap();

        assert_eq!(response.interval, 10);
    }

    #[test]
    fn test_device_auth_response_null_interval() {
        let response: DeviceAuthorizationResponse<EmptyExtraDeviceAuthorizationFields> =
            serde_json::from_str(
                r#"{
                    "device_code": "12345",
                    "verification_uri": "https://verify/here",
                    "user_code": "abcde",
                    "expires_in": 300,
                    "interval": null
                }"#,
            )
            .unwrap();

        assert_eq!(response.interval, default_devicecode_interval());
    }
}
