use crate::basic::{
    BasicClient, BasicErrorResponse, BasicErrorResponseType, BasicTokenResponse, BasicTokenType,
};
use crate::tests::colorful_extension::{
    ColorfulClient, ColorfulErrorResponseType, ColorfulFields, ColorfulTokenResponse,
    ColorfulTokenType,
};
use crate::tests::{mock_http_client, new_client, FakeError};
use crate::token::tests::custom_errors::CustomErrorClient;
use crate::{
    AccessToken, AuthType, AuthUrl, AuthorizationCode, ClientId, ClientSecret, ExtraTokenFields,
    PkceCodeVerifier, RedirectUrl, RefreshToken, RequestTokenError, ResourceOwnerPassword,
    ResourceOwnerUsername, Scope, StandardErrorResponse, StandardTokenResponse, TokenResponse,
    TokenType, TokenUrl,
};

use http::header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE};
use http::{HeaderValue, Response, StatusCode};

use std::borrow::Cow;
use std::time::Duration;

// Because the secret types don't implement PartialEq, we can't directly use == to compare tokens.
fn assert_token_eq<EF, TT>(a: &StandardTokenResponse<EF, TT>, b: &StandardTokenResponse<EF, TT>)
where
    EF: ExtraTokenFields + PartialEq,
    TT: TokenType,
{
    assert_eq!(a.access_token().secret(), b.access_token().secret());
    assert_eq!(a.token_type(), b.token_type());
    assert_eq!(a.expires_in(), b.expires_in());
    assert_eq!(
        a.refresh_token().map(RefreshToken::secret),
        b.refresh_token().map(RefreshToken::secret)
    );
    assert_eq!(a.scopes(), b.scopes());
    assert_eq!(a.extra_fields(), b.extra_fields());
}

#[test]
fn test_exchange_code_successful_with_minimal_json_response() {
    let client = BasicClient::new(ClientId::new("aaa".to_string()))
        .set_client_secret(ClientSecret::new("bbb".to_string()))
        .set_auth_uri(AuthUrl::new("https://example.com/auth".to_string()).unwrap())
        .set_token_uri(TokenUrl::new("https://example.com/token".to_string()).unwrap());

    let token = client
        .exchange_code(AuthorizationCode::new("ccc".to_string()))
        .request(&mock_http_client(
            vec![
                (ACCEPT, "application/json"),
                (CONTENT_TYPE, "application/x-www-form-urlencoded"),
                (AUTHORIZATION, "Basic YWFhOmJiYg=="),
            ],
            "grant_type=authorization_code&code=ccc",
            None,
            Response::builder()
                .status(StatusCode::OK)
                .body(
                    "{\"access_token\": \"12/34\", \"token_type\": \"BEARER\"}"
                        .to_string()
                        .into_bytes(),
                )
                .unwrap(),
        ))
        .unwrap();

    assert_eq!("12/34", token.access_token().secret());
    assert_eq!(BasicTokenType::Bearer, *token.token_type());
    assert_eq!(None, token.expires_in());
    assert!(token.refresh_token().is_none());

    // Ensure that serialization produces an equivalent JSON value.
    let serialized_json = serde_json::to_string(&token).unwrap();
    assert_eq!(
        "{\"access_token\":\"12/34\",\"token_type\":\"bearer\"}".to_string(),
        serialized_json
    );

    let deserialized_token = serde_json::from_str::<BasicTokenResponse>(&serialized_json).unwrap();
    assert_token_eq(&token, &deserialized_token);
}

#[test]
fn test_exchange_code_successful_with_complete_json_response() {
    let client = new_client().set_auth_type(AuthType::RequestBody);
    let token = client
        .exchange_code(AuthorizationCode::new("ccc".to_string()))
        .request(&mock_http_client(
            vec![
                (ACCEPT, "application/json"),
                (CONTENT_TYPE, "application/x-www-form-urlencoded"),
            ],
            "grant_type=authorization_code&code=ccc&client_id=aaa&client_secret=bbb",
            None,
            Response::builder()
                .status(StatusCode::OK)
                .header(
                    CONTENT_TYPE,
                    HeaderValue::from_str("application/json").unwrap(),
                )
                .body(
                    "{\
                       \"access_token\": \"12/34\", \
                       \"token_type\": \"bearer\", \
                       \"scope\": \"read write\", \
                       \"expires_in\": 3600, \
                       \"refresh_token\": \"foobar\"\
                       }"
                    .to_string()
                    .into_bytes(),
                )
                .unwrap(),
        ))
        .unwrap();

    assert_eq!("12/34", token.access_token().secret());
    assert_eq!(BasicTokenType::Bearer, *token.token_type());
    assert_eq!(
        Some(&vec![
            Scope::new("read".to_string()),
            Scope::new("write".to_string()),
        ]),
        token.scopes()
    );
    assert_eq!(3600, token.expires_in().unwrap().as_secs());
    assert_eq!("foobar", token.refresh_token().unwrap().secret());

    // Ensure that serialization produces an equivalent JSON value.
    let serialized_json = serde_json::to_string(&token).unwrap();
    assert_eq!(
        "{\"access_token\":\"12/34\",\"token_type\":\"bearer\",\"expires_in\":3600,\
         \"refresh_token\":\"foobar\",\"scope\":\"read write\"}"
            .to_string(),
        serialized_json
    );

    let deserialized_token = serde_json::from_str::<BasicTokenResponse>(&serialized_json).unwrap();
    assert_token_eq(&token, &deserialized_token);
}

