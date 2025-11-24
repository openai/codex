use crate::{
    AuthType, ClientId, ClientSecret, ErrorResponse, RedirectUrl, RequestTokenError, Scope,
    CONTENT_TYPE_FORMENCODED, CONTENT_TYPE_JSON,
};

use base64::prelude::*;
use http::header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE};
use http::{HeaderValue, StatusCode};
use serde::de::DeserializeOwned;
use url::{form_urlencoded, Url};

use std::borrow::Cow;
use std::error::Error;
use std::future::Future;

/// An HTTP request.
pub type HttpRequest = http::Request<Vec<u8>>;

/// An HTTP response.
pub type HttpResponse = http::Response<Vec<u8>>;

/// An asynchronous (future-based) HTTP client.
pub trait AsyncHttpClient<'c> {
    /// Error type returned by HTTP client.
    type Error: Error + 'static;

    /// Future type returned by HTTP client.
    type Future: Future<Output = Result<HttpResponse, Self::Error>> + 'c;

    /// Perform a single HTTP request.
    fn call(&'c self, request: HttpRequest) -> Self::Future;
}
impl<'c, E, F, T> AsyncHttpClient<'c> for T
where
    E: Error + 'static,
    F: Future<Output = Result<HttpResponse, E>> + 'c,
    // We can't implement this for FnOnce because the device authorization flow requires clients to
    // supportmultiple calls.
    T: Fn(HttpRequest) -> F,
{
    type Error = E;
    type Future = F;

    fn call(&'c self, request: HttpRequest) -> Self::Future {
        self(request)
    }
}

/// A synchronous (blocking) HTTP client.
pub trait SyncHttpClient {
    /// Error type returned by HTTP client.
    type Error: Error + 'static;

