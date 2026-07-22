# codex-http-client

`codex-http-client` is the low-level HTTP transport shared by Codex crates. It is the intended
owner of the workspace's direct `reqwest` integration; product crates should use the types in this
crate instead of constructing `reqwest::Client` values themselves.

Centralizing client construction keeps outbound requests on the same policies and avoids creating
short-lived clients that fragment reqwest's connection pool. In particular, this crate owns:

- the request, response, streaming, and transport types used for outbound HTTP calls;
- custom CA handling through `CODEX_CA_CERTIFICATE` and `SSL_CERT_FILE`;
- explicit outbound proxy policy, including system, PAC/WPAD, environment, and direct routes;
- route-aware client pooling and redirect handling;
- tracing-header injection and optional request diagnostics; and
- the opt-in ChatGPT Cloudflare cookie store.

Another important motivation is consistent support for the `respect_system_proxy` feature. That
feature requires more than enabling reqwest's default proxy behavior: Codex must resolve platform
system settings and PAC/WPAD for each destination, pool connections without mixing routes, and
resolve redirect targets independently.

Higher-level retry, SSE, and request-attempt telemetry policy remains in `codex-client`.

## Outbound proxy policy

Construct one `HttpClientFactory` from the effective application configuration and pass it to the
components that make requests. Call sites should not independently inspect the feature flag or
choose `OutboundProxyPolicy::ReqwestDefault`.

The factory's policy has two modes:

- `RespectSystemProxy` resolves the route for the complete request URL. Platform system settings
  and PAC/WPAD are considered first, followed by explicit proxy environment variables and then a
  direct connection.
- `ReqwestDefault` preserves the transport's legacy proxy behavior. It exists for configurations
  where system-proxy support is disabled, not as a convenient default for new call sites.

These two modes exist because `respect_system_proxy` is currently configurable. If it graduates to
non-configurable built-in behavior, the application-level feature resolution, policy selection,
and most conditional `ReqwestDefault` plumbing can go away. The route-aware implementation would
still be needed: system and PAC decisions can vary by complete URL, redirects can select a
different route, and exceptional direct-routing requirements must remain explicit and auditable.

For a client that talks to one known destination, build it once and retain it:

```rust
use codex_http_client::ClientRouteClass;

let client = http_client_factory.build_client(api_url, ClientRouteClass::Api)?;
let response = client.get(api_url).send().await?;
```

Use `HttpClientBuilder` when the client needs additional shared configuration:

```rust
use codex_http_client::ClientRouteClass;
use codex_http_client::HttpClientBuilder;

let client = HttpClientBuilder::new()
    .default_headers(default_headers)
    .build_respecting_outbound_proxy_policy(
        &http_client_factory,
        api_url,
        ClientRouteClass::Api,
    )?;
```

The terminal method is intentionally explicit. Product traffic should normally use
`build_respecting_outbound_proxy_policy`. `build_direct` is exceptional-use-only and should be
reserved for a documented requirement such as a hermetic local test fixture, localhost callback,
or sandbox traffic whose egress routing is handled separately. The transport-default and
custom-CA-fallback terminal methods are deprecated legacy compatibility paths and must not be used
for new product traffic.

## Route-aware pooling

Use a long-lived `RouteAwareClientPool` when a component can send requests to more than one URL or
follow redirects:

```rust
use codex_http_client::ClientRouteClass;
use codex_http_client::RouteAwareClientPool;

let client_pool =
    RouteAwareClientPool::new(http_client_factory.clone(), ClientRouteClass::Api);
let response = client_pool.get(request_url).send().await?;
```

With `RespectSystemProxy`, proxy selection can depend on the full URL rather than only its origin.
The pool therefore resolves every request URL and caches up to 16 transport clients by resolved
route. This preserves connection reuse without accidentally sending a URL over a client pinned to
the wrong route.

Redirects need the same treatment. Reqwest normally follows them inside one client execution, which
would skip Codex's route selection for the redirect target. In `RespectSystemProxy` mode the pool
follows redirects itself, resolves every hop, and removes sensitive headers when an origin changes.

Do not create a new `HttpClient`, `HttpClientFactory`, or `RouteAwareClientPool` for every request.
Store the client or pool on the component that owns the traffic so its connections can be reused.

## Sensitive request data

Normal clients emit debug diagnostics containing the request URL and response headers. For
endpoints where those values may contain credentials, use
`HttpClientFactory::build_client_without_request_logging` or
`RouteAwareClientPool::new_without_request_logging`. The corresponding ChatGPT cookie-pool
constructor is `with_chatgpt_cloudflare_cookies_without_request_logging`.

The wrapper's `Debug` implementations redact request URLs and resolved proxy settings, but callers
should still avoid putting secrets in URLs whenever possible.

## Adapting to higher-level clients

Code using the transport abstraction should convert a configured wrapper rather than constructing
a raw reqwest client:

```rust
use codex_http_client::ClientRouteClass;
use codex_http_client::ReqwestTransport;

let client = http_client_factory.build_client(api_url, ClientRouteClass::Api)?;
let transport = ReqwestTransport::from_http_client(client);
```

If the existing wrapper surface cannot support a use case, extend `codex-http-client` rather than
adding a direct `reqwest` dependency to another first-party crate.