#[test]
fn test_exchange_client_credentials_with_basic_auth() {
    let client = BasicClient::new(ClientId::new("aaa/;&".to_string()))
        .set_client_secret(ClientSecret::new("bbb/;&".to_string()))
        .set_auth_uri(AuthUrl::new("https://example.com/auth".to_string()).unwrap())
        .set_token_uri(TokenUrl::new("https://example.com/token".to_string()).unwrap())
        .set_auth_type(AuthType::BasicAuth);

    let token = client
        .exchange_client_credentials()
        .request(&mock_http_client(
            vec![
                (ACCEPT, "application/json"),
                (CONTENT_TYPE, "application/x-www-form-urlencoded"),
                (AUTHORIZATION, "Basic YWFhJTJGJTNCJTI2OmJiYiUyRiUzQiUyNg=="),
            ],
            "grant_type=client_credentials",
            None,
            Response::builder()
                .status(StatusCode::OK)
                .body(
                    "{\
                       \"access_token\": \"12/34\", \
                       \"token_type\": \"bearer\", \
                       \"scope\": \"read write\"\
                       }"
                    .to_string()
                    .into_bytes(),
                )
                .unwrap(),
        ))
        .unwrap();

    assert_eq!("12/34", token.access_token().secret());
    assert_eq!(BasicTokenType::Bearer, *token.token_type());
    assert_eq!(
        Some(&vec![
            Scope::new("read".to_string()),
            Scope::new("write".to_string()),
        ]),
        token.scopes()
    );
    assert_eq!(None, token.expires_in());
    assert!(token.refresh_token().is_none());
}

#[test]
fn test_exchange_client_credentials_with_basic_auth_but_no_client_secret() {
    let client = BasicClient::new(ClientId::new("aaa/;&".to_string()))
        .set_auth_uri(AuthUrl::new("https://example.com/auth".to_string()).unwrap())
        .set_token_uri(TokenUrl::new("https://example.com/token".to_string()).unwrap())
        .set_auth_type(AuthType::BasicAuth);

    let token = client
        .exchange_client_credentials()
        .request(&mock_http_client(
            vec![
                (ACCEPT, "application/json"),
                (CONTENT_TYPE, "application/x-www-form-urlencoded"),
            ],
            "grant_type=client_credentials&client_id=aaa%2F%3B%26",
            None,
            Response::builder()
                .status(StatusCode::OK)
                .body(
                    "{\
                       \"access_token\": \"12/34\", \
                       \"token_type\": \"bearer\", \
                       \"scope\": \"read write\"\
                       }"
                    .to_string()
                    .into_bytes(),
                )
                .unwrap(),
        ))
        .unwrap();

    assert_eq!("12/34", token.access_token().secret());
    assert_eq!(BasicTokenType::Bearer, *token.token_type());
    assert_eq!(
        Some(&vec![
            Scope::new("read".to_string()),
            Scope::new("write".to_string()),
        ]),
        token.scopes()
    );
    assert_eq!(None, token.expires_in());
    assert!(token.refresh_token().is_none());
}

#[test]
fn test_exchange_client_credentials_with_body_auth_and_scope() {
    let client = new_client().set_auth_type(AuthType::RequestBody);
    let token = client
        .exchange_client_credentials()
        .add_scope(Scope::new("read".to_string()))
        .add_scope(Scope::new("write".to_string()))
        .request(&mock_http_client(
            vec![
                (ACCEPT, "application/json"),
                (CONTENT_TYPE, "application/x-www-form-urlencoded"),
            ],
            "grant_type=client_credentials&scope=read+write&client_id=aaa&client_secret=bbb",
            None,
            Response::builder()
                .status(StatusCode::OK)
                .header(
                    CONTENT_TYPE,
                    HeaderValue::from_str("APPLICATION/jSoN").unwrap(),
                )
                .body(
                    "{\
                       \"access_token\": \"12/34\", \
                       \"token_type\": \"bearer\", \
                       \"scope\": \"read write\"\
                       }"
                    .to_string()
                    .into_bytes(),
                )
                .unwrap(),
        ))
        .unwrap();

    assert_eq!("12/34", token.access_token().secret());
    assert_eq!(BasicTokenType::Bearer, *token.token_type());
    assert_eq!(
        Some(&vec![
            Scope::new("read".to_string()),
            Scope::new("write".to_string()),
        ]),
        token.scopes()
    );
    assert_eq!(None, token.expires_in());
    assert!(token.refresh_token().is_none());
}