    /// Perform a single HTTP request.
    fn call(&self, request: HttpRequest) -> Result<HttpResponse, Self::Error>;
}
impl<E, T> SyncHttpClient for T
where
    E: Error + 'static,
    // We can't implement this for FnOnce because the device authorization flow requires clients to
    // support multiple calls.
    T: Fn(HttpRequest) -> Result<HttpResponse, E>,
{
    type Error = E;

    fn call(&self, request: HttpRequest) -> Result<HttpResponse, Self::Error> {
        self(request)
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn endpoint_request<'a>(
    auth_type: &'a AuthType,
    client_id: &'a ClientId,
    client_secret: Option<&'a ClientSecret>,
    extra_params: &'a [(Cow<'a, str>, Cow<'a, str>)],
    redirect_url: Option<Cow<'a, RedirectUrl>>,
    scopes: Option<&'a Vec<Cow<'a, Scope>>>,
    url: &'a Url,
    params: Vec<(&'a str, &'a str)>,
) -> Result<HttpRequest, http::Error> {
    let mut builder = http::Request::builder()
        .uri(url.to_string())
        .method(http::Method::POST)
        .header(ACCEPT, HeaderValue::from_static(CONTENT_TYPE_JSON))
        .header(
            CONTENT_TYPE,
            HeaderValue::from_static(CONTENT_TYPE_FORMENCODED),
        );

    let scopes_opt = scopes.and_then(|scopes| {
        if !scopes.is_empty() {
            Some(
                scopes
                    .iter()
                    .map(|s| s.to_string())
                    .collect::<Vec<_>>()
                    .join(" "),
            )
        } else {
            None
        }
    });

    let mut params: Vec<(&str, &str)> = params;
    if let Some(ref scopes) = scopes_opt {
        params.push(("scope", scopes));
    }

    // FIXME: add support for auth extensions? e.g., client_secret_jwt and private_key_jwt
    match (auth_type, client_secret) {
        // Basic auth only makes sense when a client secret is provided. Otherwise, always pass the
        // client ID in the request body.
        (AuthType::BasicAuth, Some(secret)) => {
            // Section 2.3.1 of RFC 6749 requires separately url-encoding the id and secret
            // before using them as HTTP Basic auth username and password. Note that this is
            // not standard for ordinary Basic auth, so curl won't do it for us.
            let urlencoded_id: String =
                form_urlencoded::byte_serialize(client_id.as_bytes()).collect();
            let urlencoded_secret: String =
                form_urlencoded::byte_serialize(secret.secret().as_bytes()).collect();
            let b64_credential =
                BASE64_STANDARD.encode(format!("{}:{}", &urlencoded_id, urlencoded_secret));
            builder = builder.header(
                AUTHORIZATION,
                HeaderValue::from_str(&format!("Basic {}", &b64_credential)).unwrap(),
            );
        }
        (AuthType::RequestBody, _) | (AuthType::BasicAuth, None) => {
            params.push(("client_id", client_id));
            if let Some(client_secret) = client_secret {
                params.push(("client_secret", client_secret.secret()));
            }
        }
    }

    if let Some(ref redirect_url) = redirect_url {
        params.push(("redirect_uri", redirect_url.as_str()));
    }

    params.extend_from_slice(
        extra_params
            .iter()
            .map(|(k, v)| (k.as_ref(), v.as_ref()))
            .collect::<Vec<_>>()
            .as_slice(),
    );

    let body = form_urlencoded::Serializer::new(String::new())
        .extend_pairs(params)
        .finish()
        .into_bytes();

    builder.body(body)
}

pub(crate) fn endpoint_response<RE, TE, DO>(
    http_response: HttpResponse,
) -> Result<DO, RequestTokenError<RE, TE>>
where
    RE: Error,
    TE: ErrorResponse,
    DO: DeserializeOwned,
{
    check_response_status(&http_response)?;

    check_response_body(&http_response)?;

    let response_body = http_response.body().as_slice();
    serde_path_to_error::deserialize(&mut serde_json::Deserializer::from_slice(response_body))
        .map_err(|e| RequestTokenError::Parse(e, response_body.to_vec()))
}

pub(crate) fn endpoint_response_status_only<RE, TE>(
    http_response: HttpResponse,
) -> Result<(), RequestTokenError<RE, TE>>
where
    RE: Error + 'static,
    TE: ErrorResponse,
{
    check_response_status(&http_response)
}

fn check_response_status<RE, TE>(
    http_response: &HttpResponse,
) -> Result<(), RequestTokenError<RE, TE>>
where
    RE: Error + 'static,
    TE: ErrorResponse,
{
    if http_response.status() != StatusCode::OK {
        let reason = http_response.body().as_slice();
        if reason.is_empty() {
            Err(RequestTokenError::Other(
                "server returned empty error response".to_string(),
            ))
        } else {
            let error = match serde_path_to_error::deserialize::<_, TE>(
                &mut serde_json::Deserializer::from_slice(reason),
            ) {
                Ok(error) => RequestTokenError::ServerResponse(error),
                Err(error) => RequestTokenError::Parse(error, reason.to_vec()),
            };
            Err(error)
        }
    } else {
        Ok(())
    }
}

fn check_response_body<RE, TE>(
    http_response: &HttpResponse,
) -> Result<(), RequestTokenError<RE, TE>>
where
    RE: Error + 'static,
    TE: ErrorResponse,
{
    // Validate that the response Content-Type is JSON.
    http_response
    .headers()
    .get(CONTENT_TYPE)
    .map_or(Ok(()), |content_type|
      // Section 3.1.1.1 of RFC 7231 indicates that media types are case-insensitive and
      // may be followed by optional whitespace and/or a parameter (e.g., charset).
      // See https://tools.ietf.org/html/rfc7231#section-3.1.1.1.
      if content_type.to_str().ok().filter(|ct| ct.to_lowercase().starts_with(CONTENT_TYPE_JSON)).is_none() {
        Err(
          RequestTokenError::Other(
            format!(
              "unexpected response Content-Type: {content_type:?}, should be `{CONTENT_TYPE_JSON}`",
            )
          )
        )
      } else {
        Ok(())
      }
    )?;

    if http_response.body().is_empty() {
        return Err(RequestTokenError::Other(
            "server returned empty response body".to_string(),
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::tests::{new_client, FakeError};
    use crate::{AuthorizationCode, TokenResponse};

    use http::{Response, StatusCode};

    #[tokio::test]
    async fn test_async_client_closure() {
        let client = new_client();

        let http_response = Response::builder()
            .status(StatusCode::OK)
            .body(
                "{\"access_token\": \"12/34\", \"token_type\": \"BEARER\"}"
                    .to_string()
                    .into_bytes(),
            )
            .unwrap();

        let token = client
            .exchange_code(AuthorizationCode::new("ccc".to_string()))
            // NB: This tests that the closure doesn't require a static lifetime.
            .request_async(&|_| async { Ok(http_response.clone()) as Result<_, FakeError> })
            .await
            .unwrap();

        assert_eq!("12/34", token.access_token().secret());
    }
}
