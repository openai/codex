use crate::basic::{
    BasicClient, BasicErrorResponseType, BasicRevocationErrorResponse, BasicTokenType,
};
use crate::{
    AccessToken, AuthType, AuthUrl, AuthorizationCode, AuthorizationRequest, Client,
    ClientCredentialsTokenRequest, ClientId, ClientSecret, CodeTokenRequest, CsrfToken,
    DeviceAccessTokenRequest, DeviceAuthorizationRequest, DeviceAuthorizationUrl, DeviceCode,
    DeviceCodeErrorResponse, DeviceCodeErrorResponseType, EmptyExtraDeviceAuthorizationFields,
    EmptyExtraTokenFields, EndUserVerificationUrl, EndpointNotSet, EndpointSet, HttpClientError,
    HttpRequest, HttpResponse, PasswordTokenRequest, PkceCodeChallenge, PkceCodeChallengeMethod,
    PkceCodeVerifier, RedirectUrl, RefreshToken, RefreshTokenRequest, RequestTokenError,
    ResourceOwnerPassword, ResourceOwnerUsername, ResponseType, Scope,
    StandardDeviceAuthorizationResponse, StandardErrorResponse, StandardRevocableToken,
    StandardTokenIntrospectionResponse, StandardTokenResponse, TokenUrl, UserCode,
};

use http::header::HeaderName;
use http::HeaderValue;
use thiserror::Error;
use url::Url;

pub(crate) fn new_client(
) -> BasicClient<EndpointSet, EndpointNotSet, EndpointNotSet, EndpointNotSet, EndpointSet> {
    BasicClient::new(ClientId::new("aaa".to_string()))
        .set_auth_uri(AuthUrl::new("https://example.com/auth".to_string()).unwrap())
        .set_token_uri(TokenUrl::new("https://example.com/token".to_string()).unwrap())
        .set_client_secret(ClientSecret::new("bbb".to_string()))
}