#[test]
fn test_exchange_refresh_token_with_basic_auth() {
    let client = new_client().set_auth_type(AuthType::BasicAuth);
    let token = client
        .exchange_refresh_token(&RefreshToken::new("ccc".to_string()))
        .request(&mock_http_client(
            vec![
                (ACCEPT, "application/json"),
                (CONTENT_TYPE, "application/x-www-form-urlencoded"),
                (AUTHORIZATION, "Basic YWFhOmJiYg=="),
            ],
            "grant_type=refresh_token&refresh_token=ccc",
            None,
            Response::builder()
                .status(StatusCode::OK)
                .body(
                    "{\"access_token\": \"12/34\", \
                       \"token_type\": \"bearer\", \
                       \"scope\": \"read write\"\
                       }"
                    .to_string()
                    .into_bytes(),
                )
                .unwrap(),
        ))
        .unwrap();

    assert_eq!("12/34", token.access_token().secret());
    assert_eq!(BasicTokenType::Bearer, *token.token_type());
    assert_eq!(
        Some(&vec![
            Scope::new("read".to_string()),
            Scope::new("write".to_string()),
        ]),
        token.scopes()
    );
    assert_eq!(None, token.expires_in());
    assert!(token.refresh_token().is_none());
}

#[test]
fn test_exchange_refresh_token_with_json_response() {
    let client = new_client();
    let token = client
        .exchange_refresh_token(&RefreshToken::new("ccc".to_string()))
        .request(&mock_http_client(
            vec![
                (ACCEPT, "application/json"),
                (CONTENT_TYPE, "application/x-www-form-urlencoded"),
                (AUTHORIZATION, "Basic YWFhOmJiYg=="),
            ],
            "grant_type=refresh_token&refresh_token=ccc",
            None,
            Response::builder()
                .status(StatusCode::OK)
                .body(
                    "{\
                       \"access_token\": \"12/34\", \
                       \"token_type\": \"bearer\", \
                       \"scope\": \"read write\"\
                       }"
                    .to_string()
                    .into_bytes(),
                )
                .unwrap(),
        ))
        .unwrap();

    assert_eq!("12/34", token.access_token().secret());
    assert_eq!(BasicTokenType::Bearer, *token.token_type());
    assert_eq!(
        Some(&vec![
            Scope::new("read".to_string()),
            Scope::new("write".to_string()),
        ]),
        token.scopes()
    );
    assert_eq!(None, token.expires_in());
    assert!(token.refresh_token().is_none());
}

#[test]
fn test_exchange_password_with_json_response() {
    let client = new_client();
    let token = client
        .exchange_password(
            &ResourceOwnerUsername::new("user".to_string()),
            &ResourceOwnerPassword::new("pass".to_string()),
        )
        .add_scope(Scope::new("read".to_string()))
        .add_scope(Scope::new("write".to_string()))
        .request(&mock_http_client(
            vec![
                (ACCEPT, "application/json"),
                (CONTENT_TYPE, "application/x-www-form-urlencoded"),
                (AUTHORIZATION, "Basic YWFhOmJiYg=="),
            ],
            "grant_type=password&username=user&password=pass&scope=read+write",
            None,
            Response::builder()
                .status(StatusCode::OK)
                .header(
                    CONTENT_TYPE,
                    HeaderValue::from_str("application/json").unwrap(),
                )
                .body(
                    "{\
                       \"access_token\": \"12/34\", \
                       \"token_type\": \"bearer\", \
                       \"scope\": \"read write\"\
                       }"
                    .to_string()
                    .into_bytes(),
                )
                .unwrap(),
        ))
        .unwrap();

    assert_eq!("12/34", token.access_token().secret());
    assert_eq!(BasicTokenType::Bearer, *token.token_type());
    assert_eq!(
        Some(&vec![
            Scope::new("read".to_string()),
            Scope::new("write".to_string()),
        ]),
        token.scopes()
    );
    assert_eq!(None, token.expires_in());
    assert!(token.refresh_token().is_none());
}

#[test]
fn test_exchange_code_successful_with_redirect_url() {
    let client = new_client()
        .set_auth_type(AuthType::RequestBody)
        .set_redirect_uri(RedirectUrl::new("https://redirect/here".to_string()).unwrap());

    let token = client
        .exchange_code(AuthorizationCode::new("ccc".to_string()))
        .request(&mock_http_client(
            vec![
                (ACCEPT, "application/json"),
                (CONTENT_TYPE, "application/x-www-form-urlencoded"),
            ],
            "grant_type=authorization_code&code=ccc&client_id=aaa&client_secret=bbb&\
             redirect_uri=https%3A%2F%2Fredirect%2Fhere",
            None,
            Response::builder()
                .status(StatusCode::OK)
                .header(
                    CONTENT_TYPE,
                    HeaderValue::from_str("application/json").unwrap(),
                )
                .body(
                    "{\
                       \"access_token\": \"12/34\", \
                       \"token_type\": \"bearer\", \
                       \"scope\": \"read write\"\
                       }"
                    .to_string()
                    .into_bytes(),
                )
                .unwrap(),
        ))
        .unwrap();

    assert_eq!("12/34", token.access_token().secret());
    assert_eq!(BasicTokenType::Bearer, *token.token_type());
    assert_eq!(
        Some(&vec![
            Scope::new("read".to_string()),
            Scope::new("write".to_string()),
        ]),
        token.scopes()
    );
    assert_eq!(None, token.expires_in());
    assert!(token.refresh_token().is_none());
}

