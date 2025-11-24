# Unreleased

# 2.12.1

  * Do not use multi-version deps (>=x.x.x) (#907)

# 2.12.0

  * Bump MSRV 1.67 -> 1.71 because rustls will soon adopt it (#905)
  * Unpin rustls dep (>=0.23.19) (#905)

# 2.11.0

 * Fixes for changes to cargo-deny (#882)
 * Pin rustls dep on 0.23.19 to keep MSRV 1.67 (#878)
 * Bump MSRV 1.63 -> 1.67 due to time crate (#878)
 * Re-export rustls (#813)

# 2.10.1
  * default `ureq` Rustls tls config updated to avoid panic for applications
    that activate the default Rustls `aws-lc-rs` feature without setting
    a process-wide crypto provider. `ureq` will now use `*ring*` in this
    circumstance instead of panicking.

# 2.10.0
  * Bump MSRV 1.61 -> 1.63 due to rustls (#764)
  * Update deps (only patch versions in Cargo.lock) (#763)
  * Refork frewsxcv/rust-chunked-transfer to fix MIT/Apache2.0 license (#761)
  * Enable http-crate feature for docs (#755)
  * Update Rustls from 0.22 to 0.23 - this may be a breaking change if your
    application depends on Rustls 0.22 (e.g. to provide a custom 
    `rustls::ClientConfig` to `ureq`). See the [Rustls 0.23.0][rustls-0.23.0]
    changelog for a list of breaking API changes (#753)
  * Rustls dep to default to ring backend. If your project uses the
    default `ureq` TLS config, or constructs its own `rustls::ClientConfig`
    with `rustls::ClientConfig::builder()` you must ensure the Rustls 
    `aws-lc-rs` feature is not activated, or set the process default 
    cryptography provider before constructing any configs. See the Rustls
    [CryptoProvider][CryptoProvider] docs for more information (#753)
  * Remove direct dep rustls-webpki (#752)
  * Fix doc Rustls does now support IP address certificates (#759)(#753)

[rustls-0.23.0]: https://github.com/rustls/rustls/releases/tag/v%2F0.23.0
[CryptoProvider]: https://docs.rs/rustls/latest/rustls/crypto/struct.CryptoProvider.html#using-the-per-process-default-cryptoprovider

# 2.9.7

  * Update deps (`base64` 0.22, `rustls` to 0.22.4 (#747, #748)
  * Parse URL after middleware to enable changing it (#745)
  * Tidy up code and fix compilation (#742, 743)

# 2.9.6

## Fixed

  * `hootbin` is optional dep. Tests must be run with feature `testdeps` (#729)
  * Exclude script files from cargo package (#728)

# 2.9.5

## Fixed

  * Update deps (`cookie` 0.18, `cookie_store` 0.21, unpin `url`). (#722)

# 2.9.4

## Fixed

  * MSRV 1.61 with CI tests

# 2.9.3

## Fixed

  * docs.rs docs

# 2.9.2

## Added

  * Replace dependency on httpbin.org for tests/doc-tests. (#703)

## Fixed

  * Remove Header struct that never should have been exported. (#696)
  * Update deps (rustls 0.22) (#690)

# 2.9.1

## Fixed

  * Unbreak feature `http-interop`. This feature is version locked to http crate 0.2
  * New feature `http-crate`. This feature is for http crate 1.0
  * New feature `proxy-from-env` to detect proxy settings for global Agent (ureq::get).

# 2.9.0

## Fixed
  * Broken rustls dep (introduced new function in patch version) (#677)
  * Doc and test fixes (#670, #673, #674)

## Added
  * Upgraded http dep to 1.0
  * http_interop to not require utf-8 headers (#672)
  * http_interop implement conversion for `http::request::Parts` (#669)

# 2.8.0

## Fixed
  * Fix regression in IPv6 handling (#635)
  * Read proxy response to \r\n\r\n (#620)

## Added
  * Auto-detect proxy from env vars (turned off by default) (#649)
  * Conversion ureq::Response -> http::Response<Vec<u8>> (#638)
  * cargo-deny CI action to disallow copy-left and duplicate deps (#661)

# 2.7.1

## Fixed
 * Updated serde_json dependency constraint to be >=1.0.97 (#630)

# 2.7.0

## Fixed
 * Pass User-Agent when connecting to proxy (#597)
 * Proxy: Use CONNECT for HTTPS requests, but not HTTP requests (#587)
 * Cookie headers are now cleared on redirect (#608)
 * HTTP/1.0 responses with Content-Length no longer block until server closes
   the socket. (#625)

## Added
 * Conversions to and from http::Response and http::request::Builder (#591)
 * Updated to rustls 0.21 and rustls-webpki, which add support for IP address certificates (#601)
 * Response::local_addr (#605)

# 2.6.2

## Fixed
 * Non-empty connection pools were never dropped (#583)

# 2.6.1

## Fixed
 * gzip: examine Content-Length header before removing (#578)

# 2.6.0

## Added
 * Response::remote_addr() (#489)
 * Request::query_pairs() - make query params from an Iterator of pairs (#519)

## Fixed
 * Gzip responses with chunked encoding now work with connection pooling (#560)
 * Don't panic when rustls-native-certs errors (#564)
 * Responses with zero-length body now work with connection pooling (#565)

# 2.5.0
 * Add tcp no_delay option (#465)
 * Rework public TLS traits
 * Warn if requests aren't sent (#490)
 * Fixes for returning stream to pool (#509)
 * Avoid extra syscalls when content is buffered (#508)
 * Remove dep on sync_wrapper (#514, #528)
 * Error instead of panic on large deadlines (#517)
 * Make ReadWrite trait simpler (used in bespoke TLS impls) (#530)
 * Buffer short response bodies (#531)
 * Update cookie/cookie_store dep

# 2.4.0

 * Enable `gzip` feature by default (#455)
 * `gzip` and `brotli` feature flags to enable decompression (#453, #421)
 * Middleware function on agent (#448)
 * Agent option to preserve `Authorization` header on redirects (#445)
 * Deprecate re-exported serde types (#446)
 * Widen type of `send_json` to `impl Serializable` (#446)
 * `native-tls` feature provides an alternative TLS backend (#449, #391)

# 2.3.2
 * Re-introduce the `ureq::patch` and `agent::patch` calls.
 * Fix regression in 2.3.x for feature native-certs (#441)

# 2.3.1
 * Don't panic when given an invalid DNS name (#436).
 * Update to rustls-native-certs v0.6 (#432).

# 2.3.0
 * Upgrade to rustls 0.20 (#427).
 * Make test mocks of Response more accurate by removing newline (#423).
 * Redact sensitive headers when logging prelude (#414).

# 2.2.0
 * Update to latest dependencies
 * Add SOCKS4 support (#410).
 * Downgrade logging on info level to debug (#409).
 * Bugfix: Clear content-length header on redirect (#394, #395).

# 2.1.1
 * Bugfix: don't reuse conns with bytes pending from server (#372). This
   reduces Transport errors when using an Agent for connection pooling.

# 2.1.0
 * Bugfix: allow status lines without a reason phrase (#316)
 * Example: "cureq" to easier make ad-hoc command line tests (#330)
 * Override timeout per Request (#335)
 * Bugfix: handle non-utf8 status and headers (#347) and better errors (#329)
 * Request inspection (method, url, etc) (#310, #350)
 * Bugfix: stop percent encoding cookies (#353)
 * Enforce cookie RFC naming/value rules (#353)
 * Bugfix: reduce error struct size (#356)

# 2.0.2
 * Bugfix: Apply deadline across redirects. (#313)
 * OrAnyStatus::or_any_status ergonomic helper
 * Allow header lines to end with only LF (#321)

# 2.0.1
 * Fix handling of 308 redirects (port from 1.5.4 branch).
 * Return UnexpectedEof instead of InvalidData on short responses. (#293)
 * Implement std::error::Error for error::Transport. (#299)

# 2.0.0
 * Methods that formerly returned Response now return Result<Response, Error>.
   You'll need to change all instances of `.call()` to `.call()?` or handle
   errors using a `match` statement.
 * Non-2xx responses are considered Error by default. See [Error documentation]
   for details on how to get Response bodies for non-2xx.
 * Rewrite Error type. It's now an enum of two types of error: Status and
   Transport. Status errors (i.e. non-2xx) can be readily turned into a
   Response using match statements.
 * Errors now include the source error (e.g. errors from DNS or I/O) when
   appropriate, as well as the URL that caused an error.
 * The "synthetic error" concept is removed.
 * Move more configuration to Agent. Timeouts, TLS config, and proxy config
   now require building an Agent.
 * Create AgentBuilder to separate the process of building an agent from using
   the resulting agent. Headers can be set on an AgentBuilder, not the
   resulting Agent.
 * Agent is cheaply cloneable with an internal Arc. This makes it easy to
   share a single agent throughout your program.
 * There is now a default timeout_connect of 30 seconds. Read and write
   timeouts continue to be unset by default.
 * Add ureq::request_url and Agent::request_url, to send requests with
   already-parsed URLs.
 * Remove native_tls support.
 * Remove convenience methods `options(url)`, `trace(url)`, and `patch(url)`.
   To send requests with those verbs use `request(method, url)`.
 * Remove Request::build. This was a workaround because some of Request's
   methods took `&mut self` instead of `mut self`, and is no longer needed.
   You can simply delete any calls to `Request::build`.
 * Remove Agent::set_cookie.
 * Remove Header from the public API. The type wasn't used by any public
   methods.
 * Remove basic auth support. The API was incomplete. We may add back something
   better in the future.
 * Remove into_json_deserialize. Now into_json handles both serde_json::Value
   and other types that implement serde::Deserialize. If you were using
   serde_json before, you will probably have to explicitly annotate a type,
   like: `let v: serde_json::Value = response.into_json();`.
 * Rewrite README and top-level documentation.

[Error documentation]: https://docs.rs/ureq/2.0.0-rc4/ureq/enum.Error.html

# 2.0.0-rc4

 * Remove error_on_non_2xx. (#272)
 * Do more validation on status line. (#266)
 * (internal) Add history to response objects (#275)

# 2.0.0-rc3

 * Refactor Error to use an enum for easier extraction of status code errors.
 * (Internal) Use BufRead::read_line when reading headers.

# 2.0.0-rc2
 * These changes are mostly already listed under 2.0.0.
 * Remove the "synthetic error" concept. Methods that formerly returned
   Response now return Result<Response, Error>.
 * Rewrite Error type. Instead of an enum, it's now a struct with an
   ErrorKind. This allows us to store the source error when appropriate,
   as well as the URL that caused an error.
 * Move more configuration to Agent. Timeouts, TLS config, and proxy config
   now require building an Agent.
 * Create AgentBuilder to separate the process of building an agent from using
   the resulting agent. Headers can be set on an AgentBuilder, not the
   resulting Agent.
 * Agent is cheaply cloneable with an internal Arc. This makes it easy to
   share a single agent throughout your program.
 * There is now a default timeout_connect of 30 seconds. Read and write
   timeouts continue to be unset by default.
 * Add ureq::request_url and Agent::request_url, to send requests with
   already-parsed URLs.
 * Remove native_tls support.
 * Remove convenience methods `options(url)`, `trace(url)`, and `patch(url)`.
   To send requests with those verbs use `request(method, url)`.
 * Remove Request::build. This was a workaround because some of Request's
   methods took `&mut self` instead of `mut self`, and is no longer needed.
   You can simply delete any calls to `Request::build`.
 * Remove Agent::set_cookie.
 * Remove Header from the public API. The type wasn't used by any public
   methods.
 * Remove basic auth support. The API was incomplete. We may add back something
   better in the future.
 * Remove into_json_deserialize. Now into_json handles both serde_json::Value
   and other types that implement serde::Deserialize. If you were using
   serde_json before, you will probably have to explicitly annotate a type,
   like: `let v: serde_json::Value = response.into_json();`.
 * Rewrite README and top-level documentation.

# 1.5.2

 * Remove 'static constraint on Request.send(), allowing a wider variety of
   types to be passed. Also eliminate some copying. (#205).
 * Allow turning a Response into an Error (#214).
 * Update env_logger to 0.8.1 (#195).
 * Remove convenience method for CONNECT verb (#177).
 * Fix bugs in handling of timeout_read (#197 and #198).

# 1.5.1

 * Use cookie_store crate for correct cookie handling (#169).
 * Fix bug in picking wrong host for redirects introduced in 1.5.0 (#180).
 * Allow proxy settings on Agent (#178).

# 1.5.0

 * Add pluggable name resolution. Users can now override the IP addresses for
   hostnames of their choice (#148).
 * bugfix: Don't re-pool streams on drop. This would occur if the user called
   `response.into_reader()` and dropped the resulting `Read` before reading all
   the way to EOF. The result would be a BadStatus error on the next request to
   the same hostname. This only affected users using an explicit Agent (#160).
 * Automatically set Transfer-Encoding: chunked when using `send` (#86).
 * `into_reader()` now returns `impl Read + Send` instead of `impl Read` (#156).
 * Add support for log crate (#170).
 * Retry broken connections in more cases (should reduce BadStatus errors; #168).

# 1.4.1

 * Use buffer to avoid byte-by-byte parsing result in multiple syscalls.
 * Allow pooling multiple connections per host.
 * Put version in user agent "ureq/1.4.1".

# 1.4.0

  * Propagate WouldBlock in io:Error for Response::to_json.
  * Merge multiple cookies into one header to be spec compliant.
  * Allow setting TLSConnector for native-tls.
  * Remove brackets against TLS lib when IPv6 addr is used as hostname.
  * Include proxy in connection pool keys.
  * Stop assuming localhost for URLs without host part.
  * Error if body length is less than content-length.
  * Validate header names.

# 1.3.0

  * Changelog start
