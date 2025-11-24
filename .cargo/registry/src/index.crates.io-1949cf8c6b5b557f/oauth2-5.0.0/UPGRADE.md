# Upgrade Guide

## Upgrading from 4.x to 5.x

The 5.0 release includes breaking changes to address several long-standing API issues, along with
a few minor improvements. Consider following the tips below to help ensure a smooth upgrade
process.

### Upgrade Rust to 1.65 or newer

The minimum supported Rust version (MSRV) is now 1.65. Going forward, this crate will maintain a
policy of supporting Rust releases going back at least 6 months. Changes that break compatibility
with Rust releases older than 6 months will no longer be considered SemVer breaking changes and will
not result in a new major version number for this crate. MSRV changes will coincide with minor
version updates and will not happen in patch releases.

### Add typestate generic types to `Client`

Each auth flow depends on one or more server endpoints. For example, the
authorization code flow depends on both an authorization endpoint and a token endpoint, while the
client credentials flow only depends on a token endpoint. Previously, it was possible to instantiate
a `Client` without a token endpoint and then attempt to use an auth flow that required a token
endpoint, leading to errors at runtime. Also, the authorization endpoint was always required, even
for  auth flows that do not use it.

In the 5.0 release, all endpoints are optional.
[Typestates](https://cliffle.com/blog/rust-typestate/) are used to statically track, at compile
time, which endpoints' setters (e.g., `set_auth_uri()`) have been called. Auth flows that depend on
an endpoint cannot be used without first calling the corresponding setter, which is enforced by the
compiler's type checker. This guarantees that certain errors will not arise at runtime.

In addition to unconditional setters (e.g., `set_auth_uri()`), each
endpoint has a corresponding conditional setter (e.g., `set_auth_uri_option()`) that sets a
conditional typestate (`EndpointMaybeSet`). When the conditional typestate is set, endpoints can
be used via fallible methods that return `Err(ConfigurationError::MissingUrl(_))` if an endpoint
has not been set. This is useful in dynamic scenarios such as
[OpenID Connect Discovery](https://openid.net/specs/openid-connect-discovery-1_0.html), in which
it cannot be determined until runtime whether an endpoint is configured.

There are three possible typestates, each implementing the `EndpointState` trait:
* `EndpointNotSet`: the corresponding endpoint has **not** been set and cannot be used.
* `EndpointSet`: the corresponding endpoint **has** been set and is ready to be used.
* `EndpointMaybeSet`: the corresponding endpoint **may have** been set and can be used via fallible
   methods that return `Result<_, ConfigurationError>`.

The following code changes are required to support the new interface:
1. Update calls to
   [`Client::new()`](https://docs.rs/oauth2/latest/oauth2/struct.Client.html#method.new) to use the
   single-argument constructor (which accepts only a `ClientId`). Use the `set_auth_uri()`,
   `set_token_uri()`, and `set_client_secret()` methods to set the authorization endpoint,
   token endpoint, and client secret, respectively, if applicable to your application's auth flows.
2. If required by your usage of the `Client` or `BasicClient` types (i.e., if you see related
   compiler errors), add the following generic parameters:
   ```rust
   HasAuthUrl: EndpointState,
   HasDeviceAuthUrl: EndpointState,
   HasIntrospectionUrl: EndpointState,
   HasRevocationUrl: EndpointState,
   HasTokenUrl: EndpointState,
   ```
   For example, if you store a `BasicClient` within another data type, you may need to annotate it
   as `BasicClient<EndpointSet, EndpointNotSet, EndpointNotSet, EndpointNotSet, EndpointSet>` if it
   has both an authorization endpoint and a
   token endpoint set. Compiler error messages will likely guide you to the appropriate combination
   of typestates.
   
   If, instead of using `BasicClient`, you are directly using `Client` with a different set of type
   parameters, you will need to append the five generic typestate parameters. For example, replace:
   ```rust
   type SpecialClient = Client<
       BasicErrorResponse,
       SpecialTokenResponse,
       BasicTokenType,
       BasicTokenIntrospectionResponse,
       StandardRevocableToken,
       BasicRevocationErrorResponse,
   >;
   ```
   with:
   ```rust
   type SpecialClient<
       HasAuthUrl = EndpointNotSet,
       HasDeviceAuthUrl = EndpointNotSet,
       HasIntrospectionUrl = EndpointNotSet,
       HasRevocationUrl = EndpointNotSet,
       HasTokenUrl = EndpointNotSet,
   > = Client<
       BasicErrorResponse,
       SpecialTokenResponse,
       BasicTokenType,
       BasicTokenIntrospectionResponse,
       StandardRevocableToken,
       BasicRevocationErrorResponse,
       HasAuthUrl,
       HasDeviceAuthUrl,
       HasIntrospectionUrl,
       HasRevocationUrl,
       HasTokenUrl,
   >;
   ```
   The default values (`= EndpointNotSet`) are optional but often helpful since they will allow you
   to instantiate a client using `SpecialClient::new()` instead of having to specify
   `SpecialClient::<EndpointNotSet, EndpointNotSet, EndpointNotSet, EndpointNotSet, EndpointNotSet>::new()`.

### Rename endpoint getters and setters for consistency

The 4.0 release aimed to align the naming of each endpoint with the terminology used in the relevant
RFC. For example, [RFC 6749](https://datatracker.ietf.org/doc/html/rfc6749#section-3.1) uses the
term "endpoint URI" to refer to the authorization and token endpoints, while
[RFC 7009](https://datatracker.ietf.org/doc/html/rfc7009#section-2) refers to the
"token revocation endpoint URL," and
[RFC 7662](https://datatracker.ietf.org/doc/html/rfc7662#section-2) uses neither "URI" nor "URL"
to describe the introspection endpoint. However, the renaming in 4.0 was both internally
inconsistent, and inconsistent with the specs.

In 5.0, the `Client`'s getters and setters for each endpoint are now named as follows:
* Authorization endpoint: `auth_uri()`/`set_auth_uri()`
* Token endpoint: `token_uri()`/`set_token_uri()`
* Redirect: `redirect_uri()`/`set_redirect_uri()` 
* Revocation endpoint: `revocation_url()`/`set_revocation_url()`
* Introspection endpoint: `introspection_url()`/`set_introspection_url()`
* Device authorization endpoint: `device_authorization_url()`/`set_device_authorization_url()`
  (no change)

### Use stateful HTTP clients

Previously, the HTTP clients provided by this crate were stateless. For example, the
`oauth2::reqwest::async_http_client()` method would instantiate a new `reqwest::Client` for each
request. This meant that TCP connections could not be reused across requests, and customizing HTTP
clients (e.g., adding a custom request header to every request) was inconvenient.

The 5.0 release introduces two new traits: `AsyncHttpClient` and `SyncHttpClient`. Each
`request_async()` and `request()` method now accepts a reference to a type that implements these
traits, respectively, rather than a function type.

> [!WARNING]
> To prevent
[SSRF](https://cheatsheetseries.owasp.org/cheatsheets/Server_Side_Request_Forgery_Prevention_Cheat_Sheet.html)
vulnerabilities, be sure to configure the HTTP client **not to follow redirects**. For example, use
> [`redirect::Policy::none`](https://docs.rs/reqwest/latest/reqwest/redirect/struct.Policy.html#method.none)
> when using `reqwest`, or
> [`redirects(0)`](https://docs.rs/ureq/latest/ureq/struct.AgentBuilder.html#method.redirects)
> when using `ureq`.

The `AsyncHttpClient` trait is implemented for the following types:
* `reqwest::Client` (when the default `reqwest` feature is enabled)
* Any function type that implements:
  ```rust
  Fn(HttpRequest) -> F
  where
    E: std::error::Error + 'static,
    F: Future<Output = Result<HttpResponse, E>>,
  ```
  To implement a custom asynchronous HTTP client, either directly implement the `AsyncHttpClient`
  trait, or use a function that implements the signature above.

The `SyncHttpClient` trait is implemented for the following types:
* `reqwest::blocking::Client` (when the `reqwest-blocking` feature is enabled; see below)
* `ureq::Agent` (when the `ureq` feature is enabled)
* `oauth2::CurlHttpClient` (when the `curl` feature is enabled)
* Any function type that implements:
  ```rust
  Fn(HttpRequest) -> Result<HttpResponse, E>
  where
    E: std::error::Error + 'static,
  ```
  To implement a custom synchronous HTTP client, either directly implement the `SyncHttpClient`
  trait, or use a function that implements the signature above.

### Upgrade `http` to 1.0 and `reqwest` to 0.12

The 5.0 release of this crate depends on the new stable [`http`](https://docs.rs/http/latest/http/)
1.0 release, which affects various public interfaces. In particular, `reqwest` has been upgraded
to 0.12, which uses `http` 1.0.

### Enable the `reqwest-blocking` feature to use the synchronous `reqwest` HTTP client

In 4.0, enabling the (default) `reqwest` feature also enabled `reqwest`'s `blocking` feature.
To reduce dependencies and improve compilation speed, the `reqwest` feature now only enables
`reqwest`'s asynchronous (non-blocking) client. To use the synchronous (blocking) client, enable the
`reqwest-blocking` feature in `Cargo.toml`:
```toml
oauth2 = { version = "5", features = ["reqwest-blocking" ] }
```

### Use `http::{Request, Response}` for custom HTTP clients

The `HttpRequest` and `HttpResponse` structs have been replaced with type aliases to
[`http::Request`](https://docs.rs/http/latest/http/request/struct.Request.html) and
[`http::Response`](https://docs.rs/http/latest/http/response/struct.Response.html), respectively.
Custom HTTP clients will need to be updated to use the `http` types. See the
[`reqwest` client implementations](https://github.com/ramosbugs/oauth2-rs/blob/23b952b23e6069525bc7e4c4f2c4924b8d28ce3a/src/reqwest.rs)
for an example.

### Import device code flow and token revocation types from the root

Previously, certain types were exported from both the root of the crate and the `devicecode` or
`revocation` modules. These modules are no longer public, and their public types are exported from
the root. For example, if you were previously importing
`oauth2::devicecode::DeviceAuthorizationResponse`, instead import
`oauth2::DeviceAuthorizationResponse`.

### Replace `TT` generic type parameter in `TokenResponse` with associated type

Previously, the `TokenResponse` and `TokenIntrospectionResponse` traits had a generic type
parameter `TT: TokenType`. This has been replaced with an associated type called `TokenType`.
Uses of `BasicTokenResponse` and `BasicTokenIntrospectionResponse` should continue to work without
changes, but custom implementations of either trait will need to be updated to replace the type
parameter with an associated type.

#### Remove `TT` generic type parameter from `Client` and each `*Request` type

Removing the `TT` generic type parameter from `TokenResponse` (see above) made the `TT` parameters
to `Client` and each `*Request` (e.g., `CodeTokenRequest`) redundant. Consequently, the `TT`
parameter has been removed from each of these types. `BasicClient` should continue to work
without any changes, but code that provides generic types for `Client` or any of the `*Response`
types will need to be updated to remove the `TT` type parameter.

### Add `Display` to `ErrorResponse` trait

To improve error messages, the
[`RequestTokenError::ServerResponse`](https://docs.rs/oauth2/latest/oauth2/enum.RequestTokenError.html#variant.ServerResponse)
enum variant now prints a message describing the server response using the `Display` trait. For most
users (i.e., those using the default
[`StandardErrorResponse`](https://docs.rs/oauth2/latest/oauth2/struct.StandardErrorResponse.html)),
this does not require any code changes. However, users providing their own implementations
of the `ErrorResponse` trait must now implement the `Display` trait. See
`StandardErrorResponse`'s
[`Display` implementation](https://github.com/ramosbugs/oauth2-rs/blob/9d8f11addf819134f15c6d7f03276adb3d32e80b/src/error.rs#L88-L108)
for an example.