#[test]
fn test_exchange_code_successful_with_redirect_url_override() {
    let client = new_client()
        .set_auth_type(AuthType::RequestBody)
        .set_redirect_uri(RedirectUrl::new("https://redirect/here".to_string()).unwrap());

    let token = client
        .exchange_code(AuthorizationCode::new("ccc".to_string()))
        .set_redirect_uri(Cow::Owned(
            RedirectUrl::new("https://redirect/alternative".to_string()).unwrap(),
        ))
        .request(&mock_http_client(
            vec![
                (ACCEPT, "application/json"),
                (CONTENT_TYPE, "application/x-www-form-urlencoded"),
            ],
            "grant_type=authorization_code&code=ccc&client_id=aaa&client_secret=bbb&\
             redirect_uri=https%3A%2F%2Fredirect%2Falternative",
            None,
            Response::builder()
                .status(StatusCode::OK)
                .header(
                    CONTENT_TYPE,
                    HeaderValue::from_str("application/json").unwrap(),
                )
                .body(
                    "{\
                       \"access_token\": \"12/34\", \
                       \"token_type\": \"bearer\", \
                       \"scope\": \"read write\"\
                       }"
                    .to_string()
                    .into_bytes(),
                )
                .unwrap(),
        ))
        .unwrap();

    assert_eq!("12/34", token.access_token().secret());
    assert_eq!(BasicTokenType::Bearer, *token.token_type());
    assert_eq!(
        Some(&vec![
            Scope::new("read".to_string()),
            Scope::new("write".to_string()),
        ]),
        token.scopes()
    );
    assert_eq!(None, token.expires_in());
    assert!(token.refresh_token().is_none());
}

#[test]
fn test_exchange_code_successful_with_basic_auth() {
    let client = new_client()
        .set_auth_type(AuthType::BasicAuth)
        .set_redirect_uri(RedirectUrl::new("https://redirect/here".to_string()).unwrap());

    let token = client
        .exchange_code(AuthorizationCode::new("ccc".to_string()))
        .request(&mock_http_client(
            vec![
                (ACCEPT, "application/json"),
                (CONTENT_TYPE, "application/x-www-form-urlencoded"),
                (AUTHORIZATION, "Basic YWFhOmJiYg=="),
            ],
            "grant_type=authorization_code&code=ccc&redirect_uri=https%3A%2F%2Fredirect%2Fhere",
            None,
            Response::builder()
                .status(StatusCode::OK)
                .header(
                    CONTENT_TYPE,
                    HeaderValue::from_str("application/json").unwrap(),
                )
                .body(
                    "{\
                       \"access_token\": \"12/34\", \
                       \"token_type\": \"bearer\", \
                       \"scope\": \"read write\"\
                       }"
                    .to_string()
                    .into_bytes(),
                )
                .unwrap(),
        ))
        .unwrap();

    assert_eq!("12/34", token.access_token().secret());
    assert_eq!(BasicTokenType::Bearer, *token.token_type());
    assert_eq!(
        Some(&vec![
            Scope::new("read".to_string()),
            Scope::new("write".to_string()),
        ]),
        token.scopes()
    );
    assert_eq!(None, token.expires_in());
    assert!(token.refresh_token().is_none());
}

#[test]
fn test_exchange_code_successful_with_pkce_and_extension() {
    let client = new_client()
        .set_auth_type(AuthType::BasicAuth)
        .set_redirect_uri(RedirectUrl::new("https://redirect/here".to_string()).unwrap());

    let token = client
        .exchange_code(AuthorizationCode::new("ccc".to_string()))
        .set_pkce_verifier(PkceCodeVerifier::new(
            "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk".to_string(),
        ))
        .add_extra_param("foo", "bar")
        .request(&mock_http_client(
            vec![
                (ACCEPT, "application/json"),
                (CONTENT_TYPE, "application/x-www-form-urlencoded"),
                (AUTHORIZATION, "Basic YWFhOmJiYg=="),
            ],
            "grant_type=authorization_code\
             &code=ccc\
             &code_verifier=dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk\
             &redirect_uri=https%3A%2F%2Fredirect%2Fhere\
             &foo=bar",
            None,
            Response::builder()
                .status(StatusCode::OK)
                .header(
                    CONTENT_TYPE,
                    HeaderValue::from_str("application/json").unwrap(),
                )
                .body(
                    "{\
                       \"access_token\": \"12/34\", \
                       \"token_type\": \"bearer\", \
                       \"scope\": \"read write\"\
                       }"
                    .to_string()
                    .into_bytes(),
                )
                .unwrap(),
        ))
        .unwrap();

    assert_eq!("12/34", token.access_token().secret());
    assert_eq!(BasicTokenType::Bearer, *token.token_type());
    assert_eq!(
        Some(&vec![
            Scope::new("read".to_string()),
            Scope::new("write".to_string()),
        ]),
        token.scopes()
    );
    assert_eq!(None, token.expires_in());
    assert!(token.refresh_token().is_none());
}