pub(crate) fn mock_http_client(
    request_headers: Vec<(HeaderName, &'static str)>,
    request_body: &'static str,
    request_url: Option<Url>,
    response: HttpResponse,
) -> impl Fn(HttpRequest) -> Result<HttpResponse, FakeError> {
    move |request: HttpRequest| {
        assert_eq!(
            &Url::parse(&request.uri().to_string()).unwrap(),
            request_url
                .as_ref()
                .unwrap_or(&Url::parse("https://example.com/token").unwrap())
        );
        assert_eq!(
            request.headers(),
            &request_headers
                .iter()
                .map(|(name, value)| (name.clone(), HeaderValue::from_str(value).unwrap()))
                .collect(),
        );
        assert_eq!(
            &String::from_utf8(request.body().to_owned()).unwrap(),
            request_body
        );

        Ok(response.clone())
    }
}

#[derive(Debug, Error)]
pub(crate) enum FakeError {
    #[error("error")]
    Err,
}

pub(crate) mod colorful_extension {
    extern crate serde_json;

    use crate::{
        Client, ErrorResponseType, ExtraTokenFields, RevocableToken, StandardErrorResponse,
        StandardTokenIntrospectionResponse, StandardTokenResponse, TokenType,
    };

    use serde::{Deserialize, Serialize};

    use std::fmt::Error as FormatterError;
    use std::fmt::{Debug, Display, Formatter};

    pub type ColorfulClient<
        HasAuthUrl,
        HasDeviceAuthUrl,
        HasIntrospectionUrl,
        HasRevocationUrl,
        HasTokenUrl,
    > = Client<
        StandardErrorResponse<ColorfulErrorResponseType>,
        StandardTokenResponse<ColorfulFields, ColorfulTokenType>,
        StandardTokenIntrospectionResponse<ColorfulFields, ColorfulTokenType>,
        ColorfulRevocableToken,
        StandardErrorResponse<ColorfulErrorResponseType>,
        HasAuthUrl,
        HasDeviceAuthUrl,
        HasIntrospectionUrl,
        HasRevocationUrl,
        HasTokenUrl,
    >;

    #[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
    #[serde(rename_all = "lowercase")]
    pub enum ColorfulTokenType {
        Green,
        Red,
    }
    impl TokenType for ColorfulTokenType {}

    #[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
    pub struct ColorfulFields {
        #[serde(rename = "shape")]
        #[serde(skip_serializing_if = "Option::is_none")]
        pub shape: Option<String>,
        #[serde(rename = "height")]
        pub height: u32,
    }
    impl ColorfulFields {
        pub fn shape(&self) -> Option<&String> {
            self.shape.as_ref()
        }
        pub fn height(&self) -> u32 {
            self.height
        }
    }
    impl ExtraTokenFields for ColorfulFields {}

    #[derive(Clone, Deserialize, PartialEq, Eq, Serialize)]
    #[serde(rename_all = "snake_case")]
    pub enum ColorfulErrorResponseType {
        TooDark,
        TooLight,
        WrongColorSpace,
    }

    impl ColorfulErrorResponseType {
        fn to_str(&self) -> &str {
            match self {
                ColorfulErrorResponseType::TooDark => "too_dark",
                ColorfulErrorResponseType::TooLight => "too_light",
                ColorfulErrorResponseType::WrongColorSpace => "wrong_color_space",
            }
        }
    }

    impl ErrorResponseType for ColorfulErrorResponseType {}

    impl Debug for ColorfulErrorResponseType {
        fn fmt(&self, f: &mut Formatter) -> Result<(), FormatterError> {
            Display::fmt(self, f)
        }
    }

    impl Display for ColorfulErrorResponseType {
        fn fmt(&self, f: &mut Formatter) -> Result<(), FormatterError> {
            let message: &str = self.to_str();

            write!(f, "{message}")
        }
    }

    pub type ColorfulTokenResponse = StandardTokenResponse<ColorfulFields, ColorfulTokenType>;

    pub enum ColorfulRevocableToken {
        Red(String),
    }
    impl RevocableToken for ColorfulRevocableToken {
        fn secret(&self) -> &str {
            match self {
                ColorfulRevocableToken::Red(secret) => secret,
            }
        }

        fn type_hint(&self) -> Option<&str> {
            match self {
                ColorfulRevocableToken::Red(_) => Some("red_token"),
            }
        }
    }
}

pub(crate) fn mock_http_client_success_fail(
    request_url: Option<Url>,
    request_headers: Vec<(HeaderName, &'static str)>,
    request_body: &'static str,
    failure_response: HttpResponse,
    num_failures: usize,
    success_response: HttpResponse,
) -> impl Fn(HttpRequest) -> Result<HttpResponse, FakeError> {
    let responses: Vec<HttpResponse> = std::iter::from_fn(|| Some(failure_response.clone()))
        .take(num_failures)
        .chain(std::iter::once(success_response))
        .collect();
    let sync_responses = std::sync::Mutex::new(responses);

    move |request: HttpRequest| {
        assert_eq!(
            &Url::parse(&request.uri().to_string()).unwrap(),
            request_url
                .as_ref()
                .unwrap_or(&Url::parse("https://example.com/token").unwrap())
        );
        assert_eq!(
            request.headers(),
            &request_headers
                .iter()
                .map(|(name, value)| (name.clone(), HeaderValue::from_str(value).unwrap()))
                .collect(),
        );
        assert_eq!(
            &String::from_utf8(request.body().to_owned()).unwrap(),
            request_body
        );

        {
            let mut rsp_vec = sync_responses.lock().unwrap();
            if rsp_vec.len() == 0 {
                Err(FakeError::Err)
            } else {
                Ok(rsp_vec.remove(0))
            }
        }
    }
}

#[test]
fn test_send_sync_impl() {
    fn is_sync_and_send<T: Sync + Send>() {}
    #[derive(Debug)]
    struct TestError;
    impl std::fmt::Display for TestError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "TestError")
        }
    }
    impl std::error::Error for TestError {}

    is_sync_and_send::<AccessToken>();
    is_sync_and_send::<AuthUrl>();
    is_sync_and_send::<AuthorizationCode>();
    is_sync_and_send::<AuthorizationRequest>();
    is_sync_and_send::<
        Client<
            StandardErrorResponse<BasicErrorResponseType>,
            StandardTokenResponse<EmptyExtraTokenFields, BasicTokenType>,
            StandardTokenIntrospectionResponse<EmptyExtraTokenFields, BasicTokenType>,
            StandardRevocableToken,
            BasicRevocationErrorResponse,
            EndpointNotSet,
            EndpointNotSet,
            EndpointNotSet,
            EndpointNotSet,
            EndpointNotSet,
        >,
    >();
    is_sync_and_send::<
        ClientCredentialsTokenRequest<
            StandardErrorResponse<BasicErrorResponseType>,
            StandardTokenResponse<EmptyExtraTokenFields, BasicTokenType>,
        >,
    >();
    is_sync_and_send::<ClientId>();
    is_sync_and_send::<ClientSecret>();
    is_sync_and_send::<
        CodeTokenRequest<
            StandardErrorResponse<BasicErrorResponseType>,
            StandardTokenResponse<EmptyExtraTokenFields, BasicTokenType>,
        >,
    >();
    is_sync_and_send::<CsrfToken>();
    is_sync_and_send::<EmptyExtraTokenFields>();
    is_sync_and_send::<HttpRequest>();
    is_sync_and_send::<HttpResponse>();
    is_sync_and_send::<
        PasswordTokenRequest<
            StandardErrorResponse<BasicErrorResponseType>,
            StandardTokenResponse<EmptyExtraTokenFields, BasicTokenType>,
        >,
    >();
    is_sync_and_send::<PkceCodeChallenge>();
    is_sync_and_send::<PkceCodeChallengeMethod>();
    is_sync_and_send::<PkceCodeVerifier>();
    is_sync_and_send::<RedirectUrl>();
    is_sync_and_send::<RefreshToken>();
    is_sync_and_send::<
        RefreshTokenRequest<
            StandardErrorResponse<BasicErrorResponseType>,
            StandardTokenResponse<EmptyExtraTokenFields, BasicTokenType>,
        >,
    >();
    is_sync_and_send::<ResourceOwnerPassword>();
    is_sync_and_send::<ResourceOwnerUsername>();
    is_sync_and_send::<ResponseType>();
    is_sync_and_send::<Scope>();
    is_sync_and_send::<StandardErrorResponse<BasicErrorResponseType>>();
    is_sync_and_send::<StandardTokenResponse<EmptyExtraTokenFields, BasicTokenType>>();
    is_sync_and_send::<TokenUrl>();

    is_sync_and_send::<AuthType>();
    is_sync_and_send::<BasicErrorResponseType>();
    is_sync_and_send::<BasicTokenType>();
    is_sync_and_send::<RequestTokenError<TestError, StandardErrorResponse<BasicErrorResponseType>>>(
    );

    is_sync_and_send::<DeviceCode>();
    is_sync_and_send::<EndUserVerificationUrl>();
    is_sync_and_send::<UserCode>();
    is_sync_and_send::<DeviceAuthorizationUrl>();
    is_sync_and_send::<StandardDeviceAuthorizationResponse>();
    is_sync_and_send::<
        DeviceAccessTokenRequest<
            StandardTokenResponse<EmptyExtraTokenFields, BasicTokenType>,
            EmptyExtraDeviceAuthorizationFields,
        >,
    >();
    is_sync_and_send::<DeviceAuthorizationRequest<StandardErrorResponse<BasicErrorResponseType>>>();
    is_sync_and_send::<DeviceCodeErrorResponseType>();
    is_sync_and_send::<DeviceCodeErrorResponse>();

    #[cfg(feature = "curl")]
    is_sync_and_send::<HttpClientError<crate::curl::Error>>();
    #[cfg(any(feature = "reqwest", feature = "reqwest-blocking"))]
    is_sync_and_send::<HttpClientError<crate::reqwest::Error>>();
    #[cfg(feature = "ureq")]
    is_sync_and_send::<HttpClientError<crate::ureq::Error>>();
}
