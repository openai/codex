# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

# 0.6.6

## Fixed

- compression: fix panic when looking in vary header ([#578])

[#578]: https://github.com/tower-rs/tower-http/pull/578

# 0.6.5

## Added

- normalize_path: add `append_trailing_slash()` mode ([#547])

## Fixed

- redirect: remove payload headers if redirect changes method to GET ([#575])
- compression: avoid setting `vary: accept-encoding` if already set ([#572])

[#547]: https://github.com/tower-rs/tower-http/pull/547
[#572]: https://github.com/tower-rs/tower-http/pull/572
[#575]: https://github.com/tower-rs/tower-http/pull/575

# 0.6.4

## Added

- decompression: Support HTTP responses containing multiple ZSTD frames ([#548])
- The `ServiceExt` trait for chaining layers onto an arbitrary http service just
  like `ServiceBuilderExt` allows for `ServiceBuilder` ([#563])

## Fixed

- Remove unnecessary trait bounds on `S::Error` for `Service` impls of
  `RequestBodyTimeout<S>` and `ResponseBodyTimeout<S>` ([#533])
- compression: Respect `is_end_stream` ([#535])
- Fix a rare panic in `fs::ServeDir` ([#553])
- Fix invalid `content-lenght` of 1 in response to range requests to empty
  files ([#556])
- In `AsyncRequireAuthorization`, use the original inner service after it is
  ready, instead of using a clone ([#561])

[#533]: https://github.com/tower-rs/tower-http/pull/533
[#535]: https://github.com/tower-rs/tower-http/pull/535
[#548]: https://github.com/tower-rs/tower-http/pull/548
[#553]: https://github.com/tower-rs/tower-http/pull/556
[#556]: https://github.com/tower-rs/tower-http/pull/556
[#561]: https://github.com/tower-rs/tower-http/pull/561
[#563]: https://github.com/tower-rs/tower-http/pull/563

# 0.6.3

*This release was yanked because its definition of `ServiceExt` was quite unhelpful, in a way that's very unlikely that anybody would start depending on within the small timeframe before this was yanked, but that was technically breaking to change.*

# 0.6.2

## Changed:

- `CompressionBody<B>` now propagates `B`'s size hint in its `http_body::Body`
  implementation, if compression is disabled ([#531])
  - this allows a `content-length` to be included in an HTTP message with this
    body for those cases

[#531]: https://github.com/tower-rs/tower-http/pull/531

# 0.6.1

## Fixed

- **decompression:** reuse scratch buffer to significantly reduce allocations and improve performance ([#521])

[#521]: https://github.com/tower-rs/tower-http/pull/521

# 0.6.0

## Changed:

- `body` module is disabled except for `catch-panic`, `decompression-*`, `fs`, or `limit` features (BREAKING) ([#477])
- Update to `tower` 0.5 ([#503])

## Fixed

- **fs:** Precompression of static files now supports files without a file extension ([#507])

[#477]: https://github.com/tower-rs/tower-http/pull/477
[#503]: https://github.com/tower-rs/tower-http/pull/503
[#507]: https://github.com/tower-rs/tower-http/pull/507

# 0.5.2

## Added:

- **compression:** Will now send a `vary: accept-encoding` header on compressed responses ([#399])
- **compression:** Support `x-gzip` as equivalent to `gzip` in `accept-encoding` request header ([#467])

## Fixed

- **compression:** Skip compression for range requests ([#446])
- **compression:** Skip compression for SSE responses by default ([#465])
- **cors:** *Actually* keep Vary headers set by the inner service when setting response headers ([#473])
  - Version 0.5.1 intended to ship this, but the implementation was buggy and didn't actually do anything

[#399]: https://github.com/tower-rs/tower-http/pull/399
[#446]: https://github.com/tower-rs/tower-http/pull/446
[#465]: https://github.com/tower-rs/tower-http/pull/465
[#467]: https://github.com/tower-rs/tower-http/pull/467
[#473]: https://github.com/tower-rs/tower-http/pull/473

# 0.5.1 (January 14, 2024)

## Added

- **fs:** Support files precompressed with `zstd` in `ServeFile`
- **trace:** Add default generic parameters for `ResponseBody` and `ResponseFuture` ([#455])
- **trace:** Add type aliases `HttpMakeClassifier` and `GrpcMakeClassifier` ([#455])

## Fixed

- **cors:** Keep Vary headers set by the inner service when setting response headers ([#398])
- **fs:** `ServeDir` now no longer redirects from `/directory` to `/directory/`
  if `append_index_html_on_directories` is disabled ([#421])

[#398]: https://github.com/tower-rs/tower-http/pull/398
[#421]: https://github.com/tower-rs/tower-http/pull/421
[#455]: https://github.com/tower-rs/tower-http/pull/455

# 0.5.0 (November 21, 2023)

## Changed

- Bump Minimum Supported Rust Version to 1.66 ([#433])
- Update to http-body 1.0 ([#348])
- Update to http 1.0 ([#348])
- Preserve service error type in RequestDecompression ([#368])

## Fixed

- Accepts range headers with ranges where the end of range goes past the end of the document by bumping
http-range-header to `0.4`

[#418]: https://github.com/tower-rs/tower-http/pull/418
[#433]: https://github.com/tower-rs/tower-http/pull/433
[#348]: https://github.com/tower-rs/tower-http/pull/348
[#368]: https://github.com/tower-rs/tower-http/pull/368

# 0.4.2 (July 19, 2023)

## Added

- **cors:** Add support for private network preflights ([#373])
- **compression:** Implement `Default` for `DecompressionBody` ([#370])

## Changed

- **compression:** Update to async-compression 0.4 ([#371])

## Fixed

- **compression:** Override default brotli compression level 11 -> 4 ([#356])
- **trace:** Simplify dynamic tracing level application ([#380])
- **normalize_path:** Fix path normalization for preceding slashes ([#359])

[#356]: https://github.com/tower-rs/tower-http/pull/356
[#359]: https://github.com/tower-rs/tower-http/pull/359
[#370]: https://github.com/tower-rs/tower-http/pull/370
[#371]: https://github.com/tower-rs/tower-http/pull/371
[#373]: https://github.com/tower-rs/tower-http/pull/373
[#380]: https://github.com/tower-rs/tower-http/pull/380

# 0.4.1 (June 20, 2023)

## Added

- **request_id:** Derive `Default` for `MakeRequestUuid` ([#335])
- **fs:** Derive `Default` for `ServeFileSystemResponseBody` ([#336])
- **compression:** Expose compression quality on the CompressionLayer ([#333])

## Fixed

- **compression:** Improve parsing of `Accept-Encoding` request header ([#220])
- **normalize_path:** Fix path normalization of index route ([#347])
- **decompression:** Enable `multiple_members` for `GzipDecoder` ([#354])

[#347]: https://github.com/tower-rs/tower-http/pull/347
[#333]: https://github.com/tower-rs/tower-http/pull/333
[#220]: https://github.com/tower-rs/tower-http/pull/220
[#335]: https://github.com/tower-rs/tower-http/pull/335
[#336]: https://github.com/tower-rs/tower-http/pull/336
[#354]: https://github.com/tower-rs/tower-http/pull/354

# 0.4.0 (February 24, 2023)

## Added

- **decompression:** Add `RequestDecompression` middleware ([#282])
- **compression:** Implement `Default` for `CompressionBody` ([#323])
- **compression, decompression:** Support zstd (de)compression ([#322])

## Changed

- **serve_dir:** `ServeDir` and `ServeFile`'s error types are now `Infallible` and any IO errors
  will be converted into responses. Use `try_call` to generate error responses manually (BREAKING) ([#283])
- **serve_dir:** `ServeDir::fallback` and `ServeDir::not_found_service` now requires
  the fallback service to use `Infallible` as its error type (BREAKING) ([#283])
- **compression, decompression:** Tweak prefered compression encodings ([#325])

## Removed

- Removed `RequireAuthorization` in favor of `ValidateRequest` (BREAKING) ([#290])

## Fixed

- **serve_dir:** Don't include identity in Content-Encoding header ([#317])
- **compression:** Do compress SVGs ([#321])
- **serve_dir:** In `ServeDir`, convert `io::ErrorKind::NotADirectory` to `404 Not Found` ([#331])

[#282]: https://github.com/tower-rs/tower-http/pull/282
[#283]: https://github.com/tower-rs/tower-http/pull/283
[#290]: https://github.com/tower-rs/tower-http/pull/290
[#317]: https://github.com/tower-rs/tower-http/pull/317
[#321]: https://github.com/tower-rs/tower-http/pull/321
[#322]: https://github.com/tower-rs/tower-http/pull/322
[#323]: https://github.com/tower-rs/tower-http/pull/323
[#325]: https://github.com/tower-rs/tower-http/pull/325
[#331]: https://github.com/tower-rs/tower-http/pull/331

# 0.3.5 (December 02, 2022)

## Added

- Add `NormalizePath` middleware ([#275])
- Add `ValidateRequest` middleware ([#289])
- Add `RequestBodyTimeout` middleware ([#303])

## Changed

- Bump Minimum Supported Rust Version to 1.60 ([#299])

## Fixed

- **trace:** Correctly identify gRPC requests in default `on_response` callback ([#278])
- **cors:** Panic if a wildcard (`*`) is passed to `AllowOrigin::list`. Use
  `AllowOrigin::any()` instead ([#285])
- **serve_dir:** Call the fallback on non-uft8 request paths ([#310])

[#275]: https://github.com/tower-rs/tower-http/pull/275
[#278]: https://github.com/tower-rs/tower-http/pull/278
[#285]: https://github.com/tower-rs/tower-http/pull/285
[#289]: https://github.com/tower-rs/tower-http/pull/289
[#299]: https://github.com/tower-rs/tower-http/pull/299
[#303]: https://github.com/tower-rs/tower-http/pull/303
[#310]: https://github.com/tower-rs/tower-http/pull/310

# 0.3.4 (June 06, 2022)

## Added

- Add `Timeout` middleware ([#270])
- Add `RequestBodyLimit` middleware ([#271])

[#270]: https://github.com/tower-rs/tower-http/pull/270
[#271]: https://github.com/tower-rs/tower-http/pull/271

# 0.3.3 (May 08, 2022)

## Added

- **serve_dir:** Add `ServeDir::call_fallback_on_method_not_allowed` to allow calling the fallback
  for requests that aren't `GET` or `HEAD` ([#264])
- **request_id:** Add `MakeRequestUuid` for generating request ids using UUIDs ([#266])

[#264]: https://github.com/tower-rs/tower-http/pull/264
[#266]: https://github.com/tower-rs/tower-http/pull/266

## Fixed

- **serve_dir:** Include `Allow` header for `405 Method Not Allowed` responses ([#263])

[#263]: https://github.com/tower-rs/tower-http/pull/263

# 0.3.2 (April 29, 2022)

## Fixed

- **serve_dir**: Fix empty request parts being passed to `ServeDir`'s fallback instead of the actual ones ([#258])

[#258]: https://github.com/tower-rs/tower-http/pull/258

# 0.3.1 (April 28, 2022)

## Fixed

- **cors**: Only send a single origin in `Access-Control-Allow-Origin` header when a list of
  allowed origins is configured (the previous behavior of sending a comma-separated list like for
  allowed methods and allowed headers is not allowed by any standard)

# 0.3.0 (April 25, 2022)

## Added

- **fs**: Add `ServeDir::{fallback, not_found_service}` for calling another service if
  the file cannot be found ([#243])
- **fs**: Add `SetStatus` to override status codes ([#248])
- `ServeDir` and `ServeFile` now respond with `405 Method Not Allowed` to requests where the
  method isn't `GET` or `HEAD` ([#249])
- **cors**: Added `CorsLayer::very_permissive` which is like
  `CorsLayer::permissive` except it (truly) allows credentials. This is made
  possible by mirroring the request's origin as well as method and headers
  back as CORS-whitelisted ones ([#237])
- **cors**: Allow customizing the value(s) for the `Vary` header ([#237])

## Changed

- **cors**: Removed `allow-credentials: true` from `CorsLayer::permissive`.
  It never actually took effect in compliant browsers because it is mutually
  exclusive with the `*` wildcard (`Any`) on origins, methods and headers ([#237])
- **cors**: Rewrote the CORS middleware. Almost all existing usage patterns
  will continue to work. (BREAKING) ([#237])
- **cors**: The CORS middleware will now panic if you try to use `Any` in
  combination with `.allow_credentials(true)`. This configuration worked
  before, but resulted in browsers ignoring the `allow-credentials` header,
  which defeats the purpose of setting it and can be very annoying to debug
  ([#237])

## Fixed

- **fs**: Fix content-length calculation on range requests ([#228])

[#228]: https://github.com/tower-rs/tower-http/pull/228
[#237]: https://github.com/tower-rs/tower-http/pull/237
[#243]: https://github.com/tower-rs/tower-http/pull/243
[#248]: https://github.com/tower-rs/tower-http/pull/248
[#249]: https://github.com/tower-rs/tower-http/pull/249

# 0.2.4 (March 5, 2022)

## Added

- Added `CatchPanic` middleware which catches panics and converts them
  into `500 Internal Server` responses ([#214])

## Fixed

- Make parsing of `Accept-Encoding` more robust ([#220])

[#214]: https://github.com/tower-rs/tower-http/pull/214
[#220]: https://github.com/tower-rs/tower-http/pull/220

# 0.2.3 (February 18, 2022)

## Changed

- Update to tokio-util 0.7 ([#221])

## Fixed

- The CORS layer / service methods `allow_headers`, `allow_methods`, `allow_origin`
  and `expose_headers` now do nothing if given an empty `Vec`, instead of sending
  the respective header with an empty value ([#218])

[#218]: https://github.com/tower-rs/tower-http/pull/218
[#221]: https://github.com/tower-rs/tower-http/pull/221

# 0.2.2 (February 8, 2022)

## Fixed

- Add `Vary` headers for CORS preflight responses ([#216])

[#216]: https://github.com/tower-rs/tower-http/pull/216

# 0.2.1 (January 21, 2022)

## Added

- Support `Last-Modified` (and friends) headers in `ServeDir` and `ServeFile` ([#145])
- Add `AsyncRequireAuthorization::layer` ([#195])

## Fixed

- Fix build error for certain feature sets ([#209])
- `Cors`: Set `Vary` header ([#199])
- `ServeDir` and `ServeFile`: Fix potential directory traversal attack due to
  improper path validation on Windows ([#204])

[#145]: https://github.com/tower-rs/tower-http/pull/145
[#195]: https://github.com/tower-rs/tower-http/pull/195
[#199]: https://github.com/tower-rs/tower-http/pull/199
[#204]: https://github.com/tower-rs/tower-http/pull/204
[#209]: https://github.com/tower-rs/tower-http/pull/209

# 0.2.0 (December 1, 2021)

## Added

- **builder**: Add `ServiceBuilderExt` which adds methods to `tower::ServiceBuilder` for
  adding middleware from tower-http ([#106])
- **request_id**: Add `SetRequestId` and `PropagateRequestId` middleware ([#150])
- **trace**: Add `DefaultMakeSpan::level` to make log level of tracing spans easily configurable ([#124])
- **trace**: Add `LatencyUnit::Seconds` for formatting latencies as seconds ([#179])
- **trace**: Support customizing which status codes are considered failures by `GrpcErrorsAsFailures` ([#189])
- **compression**: Support specifying predicates to choose when responses should
  be compressed. This can be used to disable compression of small responses,
  responses with a certain `content-type`, or something user defined ([#172])
- **fs**: Ability to serve precompressed files ([#156])
- **fs**: Support `Range` requests ([#173])
- **fs**: Properly support HEAD requests which return no body and have the `Content-Length` header set ([#169])

## Changed

- `AddAuthorization`, `InFlightRequests`, `SetRequestHeader`,
  `SetResponseHeader`, `AddExtension`, `MapRequestBody` and `MapResponseBody`
   now requires underlying service to use `http::Request<ReqBody>` and
   `http::Response<ResBody>` as request and responses ([#182]) (BREAKING)
- **set_header**: Remove unnecessary generic parameter from `SetRequestHeaderLayer`
  and `SetResponseHeaderLayer`. This removes the need (and possibility) to specify a
  body type for these layers ([#148]) (BREAKING)
- **compression, decompression**: Change the response body error type to
  `Box<dyn std::error::Error + Send + Sync>`. This makes them usable if
  the body they're wrapping uses `Box<dyn std::error::Error + Send + Sync>` as
  its error type which they previously weren't ([#166]) (BREAKING)
- **fs**: Change response body type of `ServeDir` and `ServeFile` to
  `ServeFileSystemResponseBody` and `ServeFileSystemResponseFuture` ([#187]) (BREAKING)
- **auth**: Change `AuthorizeRequest` and `AsyncAuthorizeRequest` traits to be simpler ([#192]) (BREAKING)

## Removed

- **compression, decompression**: Remove `BodyOrIoError`. Its been replaced with `Box<dyn
  std::error::Error + Send + Sync>` ([#166]) (BREAKING)
- **compression, decompression**: Remove the `compression` and `decompression` feature. They were unnecessary
  and `compression-full`/`decompression-full` can be used to get full
  compression/decompression support. For more granular control, `[compression|decompression]-gzip`,
  `[compression|decompression]-br` and `[compression|decompression]-deflate` may
  be used instead ([#170]) (BREAKING)

[#106]: https://github.com/tower-rs/tower-http/pull/106
[#124]: https://github.com/tower-rs/tower-http/pull/124
[#148]: https://github.com/tower-rs/tower-http/pull/148
[#150]: https://github.com/tower-rs/tower-http/pull/150
[#156]: https://github.com/tower-rs/tower-http/pull/156
[#166]: https://github.com/tower-rs/tower-http/pull/166
[#169]: https://github.com/tower-rs/tower-http/pull/169
[#170]: https://github.com/tower-rs/tower-http/pull/170
[#172]: https://github.com/tower-rs/tower-http/pull/172
[#173]: https://github.com/tower-rs/tower-http/pull/173
[#179]: https://github.com/tower-rs/tower-http/pull/179
[#182]: https://github.com/tower-rs/tower-http/pull/182
[#187]: https://github.com/tower-rs/tower-http/pull/187
[#189]: https://github.com/tower-rs/tower-http/pull/189
[#192]: https://github.com/tower-rs/tower-http/pull/192

# 0.1.2 (November 13, 2021)

- New middleware: Add `Cors` for setting [CORS] headers ([#112])
- New middleware: Add `AsyncRequireAuthorization` ([#118])
- `Compression`: Don't recompress HTTP responses ([#140])
- `Compression` and `Decompression`: Pass configuration from layer into middleware ([#132])
- `ServeDir` and `ServeFile`: Improve performance ([#137])
- `Compression`: Remove needless `ResBody::Error: Into<BoxError>` bounds ([#117])
- `ServeDir`: Percent decode path segments ([#129])
- `ServeDir`: Use correct redirection status ([#130])
- `ServeDir`: Return `404 Not Found` on requests to directories if
  `append_index_html_on_directories` is set to `false` ([#122])

[#112]: https://github.com/tower-rs/tower-http/pull/112
[#118]: https://github.com/tower-rs/tower-http/pull/118
[#140]: https://github.com/tower-rs/tower-http/pull/140
[#132]: https://github.com/tower-rs/tower-http/pull/132
[#137]: https://github.com/tower-rs/tower-http/pull/137
[#117]: https://github.com/tower-rs/tower-http/pull/117
[#129]: https://github.com/tower-rs/tower-http/pull/129
[#130]: https://github.com/tower-rs/tower-http/pull/130
[#122]: https://github.com/tower-rs/tower-http/pull/122

# 0.1.1 (July 2, 2021)

- Add example of using `SharedClassifier`.
- Add `StatusInRangeAsFailures` which is a response classifier that considers
  responses with status code in a certain range as failures. Useful for HTTP
  clients where both server errors (5xx) and client errors (4xx) are considered
  failures.
- Implement `Debug` for `NeverClassifyEos`.
- Update iri-string to 0.4.
- Add `ClassifyResponse::map_failure_class` and `ClassifyEos::map_failure_class`
  for transforming the failure classification using a function.
- Clarify exactly when each `Trace` callback is called.
- Add `AddAuthorizationLayer` for setting the `Authorization` header on
  requests.

# 0.1.0 (May 27, 2021)

- Initial release.

[CORS]: https://developer.mozilla.org/en-US/docs/Web/HTTP/CORS