#[test]
fn test_exchange_refresh_token_successful_with_extension() {
    let client = new_client()
        .set_auth_type(AuthType::BasicAuth)
        .set_redirect_uri(RedirectUrl::new("https://redirect/here".to_string()).unwrap());

    let token = client
        .exchange_refresh_token(&RefreshToken::new("ccc".to_string()))
        .add_extra_param("foo", "bar")
        .request(&mock_http_client(
            vec![
                (ACCEPT, "application/json"),
                (CONTENT_TYPE, "application/x-www-form-urlencoded"),
                (AUTHORIZATION, "Basic YWFhOmJiYg=="),
            ],
            "grant_type=refresh_token&refresh_token=ccc&foo=bar",
            None,
            Response::builder()
                .status(StatusCode::OK)
                .header(
                    CONTENT_TYPE,
                    HeaderValue::from_str("application/json").unwrap(),
                )
                .body(
                    "{\
                       \"access_token\": \"12/34\", \
                       \"token_type\": \"bearer\", \
                       \"scope\": \"read write\"\
                       }"
                    .to_string()
                    .into_bytes(),
                )
                .unwrap(),
        ))
        .unwrap();

    assert_eq!("12/34", token.access_token().secret());
    assert_eq!(BasicTokenType::Bearer, *token.token_type());
    assert_eq!(
        Some(&vec![
            Scope::new("read".to_string()),
            Scope::new("write".to_string()),
        ]),
        token.scopes()
    );
    assert_eq!(None, token.expires_in());
    assert!(token.refresh_token().is_none());
}

#[test]
fn test_exchange_code_with_simple_json_error() {
    let client = new_client();
    let token = client
        .exchange_code(AuthorizationCode::new("ccc".to_string()))
        .request(&mock_http_client(
            vec![
                (ACCEPT, "application/json"),
                (CONTENT_TYPE, "application/x-www-form-urlencoded"),
                (AUTHORIZATION, "Basic YWFhOmJiYg=="),
            ],
            "grant_type=authorization_code&code=ccc",
            None,
            Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .header(
                    CONTENT_TYPE,
                    HeaderValue::from_str("application/json").unwrap(),
                )
                .body(
                    "{\
                       \"error\": \"invalid_request\", \
                       \"error_description\": \"stuff happened\"\
                       }"
                    .to_string()
                    .into_bytes(),
                )
                .unwrap(),
        ));

    assert!(token.is_err());

    let token_err = token.err().unwrap();
    match token_err {
        RequestTokenError::ServerResponse(ref error_response) => {
            assert_eq!(
                BasicErrorResponseType::InvalidRequest,
                *error_response.error()
            );
            assert_eq!(
                Some(&"stuff happened".to_string()),
                error_response.error_description()
            );
            assert_eq!(None, error_response.error_uri());

            // Test Debug trait for ErrorResponse
            assert_eq!(
                "StandardErrorResponse { error: invalid_request, \
                 error_description: Some(\"stuff happened\"), error_uri: None }",
                format!("{error_response:?}")
            );
            // Test Display trait for ErrorResponse
            assert_eq!(
                "invalid_request: stuff happened",
                format!("{error_response}")
            );

            // Test Debug trait for BasicErrorResponseType
            assert_eq!("invalid_request", format!("{:?}", error_response.error()));
            // Test Display trait for BasicErrorResponseType
            assert_eq!("invalid_request", format!("{}", error_response.error()));

            // Ensure that serialization produces an equivalent JSON value.
            let serialized_json = serde_json::to_string(&error_response).unwrap();
            assert_eq!(
                "{\"error\":\"invalid_request\",\"error_description\":\"stuff happened\"}"
                    .to_string(),
                serialized_json
            );

            let deserialized_error =
                serde_json::from_str::<BasicErrorResponse>(&serialized_json).unwrap();
            assert_eq!(error_response, &deserialized_error);
        }
        other => panic!("Unexpected error: {other:?}"),
    }

    // Test Debug trait for RequestTokenError
    assert_eq!(
        "ServerResponse(StandardErrorResponse { error: invalid_request, \
         error_description: Some(\"stuff happened\"), error_uri: None })",
        format!("{token_err:?}")
    );
    // Test Display trait for RequestTokenError
    assert_eq!(
        "Server returned error response: invalid_request: stuff happened",
        token_err.to_string()
    );
}

#[test]
fn test_exchange_code_with_json_parse_error() {
    let client = new_client();
    let token = client
        .exchange_code(AuthorizationCode::new("ccc".to_string()))
        .request(&mock_http_client(
            vec![
                (ACCEPT, "application/json"),
                (CONTENT_TYPE, "application/x-www-form-urlencoded"),
                (AUTHORIZATION, "Basic YWFhOmJiYg=="),
            ],
            "grant_type=authorization_code&code=ccc",
            None,
            Response::builder()
                .status(StatusCode::OK)
                .header(
                    CONTENT_TYPE,
                    HeaderValue::from_str("application/json").unwrap(),
                )
                .body("broken json".to_string().into_bytes())
                .unwrap(),
        ));

    assert!(token.is_err());

    match token.err().unwrap() {
        RequestTokenError::Parse(json_err, _) => {
            assert_eq!(".", json_err.path().to_string());
            assert_eq!(1, json_err.inner().line());
            assert_eq!(1, json_err.inner().column());
            assert_eq!(
                serde_json::error::Category::Syntax,
                json_err.inner().classify()
            );
        }
        other => panic!("Unexpected error: {other:?}"),
    }
}

#[test]
fn test_exchange_code_with_unexpected_content_type() {
    let client = new_client();
    let token = client
        .exchange_code(AuthorizationCode::new("ccc".to_string()))
        .request(&mock_http_client(
            vec![
                (ACCEPT, "application/json"),
                (CONTENT_TYPE, "application/x-www-form-urlencoded"),
                (AUTHORIZATION, "Basic YWFhOmJiYg=="),
            ],
            "grant_type=authorization_code&code=ccc",
            None,
            Response::builder()
                .status(StatusCode::OK)
                .header(CONTENT_TYPE, HeaderValue::from_str("text/plain").unwrap())
                .body("broken json".to_string().into_bytes())
                .unwrap(),
        ));

    assert!(token.is_err());

    match token.err().unwrap() {
        RequestTokenError::Other(error_str) => {
            assert_eq!(
                "unexpected response Content-Type: \"text/plain\", should be `application/json`",
                error_str
            );
        }
        other => panic!("Unexpected error: {other:?}"),
    }
}

#[test]
fn test_exchange_code_with_invalid_token_type() {
    let client = BasicClient::new(ClientId::new("aaa".to_string()))
        .set_auth_uri(AuthUrl::new("https://example.com/auth".to_string()).unwrap())
        .set_token_uri(TokenUrl::new("https://example.com/token".to_string()).unwrap());

    let token = client
        .exchange_code(AuthorizationCode::new("ccc".to_string()))
        .request(&mock_http_client(
            vec![
                (ACCEPT, "application/json"),
                (CONTENT_TYPE, "application/x-www-form-urlencoded"),
            ],
            "grant_type=authorization_code&code=ccc&client_id=aaa",
            None,
            Response::builder()
                .status(StatusCode::OK)
                .header(
                    CONTENT_TYPE,
                    HeaderValue::from_str("application/json").unwrap(),
                )
                .body(
                    "{\"access_token\": \"12/34\", \"token_type\": 123}"
                        .to_string()
                        .into_bytes(),
                )
                .unwrap(),
        ));

    assert!(token.is_err());
    match token.err().unwrap() {
        RequestTokenError::Parse(json_err, _) => {
            assert_eq!("token_type", json_err.path().to_string());
            assert_eq!(1, json_err.inner().line());
            assert_eq!(43, json_err.inner().column());
            assert_eq!(
                serde_json::error::Category::Data,
                json_err.inner().classify()
            );
        }
        other => panic!("Unexpected error: {other:?}"),
    }
}

#[test]
fn test_exchange_code_with_400_status_code() {
    let body = r#"{"error":"invalid_request","error_description":"Expired code."}"#;
    let client = new_client();
    let token_err = client
        .exchange_code(AuthorizationCode::new("ccc".to_string()))
        .request(&mock_http_client(
            vec![
                (ACCEPT, "application/json"),
                (CONTENT_TYPE, "application/x-www-form-urlencoded"),
                (AUTHORIZATION, "Basic YWFhOmJiYg=="),
            ],
            "grant_type=authorization_code&code=ccc",
            None,
            Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .header(
                    CONTENT_TYPE,
                    HeaderValue::from_str("application/json").unwrap(),
                )
                .body(body.to_string().into_bytes())
                .unwrap(),
        ))
        .err()
        .unwrap();

    match token_err {
        RequestTokenError::ServerResponse(ref error_response) => {
            assert_eq!(
                BasicErrorResponseType::InvalidRequest,
                *error_response.error()
            );
            assert_eq!(
                Some(&"Expired code.".to_string()),
                error_response.error_description()
            );
            assert_eq!(None, error_response.error_uri());
        }
        other => panic!("Unexpected error: {other:?}"),
    }

    assert_eq!(
        "Server returned error response: invalid_request: Expired code.",
        token_err.to_string(),
    );
}

#[test]
fn test_exchange_code_fails_gracefully_on_transport_error() {
    let client = BasicClient::new(ClientId::new("aaa".to_string()))
        .set_client_secret(ClientSecret::new("bbb".to_string()))
        .set_auth_uri(AuthUrl::new("https://auth".to_string()).unwrap())
        .set_token_uri(TokenUrl::new("https://token".to_string()).unwrap());

    let token = client
        .exchange_code(AuthorizationCode::new("ccc".to_string()))
        .request(&|_| Err(FakeError::Err));

    assert!(token.is_err());

    match token.err().unwrap() {
        RequestTokenError::Request(FakeError::Err) => (),
        other => panic!("Unexpected error: {other:?}"),
    }
}

#[test]
fn test_extension_successful_with_minimal_json_response() {
    let client = ColorfulClient::new(ClientId::new("aaa".to_string()))
        .set_client_secret(ClientSecret::new("bbb".to_string()))
        .set_auth_uri(AuthUrl::new("https://example.com/auth".to_string()).unwrap())
        .set_token_uri(TokenUrl::new("https://example.com/token".to_string()).unwrap());

    let token = client
        .exchange_code(AuthorizationCode::new("ccc".to_string()))
        .request(&mock_http_client(
            vec![
                (ACCEPT, "application/json"),
                (CONTENT_TYPE, "application/x-www-form-urlencoded"),
                (AUTHORIZATION, "Basic YWFhOmJiYg=="),
            ],
            "grant_type=authorization_code&code=ccc",
            None,
            Response::builder()
                .status(StatusCode::OK)
                .header(
                    CONTENT_TYPE,
                    HeaderValue::from_str("application/json").unwrap(),
                )
                .body(
                    "{\"access_token\": \"12/34\", \"token_type\": \"green\", \"height\": 10}"
                        .to_string()
                        .into_bytes(),
                )
                .unwrap(),
        ))
        .unwrap();

    assert_eq!("12/34", token.access_token().secret());
    assert_eq!(ColorfulTokenType::Green, *token.token_type());
    assert_eq!(None, token.expires_in());
    assert!(token.refresh_token().is_none());
    assert_eq!(None, token.extra_fields().shape());
    assert_eq!(10, token.extra_fields().height());

    // Ensure that serialization produces an equivalent JSON value.
    let serialized_json = serde_json::to_string(&token).unwrap();
    assert_eq!(
        "{\"access_token\":\"12/34\",\"token_type\":\"green\",\"height\":10}".to_string(),
        serialized_json
    );

    let deserialized_token =
        serde_json::from_str::<ColorfulTokenResponse>(&serialized_json).unwrap();
    assert_token_eq(&token, &deserialized_token);
}

#[test]
fn test_extension_successful_with_complete_json_response() {
    let client = ColorfulClient::new(ClientId::new("aaa".to_string()))
        .set_client_secret(ClientSecret::new("bbb".to_string()))
        .set_auth_uri(AuthUrl::new("https://example.com/auth".to_string()).unwrap())
        .set_token_uri(TokenUrl::new("https://example.com/token".to_string()).unwrap())
        .set_auth_type(AuthType::RequestBody);

    let token = client
        .exchange_code(AuthorizationCode::new("ccc".to_string()))
        .request(&mock_http_client(
            vec![
                (ACCEPT, "application/json"),
                (CONTENT_TYPE, "application/x-www-form-urlencoded"),
            ],
            "grant_type=authorization_code&code=ccc&client_id=aaa&client_secret=bbb",
            None,
            Response::builder()
                .status(StatusCode::OK)
                .header(
                    CONTENT_TYPE,
                    HeaderValue::from_str("application/json").unwrap(),
                )
                .body(
                    "{\
                       \"access_token\": \"12/34\", \
                       \"token_type\": \"red\", \
                       \"scope\": \"read write\", \
                       \"expires_in\": 3600, \
                       \"refresh_token\": \"foobar\", \
                       \"shape\": \"round\", \
                       \"height\": 12\
                       }"
                    .to_string()
                    .into_bytes(),
                )
                .unwrap(),
        ))
        .unwrap();

    assert_eq!("12/34", token.access_token().secret());
    assert_eq!(ColorfulTokenType::Red, *token.token_type());
    assert_eq!(
        Some(&vec![
            Scope::new("read".to_string()),
            Scope::new("write".to_string()),
        ]),
        token.scopes()
    );
    assert_eq!(3600, token.expires_in().unwrap().as_secs());
    assert_eq!("foobar", token.refresh_token().unwrap().secret());
    assert_eq!(Some(&"round".to_string()), token.extra_fields().shape());
    assert_eq!(12, token.extra_fields().height());

    // Ensure that serialization produces an equivalent JSON value.
    let serialized_json = serde_json::to_string(&token).unwrap();
    assert_eq!(
        "{\"access_token\":\"12/34\",\"token_type\":\"red\",\"expires_in\":3600,\
         \"refresh_token\":\"foobar\",\"scope\":\"read write\",\"shape\":\"round\",\"height\":12}"
            .to_string(),
        serialized_json
    );

    let deserialized_token =
        serde_json::from_str::<ColorfulTokenResponse>(&serialized_json).unwrap();
    assert_token_eq(&token, &deserialized_token);
}

#[test]
fn test_extension_with_simple_json_error() {
    let client = ColorfulClient::new(ClientId::new("aaa".to_string()))
        .set_client_secret(ClientSecret::new("bbb".to_string()))
        .set_auth_uri(AuthUrl::new("https://example.com/auth".to_string()).unwrap())
        .set_token_uri(TokenUrl::new("https://example.com/token".to_string()).unwrap());

    let token = client
        .exchange_code(AuthorizationCode::new("ccc".to_string()))
        .request(&mock_http_client(
            vec![
                (ACCEPT, "application/json"),
                (CONTENT_TYPE, "application/x-www-form-urlencoded"),
                (AUTHORIZATION, "Basic YWFhOmJiYg=="),
            ],
            "grant_type=authorization_code&code=ccc",
            None,
            Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .header(
                    CONTENT_TYPE,
                    HeaderValue::from_str("application/json").unwrap(),
                )
                .body(
                    "{\"error\": \"too_light\", \"error_description\": \"stuff happened\", \
                       \"error_uri\": \"https://errors\"}"
                        .to_string()
                        .into_bytes(),
                )
                .unwrap(),
        ));

    assert!(token.is_err());

    let token_err = token.err().unwrap();
    match token_err {
        RequestTokenError::ServerResponse(ref error_response) => {
            assert_eq!(ColorfulErrorResponseType::TooLight, *error_response.error());
            assert_eq!(
                Some(&"stuff happened".to_string()),
                error_response.error_description()
            );
            assert_eq!(
                Some(&"https://errors".to_string()),
                error_response.error_uri()
            );

            // Ensure that serialization produces an equivalent JSON value.
            let serialized_json = serde_json::to_string(&error_response).unwrap();
            assert_eq!(
                "{\"error\":\"too_light\",\"error_description\":\"stuff happened\",\
                 \"error_uri\":\"https://errors\"}"
                    .to_string(),
                serialized_json
            );

            let deserialized_error = serde_json::from_str::<
                StandardErrorResponse<ColorfulErrorResponseType>,
            >(&serialized_json)
            .unwrap();
            assert_eq!(error_response, &deserialized_error);
        }
        other => panic!("Unexpected error: {other:?}"),
    }

    // Test Debug trait for RequestTokenError
    assert_eq!(
        "ServerResponse(StandardErrorResponse { error: too_light, \
         error_description: Some(\"stuff happened\"), error_uri: Some(\"https://errors\") })",
        format!("{token_err:?}")
    );
    // Test Display trait for RequestTokenError
    assert_eq!(
        "Server returned error response: too_light: stuff happened (see https://errors)",
        token_err.to_string()
    );
}

mod custom_errors {
    use crate::tests::colorful_extension::{
        ColorfulFields, ColorfulRevocableToken, ColorfulTokenType,
    };
    use crate::{Client, ErrorResponse, StandardTokenIntrospectionResponse, StandardTokenResponse};

    use serde::{Deserialize, Serialize};

    use std::fmt::Error as FormatterError;
    use std::fmt::{Display, Formatter};

    extern crate serde_json;

    #[derive(Serialize, Deserialize, Debug)]
    pub struct CustomErrorResponse {
        pub custom_error: String,
    }

    impl Display for CustomErrorResponse {
        fn fmt(&self, f: &mut Formatter) -> Result<(), FormatterError> {
            write!(f, "Custom Error from server")
        }
    }

    impl ErrorResponse for CustomErrorResponse {}

    pub type CustomErrorClient<
        HasAuthUrl,
        HasDeviceAuthUrl,
        HasIntrospectionUrl,
        HasRevocationUrl,
        HasTokenUrl,
    > = Client<
        CustomErrorResponse,
        StandardTokenResponse<ColorfulFields, ColorfulTokenType>,
        StandardTokenIntrospectionResponse<ColorfulFields, ColorfulTokenType>,
        ColorfulRevocableToken,
        CustomErrorResponse,
        HasAuthUrl,
        HasDeviceAuthUrl,
        HasIntrospectionUrl,
        HasRevocationUrl,
        HasTokenUrl,
    >;
}

#[test]
fn test_extension_with_custom_json_error() {
    let client = CustomErrorClient::new(ClientId::new("aaa".to_string()))
        .set_client_secret(ClientSecret::new("bbb".to_string()))
        .set_auth_uri(AuthUrl::new("https://example.com/auth".to_string()).unwrap())
        .set_token_uri(TokenUrl::new("https://example.com/token".to_string()).unwrap());

    let token = client
        .exchange_code(AuthorizationCode::new("ccc".to_string()))
        .request(&mock_http_client(
            vec![
                (ACCEPT, "application/json"),
                (CONTENT_TYPE, "application/x-www-form-urlencoded"),
                (AUTHORIZATION, "Basic YWFhOmJiYg=="),
            ],
            "grant_type=authorization_code&code=ccc",
            None,
            Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .header(
                    CONTENT_TYPE,
                    HeaderValue::from_str("application/json").unwrap(),
                )
                .body(
                    "{\"custom_error\": \"non-compliant oauth implementation ;-)\"}"
                        .to_string()
                        .into_bytes(),
                )
                .unwrap(),
        ));

    assert!(token.is_err());

    match token.err().unwrap() {
        RequestTokenError::ServerResponse(e) => {
            assert_eq!("non-compliant oauth implementation ;-)", e.custom_error)
        }
        e => panic!("failed to correctly parse custom server error, got {e:?}"),
    };
}

#[test]
fn test_extension_serializer() {
    let mut token_response = ColorfulTokenResponse::new(
        AccessToken::new("mysecret".to_string()),
        ColorfulTokenType::Red,
        ColorfulFields {
            shape: Some("circle".to_string()),
            height: 10,
        },
    );
    token_response.set_expires_in(Some(&Duration::from_secs(3600)));
    token_response.set_refresh_token(Some(RefreshToken::new("myothersecret".to_string())));
    let serialized = serde_json::to_string(&token_response).unwrap();
    assert_eq!(
        "{\
         \"access_token\":\"mysecret\",\
         \"token_type\":\"red\",\
         \"expires_in\":3600,\
         \"refresh_token\":\"myothersecret\",\
         \"shape\":\"circle\",\
         \"height\":10\
         }",
        serialized,
    );
}
