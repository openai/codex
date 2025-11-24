# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

# 0.8.4

- **added:** `Router::reset_fallback` ([#3320])
- **added:** `WebSocketUpgrade::selected_protocol` ([#3248])
- **fixed:** Panic location for overlapping method routes ([#3319])
- **fixed:** Don't leak a tokio task when using `serve` without graceful shutdown ([#3129])

[#3319]: https://github.com/tokio-rs/axum/pull/3319
[#3320]: https://github.com/tokio-rs/axum/pull/3320
[#3248]: https://github.com/tokio-rs/axum/pull/3248
[#3129]: https://github.com/tokio-rs/axum/pull/3129

# 0.8.3

- **added:** Implement `From<Bytes>` for `Message` ([#3273])
- **added:** Implement `OptionalFromRequest` for `Json` ([#3142])
- **added:** Implement `OptionalFromRequest` for `Extension` ([#3157])
- **added:** Allow setting the read buffer capacity of `WebSocketUpgrade` ([#3178])
- **changed:** Improved code size / compile time of dependent crates ([#3285], [#3294])

[#3273]: https://github.com/tokio-rs/axum/pull/3273
[#3142]: https://github.com/tokio-rs/axum/pull/3142
[#3157]: https://github.com/tokio-rs/axum/pull/3157
[#3178]: https://github.com/tokio-rs/axum/pull/3178
[#3285]: https://github.com/tokio-rs/axum/pull/3285
[#3294]: https://github.com/tokio-rs/axum/pull/3294

# 0.8.2

Yanked from crates.io due to unforeseen breaking change, see [#3190] for details.

[#3190]: https://github.com/tokio-rs/axum/pull/3190

# 0.8.1

- **fixed:** Removed the warning about breaking changes from README

# 0.8.0

## since rc.1

<details>

- **breaking:** `axum::extract::ws::Message` now uses `Bytes` in place of `Vec<u8>`,
  and a new `Utf8Bytes` type in place of `String`, for its variants ([#3078])
- **breaking:** Remove `OptionalFromRequestParts` impl for `Query` ([#3088])
- **changed:** Upgraded `tokio-tungstenite` to 0.26 ([#3078])
- **changed:** Query/Form: Use `serde_path_to_error` to report fields that failed to parse ([#3081])

[#3088]: https://github.com/tokio-rs/axum/pull/3088

</details>

## full changelog

*Note: there are further relevant changes in [axum-core's changelog][core-changelog]*

- **breaking:** Upgrade matchit to 0.8, changing the path parameter syntax from `/:single` and `/*many`
  to `/{single}` and `/{*many}`; the old syntax produces a panic to avoid silent change in behavior ([#2645])
- **breaking:** Require `Sync` for all handlers and services added to `Router`
  and `MethodRouter` ([#2473])
- **breaking:** The tuple and tuple_struct `Path` extractor deserializers now check that the number of parameters matches the tuple length exactly ([#2931])
- **breaking:** Move `Host` extractor to `axum-extra` ([#2956])
- **breaking:** Remove `WebSocket::close`.
  Users should explicitly send close messages themselves. ([#2974])
- **breaking:** Make `serve` generic over the listener and IO types ([#2941])
- **breaking:** Remove `Serve::tcp_nodelay` and `WithGracefulShutdown::tcp_nodelay`.
  See `serve::ListenerExt` for an API that let you set arbitrary TCP stream properties. ([#2941])
- **breaking:** `Option<Path<T>>` no longer swallows all error conditions,
  instead rejecting the request in many cases; see its documentation for details ([#2475])
- **breaking:** `axum::extract::ws::Message` now uses `Bytes` in place of `Vec<u8>`,
  and a new `Utf8Bytes` type in place of `String`, for its variants ([#3078])
- **fixed:** Skip SSE incompatible chars of `serde_json::RawValue` in `Event::json_data` ([#2992])
- **fixed:** Don't panic when array type is used for path segment ([#3039])
- **fixed:** Avoid setting `content-length` before middleware.
  This allows middleware to add bodies to requests without needing to manually set `content-length` ([#2897])
- **change:** Update minimum rust version to 1.75 ([#2943])
- **changed:** Upgraded `tokio-tungstenite` to 0.26 ([#3078])
- **changed:** Query/Form: Use `serde_path_to_error` to report fields that failed to parse ([#3081])
- **added:** Add `method_not_allowed_fallback` to set a fallback when a path matches but there is no handler for the given HTTP method ([#2903])
- **added:** Add `NoContent` as a self-described shortcut for `StatusCode::NO_CONTENT` ([#2978])
- **added:** Add support for WebSockets over HTTP/2.
  They can be enabled by changing `get(ws_endpoint)` handlers to `any(ws_endpoint)` ([#2894])
- **added:** Add `MethodFilter::CONNECT`, `routing::connect[_service]`
  and `MethodRouter::connect[_service]` ([#2961])
- **added:** Extend `FailedToDeserializePathParams::kind` enum with (`ErrorKind::DeserializeError`)
  This new variant captures both `key`, `value`, and `message` from named path parameters parse errors,
  instead of only deserialization error message in `ErrorKind::Message`. ([#2720])

[#3078]: https://github.com/tokio-rs/axum/pull/3078
[#3081]: https://github.com/tokio-rs/axum/pull/3081

## rc.1

*Note: there are further relevant changes in [axum-core's changelog][core-changelog]*

- **breaking:** Move `Host` extractor to `axum-extra` ([#2956])
- **breaking:** Remove `WebSocket::close`.
  Users should explicitly send close messages themselves. ([#2974])
- **breaking:** Make `serve` generic over the listener and IO types ([#2941])
- **breaking:** Remove `Serve::tcp_nodelay` and `WithGracefulShutdown::tcp_nodelay`.
  See `serve::ListenerExt` for an API that let you set arbitrary TCP stream properties. ([#2941])
- **breaking:** `Option<Path<T>>` and `Option<Query<T>>` no longer swallow all error conditions,
  instead rejecting the request in many cases; see their documentation for details ([#2475])
- **fixed:** Skip SSE incompatible chars of `serde_json::RawValue` in `Event::json_data` ([#2992])
- **fixed:** Don't panic when array type is used for path segment ([#3039])
- **fixed:** Avoid setting `content-length` before middleware.
  This allows middleware to add bodies to requests without needing to manually set `content-length` ([#2897])
- **added:** Add `method_not_allowed_fallback` to set a fallback when a path matches but there is no handler for the given HTTP method ([#2903])
- **added:** Add `NoContent` as a self-described shortcut for `StatusCode::NO_CONTENT` ([#2978])
- **added:** Add support for WebSockets over HTTP/2.
  They can be enabled by changing `get(ws_endpoint)` handlers to `any(ws_endpoint)` ([#2894])
- **added:** Add `MethodFilter::CONNECT`, `routing::connect[_service]`
  and `MethodRouter::connect[_service]` ([#2961])
- **added:** Extend `FailedToDeserializePathParams::kind` enum with (`ErrorKind::DeserializeError`)
  This new variant captures both `key`, `value`, and `message` from named path parameters parse errors,
  instead of only deserialization error message in `ErrorKind::Message`. ([#2720])

[core-changelog]: ../axum-core/CHANGELOG.md
[#2475]: https://github.com/tokio-rs/axum/pull/2475
[#2897]: https://github.com/tokio-rs/axum/pull/2897
[#2903]: https://github.com/tokio-rs/axum/pull/2903
[#2894]: https://github.com/tokio-rs/axum/pull/2894
[#2956]: https://github.com/tokio-rs/axum/pull/2956
[#2961]: https://github.com/tokio-rs/axum/pull/2961
[#2974]: https://github.com/tokio-rs/axum/pull/2974
[#2978]: https://github.com/tokio-rs/axum/pull/2978
[#2992]: https://github.com/tokio-rs/axum/pull/2992
[#2720]: https://github.com/tokio-rs/axum/pull/2720
[#3039]: https://github.com/tokio-rs/axum/pull/3039
[#2941]: https://github.com/tokio-rs/axum/pull/2941

## alpha.1

- **breaking:** Require `Sync` for all handlers and services added to `Router`
  and `MethodRouter` ([#2473])
- **breaking:** The tuple and tuple_struct `Path` extractor deserializers now check that the number of parameters matches the tuple length exactly ([#2931])
- **breaking:** Upgrade matchit to 0.8, changing the path parameter syntax from `/:single` and `/*many`
  to `/{single}` and `/{*many}`; the old syntax produces a panic to avoid silent change in behavior ([#2645])
- **change:** Update minimum rust version to 1.75 ([#2943])

[#2473]: https://github.com/tokio-rs/axum/pull/2473
[#2645]: https://github.com/tokio-rs/axum/pull/2645
[#2931]: https://github.com/tokio-rs/axum/pull/2931
[#2943]: https://github.com/tokio-rs/axum/pull/2943

# 0.7.9

- **fixed:** Avoid setting content-length before middleware ([#3031])

[#3031]:https://github.com/tokio-rs/axum/pull/3031

# 0.7.8

- **fixed:** Skip SSE incompatible chars of `serde_json::RawValue` in `Event::json_data` ([#2992])
- **added:** Add `method_not_allowed_fallback` to set a fallback when a path matches but there is no handler for the given HTTP method ([#2903])
- **added:** Add `MethodFilter::CONNECT`, `routing::connect[_service]`
  and `MethodRouter::connect[_service]` ([#2961])
- **added:** Add `NoContent` as a self-described shortcut for `StatusCode::NO_CONTENT` ([#2978])

[#2903]: https://github.com/tokio-rs/axum/pull/2903
[#2961]: https://github.com/tokio-rs/axum/pull/2961
[#2978]: https://github.com/tokio-rs/axum/pull/2978
[#2992]: https://github.com/tokio-rs/axum/pull/2992

# 0.7.7

- **change**: Remove manual tables of content from the documentation, since
  rustdoc now generates tables of content in the sidebar ([#2921])

[#2921]: https://github.com/tokio-rs/axum/pull/2921

# 0.7.6

- **change:** Avoid cloning `Arc` during deserialization of `Path`
- **added:** `axum::serve::Serve::tcp_nodelay` and `axum::serve::WithGracefulShutdown::tcp_nodelay` ([#2653])
- **added:** `Router::has_routes` function ([#2790])
- **change:** Update tokio-tungstenite to 0.23 ([#2841])
- **added:** `Serve::local_addr` and `WithGracefulShutdown::local_addr` functions ([#2881])

[#2653]: https://github.com/tokio-rs/axum/pull/2653
[#2790]: https://github.com/tokio-rs/axum/pull/2790
[#2841]: https://github.com/tokio-rs/axum/pull/2841
[#2881]: https://github.com/tokio-rs/axum/pull/2881

# 0.7.5 (24. March, 2024)

- **fixed:** Fixed layers being cloned when calling `axum::serve` directly with
  a `Router` or `MethodRouter` ([#2586])
- **fixed:** `h2` is no longer pulled as a dependency unless the `http2` feature
  is enabled ([#2605])
- **added:** Add `#[debug_middleware]` ([#1993], [#2725])

[#1993]: https://github.com/tokio-rs/axum/pull/1993
[#2725]: https://github.com/tokio-rs/axum/pull/2725
[#2586]: https://github.com/tokio-rs/axum/pull/2586
[#2605]: https://github.com/tokio-rs/axum/pull/2605

# 0.7.4 (13. January, 2024)

- **fixed:** Fix performance regression present since axum 0.7.0 ([#2483])
- **fixed:** Improve `debug_handler` on tuple response types ([#2201])
- **added:** Add `must_use` attribute to `Serve` and `WithGracefulShutdown` ([#2484])
- **added:** Re-export `axum_core::body::BodyDataStream` from axum

[#2201]: https://github.com/tokio-rs/axum/pull/2201
[#2483]: https://github.com/tokio-rs/axum/pull/2483
[#2484]: https://github.com/tokio-rs/axum/pull/2484

# 0.7.3 (29. December, 2023)

- **added:** `Body` implements `From<()>` now ([#2411])
- **change:** Update version of multer used internally for multipart ([#2433])
- **change:** Update tokio-tungstenite to 0.21 ([#2435])
- **added:** Enable `tracing` feature by default ([#2460])
- **added:** Support graceful shutdown on `serve` ([#2398])
- **added:** `RouterIntoService` implements `Clone` ([#2456])

[#2411]: https://github.com/tokio-rs/axum/pull/2411
[#2433]: https://github.com/tokio-rs/axum/pull/2433
[#2435]: https://github.com/tokio-rs/axum/pull/2435
[#2460]: https://github.com/tokio-rs/axum/pull/2460
[#2398]: https://github.com/tokio-rs/axum/pull/2398
[#2456]: https://github.com/tokio-rs/axum/pull/2456

# 0.7.2 (03. December, 2023)

- **added:** Add `axum::body::to_bytes` ([#2373])
- **fixed:** Gracefully handle accept errors in `serve` ([#2400])

[#2373]: https://github.com/tokio-rs/axum/pull/2373
[#2400]: https://github.com/tokio-rs/axum/pull/2400

# 0.7.1 (27. November, 2023)

- **fix**: Fix readme.

# 0.7.0 (27. November, 2023)

- **breaking:** Update public dependencies. axum now requires
    - [hyper](https://crates.io/crates/hyper) 1.0
    - [http](https://crates.io/crates/http) 1.0
    - [http-body](https://crates.io/crates/http-body) 1.0
- **breaking:** axum now requires [tower-http](https://crates.io/crates/tower-http) 0.5
- **breaking:** Remove deprecated `WebSocketUpgrade::max_send_queue`
- **breaking:** The following types/traits are no longer generic over the request body
  (i.e. the `B` type param has been removed) ([#1751] and [#1789]):
  - `FromRequestParts`
  - `FromRequest`
  - `HandlerService`
  - `HandlerWithoutStateExt`
  - `Handler`
  - `LayeredFuture`
  - `Layered`
  - `MethodRouter`
  - `Next`
  - `RequestExt`
  - `RouteFuture`
  - `Route`
  - `Router`
- **breaking:** axum no longer re-exports `hyper::Body` as that type is removed
  in hyper 1.0. Instead axum has its own body type at `axum::body::Body` ([#1751])
- **breaking:** `extract::BodyStream` has been removed as `body::Body`
  implements `Stream` and `FromRequest` directly ([#1751])
- **breaking:** Change `sse::Event::json_data` to use `axum_core::Error` as its error type ([#1762])
- **breaking:** Rename `DefaultOnFailedUpdgrade` to `DefaultOnFailedUpgrade` ([#1664])
- **breaking:** Rename `OnFailedUpdgrade` to `OnFailedUpgrade` ([#1664])
- **breaking:** `TypedHeader` has been moved to `axum-extra` as `axum_extra::TypedHeader` and requires enabling the `typed-header` feature on `axum-extra`. The `headers` feature has been removed from axum; what it provided under `axum::headers` is now found in `axum_extra::headers` by default. ([#1850])
- **breaking:** Removed re-exports of `Empty` and `Full`. Use
  `axum::body::Body::empty` and `axum::body::Body::from` respectively ([#1789])
- **breaking:** The response returned by `IntoResponse::into_response` must use
  `axum::body::Body` as the body type. `axum::response::Response` does this
  ([#1789])
- **breaking:** Removed the `BoxBody` type alias and its `box_body`
  constructor. Use `axum::body::Body::new` instead ([#1789])
- **breaking:** Remove `RawBody` extractor. `axum::body::Body` implements `FromRequest` directly ([#1789])
- **breaking:** The following types from `http-body` no longer implement `IntoResponse`:
  - `Full`, use `Body::from` instead
  - `Empty`, use `Body::empty` instead
  - `BoxBody`, use `Body::new` instead
  - `UnsyncBoxBody`, use `Body::new` instead
  - `MapData`, use `Body::new` instead
  - `MapErr`, use `Body::new` instead
- **added:** Add `axum::extract::Request` type alias where the body is `axum::body::Body` ([#1789])
- **added:** Add `Router::as_service` and `Router::into_service` to workaround
  type inference issues when calling `ServiceExt` methods on a `Router` ([#1835])
- **breaking:** Removed `axum::Server` as it was removed in hyper 1.0. Instead
  use `axum::serve(listener, service)` or hyper/hyper-util for more configuration options ([#1868])
- **breaking:** Only inherit fallbacks for routers nested with `Router::nest`.
  Routers nested with `Router::nest_service` will no longer inherit fallbacks ([#1956])
- **fixed:** Don't remove the `Sec-WebSocket-Key` header in `WebSocketUpgrade` ([#1972])
- **added:** Add `axum::extract::Query::try_from_uri` ([#2058])
- **added:** Implement `IntoResponse` for `Box<str>` and `Box<[u8]>` ([#2035])
- **breaking:** Simplify `MethodFilter`. It no longer uses bitflags ([#2073])
- **fixed:** Fix bugs around merging routers with nested fallbacks ([#2096])
- **fixed:** Fix `.source()` of composite rejections ([#2030])
- **fixed:** Allow unreachable code in `#[debug_handler]` ([#2014])
- **change:** axum's MSRV is now 1.66 ([#1882])
- **added:** Implement `IntoResponse` for `(R,) where R: IntoResponse` ([#2143])
- **changed:** For SSE, add space between field and value for compatibility ([#2149])
- **added:** Add `NestedPath` extractor ([#1924])
- **added:** Add `handle_error` function to existing `ServiceExt` trait ([#2235])
- **breaking:** `impl<T> IntoResponse(Parts) for Extension<T>` now requires
  `T: Clone`, as that is required by the http crate ([#1882])
- **added:** Add `axum::Json::from_bytes` ([#2244])
- **added:** Implement `FromRequestParts` for `http::request::Parts` ([#2328])
- **added:** Implement `FromRequestParts` for `http::Extensions` ([#2328])
- **fixed:** Clearly document applying `DefaultBodyLimit` to individual routes ([#2157])

[#1664]: https://github.com/tokio-rs/axum/pull/1664
[#1751]: https://github.com/tokio-rs/axum/pull/1751
[#1762]: https://github.com/tokio-rs/axum/pull/1762
[#1789]: https://github.com/tokio-rs/axum/pull/1789
[#1835]: https://github.com/tokio-rs/axum/pull/1835
[#1850]: https://github.com/tokio-rs/axum/pull/1850
[#1868]: https://github.com/tokio-rs/axum/pull/1868
[#1882]: https://github.com/tokio-rs/axum/pull/1882
[#1924]: https://github.com/tokio-rs/axum/pull/1924
[#1956]: https://github.com/tokio-rs/axum/pull/1956
[#1972]: https://github.com/tokio-rs/axum/pull/1972
[#2014]: https://github.com/tokio-rs/axum/pull/2014
[#2021]: https://github.com/tokio-rs/axum/pull/2021
[#2030]: https://github.com/tokio-rs/axum/pull/2030
[#2058]: https://github.com/tokio-rs/axum/pull/2058
[#2073]: https://github.com/tokio-rs/axum/pull/2073
[#2096]: https://github.com/tokio-rs/axum/pull/2096
[#2140]: https://github.com/tokio-rs/axum/pull/2140
[#2143]: https://github.com/tokio-rs/axum/pull/2143
[#2149]: https://github.com/tokio-rs/axum/pull/2149
[#2157]: https://github.com/tokio-rs/axum/pull/2157
[#2235]: https://github.com/tokio-rs/axum/pull/2235
[#2244]: https://github.com/tokio-rs/axum/pull/2244
[#2328]: https://github.com/tokio-rs/axum/pull/2328

# 0.6.20 (03. August, 2023)

- **added:** `WebSocketUpgrade::write_buffer_size` and `WebSocketUpgrade::max_write_buffer_size`
- **changed:** Deprecate `WebSocketUpgrade::max_send_queue`
- **change:** Update tokio-tungstenite to 0.20
- **added:** Implement `Handler` for `T: IntoResponse` ([#2140])

[#2140]: https://github.com/tokio-rs/axum/pull/2140

# 0.6.19 (17. July, 2023)

- **added:** Add `axum::extract::Query::try_from_uri` ([#2058])
- **added:** Implement `IntoResponse` for `Box<str>` and `Box<[u8]>` ([#2035])
- **fixed:** Fix bugs around merging routers with nested fallbacks ([#2096])
- **fixed:** Fix `.source()` of composite rejections ([#2030])
- **fixed:** Allow unreachable code in `#[debug_handler]` ([#2014])
- **change:** Update tokio-tungstenite to 0.19 ([#2021])
- **change:** axum's MSRV is now 1.63 ([#2021])

[#2014]: https://github.com/tokio-rs/axum/pull/2014
[#2021]: https://github.com/tokio-rs/axum/pull/2021
[#2030]: https://github.com/tokio-rs/axum/pull/2030
[#2035]: https://github.com/tokio-rs/axum/pull/2035
[#2058]: https://github.com/tokio-rs/axum/pull/2058
[#2096]: https://github.com/tokio-rs/axum/pull/2096

# 0.6.18 (30. April, 2023)

- **fixed:** Don't remove the `Sec-WebSocket-Key` header in `WebSocketUpgrade` ([#1972])

[#1972]: https://github.com/tokio-rs/axum/pull/1972

# 0.6.17 (25. April, 2023)

- **fixed:** Fix fallbacks causing a panic on `CONNECT` requests ([#1958])

[#1958]: https://github.com/tokio-rs/axum/pull/1958

# 0.6.16 (18. April, 2023)

- **fixed:** Don't allow extracting `MatchedPath` in fallbacks ([#1934])
- **fixed:** Fix panic if `Router` with something nested at `/` was used as a fallback ([#1934])
- **added:** Document that `Router::new().fallback(...)` isn't optimal ([#1940])

[#1934]: https://github.com/tokio-rs/axum/pull/1934
[#1940]: https://github.com/tokio-rs/axum/pull/1940

# 0.6.15 (12. April, 2023)

- **fixed:** Removed additional leftover debug messages ([#1927])

[#1927]: https://github.com/tokio-rs/axum/pull/1927

# 0.6.14 (11. April, 2023)

- **fixed:** Removed leftover "path_router hit" debug message ([#1925])

[#1925]: https://github.com/tokio-rs/axum/pull/1925

# 0.6.13 (11. April, 2023)

- **added:** Log rejections from built-in extractors with the
  `axum::rejection=trace` target ([#1890])
- **fixed:** Fixed performance regression with `Router::nest` introduced in
  0.6.0. `nest` now flattens the routes which performs better ([#1711])
- **fixed:** Extracting `MatchedPath` in nested handlers now gives the full
  matched path, including the nested path ([#1711])
- **added:** Implement `Deref` and `DerefMut` for built-in extractors ([#1922])

[#1711]: https://github.com/tokio-rs/axum/pull/1711
[#1890]: https://github.com/tokio-rs/axum/pull/1890
[#1922]: https://github.com/tokio-rs/axum/pull/1922

# 0.6.12 (22. March, 2023)

- **added:** Implement `IntoResponse` for `MultipartError` ([#1861])
- **fixed:** More clearly document what wildcards matches ([#1873])

[#1861]: https://github.com/tokio-rs/axum/pull/1861
[#1873]: https://github.com/tokio-rs/axum/pull/1873

# 0.6.11 (13. March, 2023)

- **fixed:** Don't require `S: Debug` for `impl Debug for Router<S>` ([#1836])
- **fixed:** Clone state a bit less when handling requests ([#1837])
- **fixed:** Unpin itoa dependency ([#1815])

[#1815]: https://github.com/tokio-rs/axum/pull/1815
[#1836]: https://github.com/tokio-rs/axum/pull/1836
[#1837]: https://github.com/tokio-rs/axum/pull/1837

# 0.6.10 (03. March, 2023)

- **fixed:** Add `#[must_use]` attributes to types that do nothing unless used ([#1809])
- **fixed:** Gracefully handle missing headers in the `TypedHeader` extractor ([#1810])
- **fixed:** Fix routing issues when loading a `Router` via a dynamic library ([#1806])

[#1806]: https://github.com/tokio-rs/axum/pull/1806
[#1809]: https://github.com/tokio-rs/axum/pull/1809
[#1810]: https://github.com/tokio-rs/axum/pull/1810

# 0.6.9 (24. February, 2023)

- **changed:** Update to tower-http 0.4. axum is still compatible with tower-http 0.3 ([#1783])

[#1783]: https://github.com/tokio-rs/axum/pull/1783

# 0.6.8 (24. February, 2023)

- **fixed:** Fix `Allow` missing from routers with middleware ([#1773])
- **added:** Add `KeepAlive::event` for customizing the event sent for SSE keep alive ([#1729])

[#1729]: https://github.com/tokio-rs/axum/pull/1729
[#1773]: https://github.com/tokio-rs/axum/pull/1773

# 0.6.7 (17. February, 2023)

- **added:** Add `FormRejection::FailedToDeserializeFormBody` which is returned
  if the request body couldn't be deserialized into the target type, as opposed
  to `FailedToDeserializeForm` which is only for query parameters ([#1683])
- **added:** Add `MockConnectInfo` for setting `ConnectInfo` during tests ([#1767])

[#1683]: https://github.com/tokio-rs/axum/pull/1683
[#1767]: https://github.com/tokio-rs/axum/pull/1767

# 0.6.6 (12. February, 2023)

- **fixed:** Enable passing `MethodRouter` to `Router::fallback` ([#1730])

[#1730]: https://github.com/tokio-rs/axum/pull/1730

# 0.6.5 (11. February, 2023)

- **fixed:** Fix `#[debug_handler]` sometimes giving wrong borrow related suggestions ([#1710])
- Document gotchas related to using `impl IntoResponse` as the return type from handler functions ([#1736])

[#1710]: https://github.com/tokio-rs/axum/pull/1710
[#1736]: https://github.com/tokio-rs/axum/pull/1736

# 0.6.4 (22. January, 2023)

- Depend on axum-macros 0.3.2

# 0.6.3 (20. January, 2023)

- **added:** Implement `IntoResponse` for `&'static [u8; N]` and `[u8; N]` ([#1690])
- **fixed:** Make `Path` support types using `serde::Deserializer::deserialize_any` ([#1693])
- **added:** Add `RawPathParams` ([#1713])
- **added:** Implement `Clone` and `Service` for `axum::middleware::Next` ([#1712])
- **fixed:** Document required tokio features to run "Hello, World!" example ([#1715])

[#1690]: https://github.com/tokio-rs/axum/pull/1690
[#1693]: https://github.com/tokio-rs/axum/pull/1693
[#1712]: https://github.com/tokio-rs/axum/pull/1712
[#1713]: https://github.com/tokio-rs/axum/pull/1713
[#1715]: https://github.com/tokio-rs/axum/pull/1715

# 0.6.2 (9. January, 2023)

- **added:** Add `body_text` and `status` methods to built-in rejections ([#1612])
- **added:** Enable the `runtime` feature of `hyper` when using `tokio` ([#1671])

[#1612]: https://github.com/tokio-rs/axum/pull/1612
[#1671]: https://github.com/tokio-rs/axum/pull/1671

# 0.6.1 (29. November, 2022)

- **added:** Expand the docs for `Router::with_state` ([#1580])

[#1580]: https://github.com/tokio-rs/axum/pull/1580

# 0.6.0 (25. November, 2022)

## Routing

- **fixed:** Nested routers are now allowed to have fallbacks ([#1521]):

  ```rust
  let api_router = Router::new()
      .route("/users", get(|| { ... }))
      .fallback(api_fallback);

  let app = Router::new()
      // this would panic in 0.5 but in 0.6 it just works
      //
      // requests starting with `/api` but not handled by `api_router`
      // will go to `api_fallback`
      .nest("/api", api_router);
  ```

  The outer router's fallback will still apply if a nested router doesn't have
  its own fallback:

  ```rust
  // this time without a fallback
  let api_router = Router::new().route("/users", get(|| { ... }));

  let app = Router::new()
      .nest("/api", api_router)
      // `api_router` will inherit this fallback
      .fallback(app_fallback);
  ```

- **breaking:** The request `/foo/` no longer matches `/foo/*rest`. If you want
  to match `/foo/` you have to add a route specifically for that ([#1086])

  For example:

  ```rust
  use axum::{Router, routing::get, extract::Path};

  let app = Router::new()
      // this will match `/foo/bar/baz`
      .route("/foo/*rest", get(handler))
      // this will match `/foo/`
      .route("/foo/", get(handler))
      // if you want `/foo` to match you must also add an explicit route for it
      .route("/foo", get(handler));

  async fn handler(
      // use an `Option` because `/foo/` and `/foo` don't have any path params
      params: Option<Path<String>>,
  ) {}
  ```

- **breaking:** Path params for wildcard routes no longer include the prefix
  `/`. e.g. `/foo.js` will match `/*filepath` with a value of `foo.js`, _not_
  `/foo.js` ([#1086])

  For example:

  ```rust
  use axum::{Router, routing::get, extract::Path};

  let app = Router::new().route("/foo/*rest", get(handler));

  async fn handler(
      Path(params): Path<String>,
  ) {
      // for the request `/foo/bar/baz` the value of `params` will be `bar/baz`
      //
      // on 0.5 it would be `/bar/baz`
  }
  ```

- **fixed:** Routes like `/foo` and `/*rest` are no longer considered
  overlapping. `/foo` will take priority ([#1086])

  For example:

  ```rust
  use axum::{Router, routing::get};

  let app = Router::new()
      // this used to not be allowed but now just works
      .route("/foo/*rest", get(foo))
      .route("/foo/bar", get(bar));

  async fn foo() {}

  async fn bar() {}
  ```

- **breaking:** Automatic trailing slash redirects have been removed.
  Previously if you added a route for `/foo`, axum would redirect calls to
  `/foo/` to `/foo` (or vice versa for `/foo/`):

  ```rust
  use axum::{Router, routing::get};

  let app = Router::new()
      // a request to `GET /foo/` will now get `404 Not Found`
      // whereas in 0.5 axum would redirect to `/foo`
      //
      // same goes the other way if you had the route `/foo/`
      // axum will no longer redirect from `/foo` to `/foo/`
      .route("/foo", get(handler));

  async fn handler() {}
  ```

  Either explicitly add routes for `/foo` and `/foo/` or use
  `axum_extra::routing::RouterExt::route_with_tsr` if you want the old behavior
  ([#1119])

- **breaking:** `Router::fallback` now only accepts `Handler`s (similarly to
  what `get`, `post`, etc. accept). Use the new `Router::fallback_service` for
  setting any `Service` as the fallback ([#1155])

  This fallback on 0.5:

  ```rust
  use axum::{Router, handler::Handler};

  let app = Router::new().fallback(fallback.into_service());

  async fn fallback() {}
  ```

  Becomes this in 0.6

  ```rust
  use axum::Router;

  let app = Router::new().fallback(fallback);

  async fn fallback() {}
  ```

- **breaking:** It is no longer supported to `nest` twice at the same path, i.e.
  `.nest("/foo", a).nest("/foo", b)` will panic. Instead use `.nest("/foo", a.merge(b))`
- **breaking:** It is no longer supported to `nest` a router and add a route at
  the same path, such as `.nest("/a", _).route("/a", _)`. Instead use
  `.nest("/a/", _).route("/a", _)`.
- **changed:** `Router::nest` now only accepts `Router`s, the general-purpose
  `Service` nesting method has been renamed to `nest_service` ([#1368])
- **breaking:** Allow `Error: Into<Infallible>` for `Route::{layer, route_layer}` ([#924])
- **breaking:** `MethodRouter` now panics on overlapping routes ([#1102])
- **breaking:** `Router::route` now only accepts `MethodRouter`s created with
  `get`, `post`, etc. Use the new `Router::route_service` for routing to
  any `Service`s ([#1155])
- **breaking:** Adding a `.route_layer` onto a `Router` or `MethodRouter`
  without any routes will now result in a panic. Previously, this just did
  nothing. [#1327]
- **breaking:** `RouterService` has been removed since `Router` now implements
  `Service` when the state is `()`. Use `Router::with_state` to provide the
  state and get a `Router<()>`. Note that `RouterService` only existed in the
  pre-releases, not 0.5 ([#1552])

## Extractors

- **added:** Added new type safe `State` extractor. This can be used with
  `Router::with_state` and gives compile errors for missing states, whereas
  `Extension` would result in runtime errors ([#1155])

  We recommend migrating from `Extension` to `State` for sharing application state since that is more type
  safe and faster. That is done by using `Router::with_state` and `State`.

  This setup in 0.5

  ```rust
  use axum::{routing::get, Extension, Router};

  let app = Router::new()
      .route("/", get(handler))
      .layer(Extension(AppState {}));

  async fn handler(Extension(app_state): Extension<AppState>) {}

  #[derive(Clone)]
  struct AppState {}
  ```

  Becomes this in 0.6 using `State`:

  ```rust
  use axum::{routing::get, extract::State, Router};

  let app = Router::new()
      .route("/", get(handler))
      .with_state(AppState {});

  async fn handler(State(app_state): State<AppState>) {}

  #[derive(Clone)]
  struct AppState {}
  ```

  If you have multiple extensions, you can use fields on `AppState` and implement
  `FromRef`:

  ```rust
  use axum::{extract::{State, FromRef}, routing::get, Router};

  let state = AppState {
      client: HttpClient {},
      database: Database {},
  };

  let app = Router::new().route("/", get(handler)).with_state(state);

  async fn handler(
      State(client): State<HttpClient>,
      State(database): State<Database>,
  ) {}

  // the derive requires enabling the "macros" feature
  #[derive(Clone, FromRef)]
  struct AppState {
      client: HttpClient,
      database: Database,
  }

  #[derive(Clone)]
  struct HttpClient {}

  #[derive(Clone)]
  struct Database {}
  ```

- **breaking:** It is now only possible for one extractor per handler to consume
  the request body. In 0.5 doing so would result in runtime errors but in 0.6 it
  is a compile error ([#1272])

  axum enforces this by only allowing the _last_ extractor to consume the
  request.

  For example:

  ```rust
  use axum::{Json, http::HeaderMap};

  // This won't compile on 0.6 because both `Json` and `String` need to consume
  // the request body. You can use either `Json` or `String`, but not both.
  async fn handler_1(
      json: Json<serde_json::Value>,
      string: String,
  ) {}

  // This won't work either since `Json` is not the last extractor.
  async fn handler_2(
      json: Json<serde_json::Value>,
      headers: HeaderMap,
  ) {}

  // This works!
  async fn handler_3(
      headers: HeaderMap,
      json: Json<serde_json::Value>,
  ) {}
  ```

  This is done by reworking the `FromRequest` trait and introducing a new
  `FromRequestParts` trait.

  If your extractor needs to consume the request body then you should implement
  `FromRequest`, otherwise implement `FromRequestParts`.

  This extractor in 0.5:

  ```rust
  struct MyExtractor { /* ... */ }

  impl<B> FromRequest<B> for MyExtractor
  where
      B: Send,
  {
      type Rejection = StatusCode;

      async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
          // ...
      }
  }
  ```

  Becomes this in 0.6:

  ```rust
  use axum::{
      extract::{FromRequest, FromRequestParts},
      http::{StatusCode, Request, request::Parts},
  };

  struct MyExtractor { /* ... */ }

  // implement `FromRequestParts` if you don't need to consume the request body
  impl<S> FromRequestParts<S> for MyExtractor
  where
      S: Send + Sync,
  {
      type Rejection = StatusCode;

      async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
          // ...
      }
  }

  // implement `FromRequest` if you do need to consume the request body
  impl<S, B> FromRequest<S, B> for MyExtractor
  where
      S: Send + Sync,
      B: Send + 'static,
  {
      type Rejection = StatusCode;

      async fn from_request(req: Request<B>, state: &S) -> Result<Self, Self::Rejection> {
          // ...
      }
  }
  ```

  For an example of how to write an extractor that accepts different
  `Content-Types` see the [`parse-body-based-on-content-type`] example.

- **added:** `FromRequest` and `FromRequestParts` derive macro re-exports from
  [`axum-macros`] behind the `macros` feature ([#1352])
- **added:** Add `RequestExt` and `RequestPartsExt` which adds convenience
  methods for running extractors to `http::Request` and `http::request::Parts` ([#1301])
- **added**: `JsonRejection` now displays the path at which a deserialization
  error occurred ([#1371])
- **added:** Add `extract::RawForm` for accessing raw urlencoded query bytes or request body ([#1487])
- **fixed:** Used `400 Bad Request` for `FailedToDeserializeQueryString`
  rejections, instead of `422 Unprocessable Entity` ([#1387])
- **changed**: The inner error of a `JsonRejection` is now
  `serde_path_to_error::Error<serde_json::Error>`.  Previously it was
  `serde_json::Error` ([#1371])
- **changed:** The default body limit now applies to the `Multipart` extractor ([#1420])
- **breaking:** `ContentLengthLimit` has been removed. Use `DefaultBodyLimit` instead ([#1400])
- **breaking:** `RequestParts` has been removed as part of the `FromRequest`
  rework ([#1272])
- **breaking:** `BodyAlreadyExtracted` has been removed ([#1272])
- **breaking:** The following types or traits have a new `S` type param
  which represents the state ([#1155]):
  - `Router`, defaults to `()`
  - `MethodRouter`, defaults to `()`
  - `FromRequest`, no default
  - `Handler`, no default
- **breaking:** `MatchedPath` can now no longer be extracted in middleware for
  nested routes. In previous versions it returned invalid data when extracted
  from a middleware applied to a nested router. `MatchedPath` can still be
  extracted from handlers and middleware that aren't on nested routers ([#1462])
- **breaking:** Rename `FormRejection::FailedToDeserializeQueryString` to
  `FormRejection::FailedToDeserializeForm` ([#1496])

## Middleware

- **added:** Support running extractors on `middleware::from_fn` functions ([#1088])
- **added**: Add `middleware::from_fn_with_state` to enable running extractors that require
  state ([#1342])
- **added:** Add `middleware::from_extractor_with_state` ([#1396])
- **added:** Add `map_request`, `map_request_with_state` for transforming the
  request with an async function ([#1408])
- **added:** Add `map_response`, `map_response_with_state` for transforming the
  response with an async function ([#1414])
- **added:** Support any middleware response that implements `IntoResponse` ([#1152])
- **breaking:** Remove `extractor_middleware` which was previously deprecated.
  Use `axum::middleware::from_extractor` instead ([#1077])
- **breaking:** Require middleware added with `Handler::layer` to have
  `Infallible` as the error type ([#1152])

## Misc

- **added:** Support compiling to WASM. See the `simple-router-wasm` example
  for more details ([#1382])
- **added:** Add `ServiceExt` with methods for turning any `Service` into a
  `MakeService` similarly to `Router::into_make_service` ([#1302])
- **added:** String and binary `From` impls have been added to `extract::ws::Message`
  to be more inline with `tungstenite` ([#1421])
- **added:** Add `#[derive(axum::extract::FromRef)]` ([#1430])
- **added:** Add `accept_unmasked_frames` setting in WebSocketUpgrade ([#1529])
- **added:** Add `WebSocketUpgrade::on_failed_upgrade` to customize what to do
  when upgrading a connection fails ([#1539])
- **fixed:** Annotate panicking functions with `#[track_caller]` so the error
  message points to where the user added the invalid route, rather than
  somewhere internally in axum ([#1248])
- **changed:** axum's MSRV is now 1.60 ([#1239])
- **changed:** For methods that accept some `S: Service`, the bounds have been
  relaxed so the response type must implement `IntoResponse` rather than being a
  literal `Response`
- **breaking:** New `tokio` default feature needed for WASM support. If you
  don't need WASM support but have `default_features = false` for other reasons
  you likely need to re-enable the `tokio` feature ([#1382])
- **breaking:** `handler::{WithState, IntoService}` are merged into one type,
  named `HandlerService` ([#1418])

[#924]: https://github.com/tokio-rs/axum/pull/924
[#1077]: https://github.com/tokio-rs/axum/pull/1077
[#1086]: https://github.com/tokio-rs/axum/pull/1086
[#1088]: https://github.com/tokio-rs/axum/pull/1088
[#1102]: https://github.com/tokio-rs/axum/pull/1102
[#1119]: https://github.com/tokio-rs/axum/pull/1119
[#1152]: https://github.com/tokio-rs/axum/pull/1152
[#1155]: https://github.com/tokio-rs/axum/pull/1155
[#1239]: https://github.com/tokio-rs/axum/pull/1239
[#1248]: https://github.com/tokio-rs/axum/pull/1248
[#1272]: https://github.com/tokio-rs/axum/pull/1272
[#1301]: https://github.com/tokio-rs/axum/pull/1301
[#1302]: https://github.com/tokio-rs/axum/pull/1302
[#1327]: https://github.com/tokio-rs/axum/pull/1327
[#1342]: https://github.com/tokio-rs/axum/pull/1342
[#1346]: https://github.com/tokio-rs/axum/pull/1346
[#1352]: https://github.com/tokio-rs/axum/pull/1352
[#1368]: https://github.com/tokio-rs/axum/pull/1368
[#1371]: https://github.com/tokio-rs/axum/pull/1371
[#1382]: https://github.com/tokio-rs/axum/pull/1382
[#1387]: https://github.com/tokio-rs/axum/pull/1387
[#1389]: https://github.com/tokio-rs/axum/pull/1389
[#1396]: https://github.com/tokio-rs/axum/pull/1396
[#1397]: https://github.com/tokio-rs/axum/pull/1397
[#1400]: https://github.com/tokio-rs/axum/pull/1400
[#1408]: https://github.com/tokio-rs/axum/pull/1408
[#1414]: https://github.com/tokio-rs/axum/pull/1414
[#1418]: https://github.com/tokio-rs/axum/pull/1418
[#1420]: https://github.com/tokio-rs/axum/pull/1420
[#1421]: https://github.com/tokio-rs/axum/pull/1421
[#1430]: https://github.com/tokio-rs/axum/pull/1430
[#1462]: https://github.com/tokio-rs/axum/pull/1462
[#1487]: https://github.com/tokio-rs/axum/pull/1487
[#1496]: https://github.com/tokio-rs/axum/pull/1496
[#1521]: https://github.com/tokio-rs/axum/pull/1521
[#1529]: https://github.com/tokio-rs/axum/pull/1529
[#1532]: https://github.com/tokio-rs/axum/pull/1532
[#1539]: https://github.com/tokio-rs/axum/pull/1539
[#1552]: https://github.com/tokio-rs/axum/pull/1552
[`axum-macros`]: https://docs.rs/axum-macros/latest/axum_macros/
[`parse-body-based-on-content-type`]: https://github.com/tokio-rs/axum/blob/main/examples/parse-body-based-on-content-type/src/main.rs

<details>
<summary>0.6.0 Pre-Releases</summary>

# 0.6.0-rc.5 (18. November, 2022)

- **breaking:** `Router::with_state` is no longer a constructor. It is instead
  used to convert the router into a `RouterService` ([#1532])

  This nested router on 0.6.0-rc.4

  ```rust
  Router::with_state(state).route(...);
  ```

  Becomes this in 0.6.0-rc.5

  ```rust
  Router::new().route(...).with_state(state);
  ```

- **breaking:** `Router::inherit_state` has been removed. Use
  `Router::with_state` instead ([#1532])
- **breaking:** `Router::nest` and `Router::merge` now only supports nesting
  routers that use the same state type as the router they're being merged into.
  Use `FromRef` for substates ([#1532])

- **added:** Add `accept_unmasked_frames` setting in WebSocketUpgrade ([#1529])
- **fixed:** Nested routers will now inherit fallbacks from outer routers ([#1521])
- **added:** Add `WebSocketUpgrade::on_failed_upgrade` to customize what to do
  when upgrading a connection fails ([#1539])

[#1521]: https://github.com/tokio-rs/axum/pull/1521
[#1529]: https://github.com/tokio-rs/axum/pull/1529
[#1532]: https://github.com/tokio-rs/axum/pull/1532
[#1539]: https://github.com/tokio-rs/axum/pull/1539

# 0.6.0-rc.4 (9. November, 2022)

- **changed**: The inner error of a `JsonRejection` is now
  `serde_path_to_error::Error<serde_json::Error>`.  Previously it was
  `serde_json::Error` ([#1371])
- **added**: `JsonRejection` now displays the path at which a deserialization
  error occurred ([#1371])
- **fixed:** Support streaming/chunked requests in `ContentLengthLimit` ([#1389])
- **fixed:** Used `400 Bad Request` for `FailedToDeserializeQueryString`
  rejections, instead of `422 Unprocessable Entity` ([#1387])
- **added:** Add `middleware::from_extractor_with_state` ([#1396])
- **added:** Add `DefaultBodyLimit::max` for changing the default body limit ([#1397])
- **added:** Add `map_request`, `map_request_with_state` for transforming the
  request with an async function ([#1408])
- **added:** Add `map_response`, `map_response_with_state` for transforming the
  response with an async function ([#1414])
- **breaking:** `ContentLengthLimit` has been removed. Use `DefaultBodyLimit` instead ([#1400])
- **changed:** `Router` no longer implements `Service`, call `.into_service()`
  on it to obtain a `RouterService` that does ([#1368])
- **added:** Add `Router::inherit_state`, which creates a `Router` with an
  arbitrary state type without actually supplying the state; such a `Router`
  can't be turned into a service directly (`.into_service()` will panic), but
  can be nested or merged into a `Router` with the same state type ([#1368])
- **changed:** `Router::nest` now only accepts `Router`s, the general-purpose
  `Service` nesting method has been renamed to `nest_service` ([#1368])
- **added:** Support compiling to WASM. See the `simple-router-wasm` example
  for more details ([#1382])
- **breaking:** New `tokio` default feature needed for WASM support. If you
  don't need WASM support but have `default_features = false` for other reasons
  you likely need to re-enable the `tokio` feature ([#1382])
- **breaking:** `handler::{WithState, IntoService}` are merged into one type,
  named `HandlerService` ([#1418])
- **changed:** The default body limit now applies to the `Multipart` extractor ([#1420])
- **added:** String and binary `From` impls have been added to `extract::ws::Message`
  to be more inline with `tungstenite` ([#1421])
- **added:** Add `#[derive(axum::extract::FromRef)]` ([#1430])
- **added:** `FromRequest` and `FromRequestParts` derive macro re-exports from
  [`axum-macros`] behind the `macros` feature ([#1352])
- **breaking:** `MatchedPath` can now no longer be extracted in middleware for
  nested routes ([#1462])
- **added:** Add `extract::RawForm` for accessing raw urlencoded query bytes or request body ([#1487])
- **breaking:** Rename `FormRejection::FailedToDeserializeQueryString` to
  `FormRejection::FailedToDeserializeForm` ([#1496])

[#1352]: https://github.com/tokio-rs/axum/pull/1352
[#1368]: https://github.com/tokio-rs/axum/pull/1368
[#1371]: https://github.com/tokio-rs/axum/pull/1371
[#1382]: https://github.com/tokio-rs/axum/pull/1382
[#1387]: https://github.com/tokio-rs/axum/pull/1387
[#1389]: https://github.com/tokio-rs/axum/pull/1389
[#1396]: https://github.com/tokio-rs/axum/pull/1396
[#1397]: https://github.com/tokio-rs/axum/pull/1397
[#1400]: https://github.com/tokio-rs/axum/pull/1400
[#1408]: https://github.com/tokio-rs/axum/pull/1408
[#1414]: https://github.com/tokio-rs/axum/pull/1414
[#1418]: https://github.com/tokio-rs/axum/pull/1418
[#1420]: https://github.com/tokio-rs/axum/pull/1420
[#1421]: https://github.com/tokio-rs/axum/pull/1421
[#1430]: https://github.com/tokio-rs/axum/pull/1430
[#1462]: https://github.com/tokio-rs/axum/pull/1462
[#1487]: https://github.com/tokio-rs/axum/pull/1487
[#1496]: https://github.com/tokio-rs/axum/pull/1496

# 0.6.0-rc.3 (8. November, 2022)

Yanked, as it didn't compile in release mode.

# 0.6.0-rc.2 (10. September, 2022)

## Security

- **breaking:** Added default limit to how much data `Bytes::from_request` will
  consume. Previously it would attempt to consume the entire request body
  without checking its length. This meant if a malicious peer sent an large (or
  infinite) request body your server might run out of memory and crash.

  The default limit is at 2 MB and can be disabled by adding the new
  `DefaultBodyLimit::disable()` middleware. See its documentation for more
  details.

  This also applies to these extractors which used `Bytes::from_request`
  internally:
  - `Form`
  - `Json`
  - `String`

  ([#1346])

## Routing

- **breaking:** Adding a `.route_layer` onto a `Router` or `MethodRouter`
  without any routes will now result in a panic. Previously, this just did
  nothing. [#1327]


[`axum-macros`]: https://docs.rs/axum-macros/latest/axum_macros/

## Middleware

- **added**: Add `middleware::from_fn_with_state` and
  `middleware::from_fn_with_state_arc` to enable running extractors that require
  state ([#1342])

[#1327]: https://github.com/tokio-rs/axum/pull/1327
[#1342]: https://github.com/tokio-rs/axum/pull/1342
[#1346]: https://github.com/tokio-rs/axum/pull/1346

# 0.6.0-rc.1 (23. August, 2022)

## Routing

- **breaking:** Nested `Router`s will no longer delegate to the outer `Router`'s
  fallback. Instead you must explicitly set a fallback on the inner `Router` ([#1086])

  This nested router on 0.5:

  ```rust
  use axum::{Router, handler::Handler};

  let api_routes = Router::new();

  let app = Router::new()
      .nest("/api", api_routes)
      .fallback(fallback.into_service());

  async fn fallback() {}
  ```

  Becomes this in 0.6:

  ```rust
  use axum::Router;

  let api_routes = Router::new()
      // we have to explicitly set the fallback here
      // since nested routers no longer delegate to the outer
      // router's fallback
      .fallback(fallback);

  let app = Router::new()
      .nest("/api", api_routes)
      .fallback(fallback);

  async fn fallback() {}
  ```

- **breaking:** The request `/foo/` no longer matches `/foo/*rest`. If you want
  to match `/foo/` you have to add a route specifically for that ([#1086])

  For example:

  ```rust
  use axum::{Router, routing::get, extract::Path};

  let app = Router::new()
      // this will match `/foo/bar/baz`
      .route("/foo/*rest", get(handler))
      // this will match `/foo/`
      .route("/foo/", get(handler))
      // if you want `/foo` to match you must also add an explicit route for it
      .route("/foo", get(handler));

  async fn handler(
      // use an `Option` because `/foo/` and `/foo` don't have any path params
      params: Option<Path<String>>,
  ) {}
  ```

- **breaking:** Path params for wildcard routes no longer include the prefix
  `/`. e.g. `/foo.js` will match `/*filepath` with a value of `foo.js`, _not_
  `/foo.js` ([#1086])

  For example:

  ```rust
  use axum::{Router, routing::get, extract::Path};

  let app = Router::new().route("/foo/*rest", get(handler));

  async fn handler(
      Path(params): Path<String>,
  ) {
      // for the request `/foo/bar/baz` the value of `params` will be `bar/baz`
      //
      // on 0.5 it would be `/bar/baz`
  }
  ```

- **fixed:** Routes like `/foo` and `/*rest` are no longer considered
  overlapping. `/foo` will take priority ([#1086])

  For example:

  ```rust
  use axum::{Router, routing::get};

  let app = Router::new()
      // this used to not be allowed but now just works
      .route("/foo/*rest", get(foo))
      .route("/foo/bar", get(bar));

  async fn foo() {}

  async fn bar() {}
  ```

- **breaking:** Trailing slash redirects have been removed. Previously if you
  added a route for `/foo`, axum would redirect calls to `/foo/` to `/foo` (or
  vice versa for `/foo/`). That is no longer supported and such requests will
  now be sent to the fallback. Consider using
  `axum_extra::routing::RouterExt::route_with_tsr` if you want the old behavior
  ([#1119])

  For example:

  ```rust
  use axum::{Router, routing::get};

  let app = Router::new()
      // a request to `GET /foo/` will now get `404 Not Found`
      // whereas in 0.5 axum would redirect to `/foo`
      //
      // same goes the other way if you had the route `/foo/`
      // axum will no longer redirect from `/foo` to `/foo/`
      .route("/foo", get(handler));

  async fn handler() {}
  ```

- **breaking:** `Router::fallback` now only accepts `Handler`s (similarly to
  what `get`, `post`, etc accept). Use the new `Router::fallback_service` for
  setting any `Service` as the fallback ([#1155])

  This fallback on 0.5:

  ```rust
  use axum::{Router, handler::Handler};

  let app = Router::new().fallback(fallback.into_service());

  async fn fallback() {}
  ```

  Becomes this in 0.6

  ```rust
  use axum::Router;

  let app = Router::new().fallback(fallback);

  async fn fallback() {}
  ```

- **breaking:** Allow `Error: Into<Infallible>` for `Route::{layer, route_layer}` ([#924])
- **breaking:** `MethodRouter` now panics on overlapping routes ([#1102])
- **breaking:** `Router::route` now only accepts `MethodRouter`s created with
  `get`, `post`, etc. Use the new `Router::route_service` for routing to
  any `Service`s ([#1155])

## Extractors

- **added:** Added new type safe `State` extractor. This can be used with
  `Router::with_state` and gives compile errors for missing states, whereas
  `Extension` would result in runtime errors ([#1155])

  We recommend migrating from `Extension` to `State` since that is more type
  safe and faster. That is done by using `Router::with_state` and `State`.

  This setup in 0.5

  ```rust
  use axum::{routing::get, Extension, Router};

  let app = Router::new()
      .route("/", get(handler))
      .layer(Extension(AppState {}));

  async fn handler(Extension(app_state): Extension<AppState>) {}

  #[derive(Clone)]
  struct AppState {}
  ```

  Becomes this in 0.6 using `State`:

  ```rust
  use axum::{routing::get, extract::State, Router};

  let app = Router::with_state(AppState {})
      .route("/", get(handler));

  async fn handler(State(app_state): State<AppState>) {}

  #[derive(Clone)]
  struct AppState {}
  ```

  If you have multiple extensions you can use fields on `AppState` and implement
  `FromRef`:

  ```rust
  use axum::{extract::{State, FromRef}, routing::get, Router};

  let state = AppState {
      client: HttpClient {},
      database: Database {},
  };

  let app = Router::with_state(state).route("/", get(handler));

  async fn handler(
      State(client): State<HttpClient>,
      State(database): State<Database>,
  ) {}

  #[derive(Clone)]
  struct AppState {
      client: HttpClient,
      database: Database,
  }

  #[derive(Clone)]
  struct HttpClient {}

  impl FromRef<AppState> for HttpClient {
      fn from_ref(state: &AppState) -> Self {
          state.client.clone()
      }
  }

  #[derive(Clone)]
  struct Database {}

  impl FromRef<AppState> for Database {
      fn from_ref(state: &AppState) -> Self {
          state.database.clone()
      }
  }
  ```
- **breaking:** It is now only possible for one extractor per handler to consume
  the request body. In 0.5 doing so would result in runtime errors but in 0.6 it
  is a compile error ([#1272])

  axum enforces this by only allowing the _last_ extractor to consume the
  request.

  For example:

  ```rust
  use axum::{Json, http::HeaderMap};

  // This won't compile on 0.6 because both `Json` and `String` need to consume
  // the request body. You can use either `Json` or `String`, but not both.
  async fn handler_1(
      json: Json<serde_json::Value>,
      string: String,
  ) {}

  // This won't work either since `Json` is not the last extractor.
  async fn handler_2(
      json: Json<serde_json::Value>,
      headers: HeaderMap,
  ) {}

  // This works!
  async fn handler_3(
      headers: HeaderMap,
      json: Json<serde_json::Value>,
  ) {}
  ```

  This is done by reworking the `FromRequest` trait and introducing a new
  `FromRequestParts` trait.

  If your extractor needs to consume the request body then you should implement
  `FromRequest`, otherwise implement `FromRequestParts`.

  This extractor in 0.5:

  ```rust
  struct MyExtractor { /* ... */ }

  impl<B> FromRequest<B> for MyExtractor
  where
      B: Send,
  {
      type Rejection = StatusCode;

      async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
          // ...
      }
  }
  ```

  Becomes this in 0.6:

  ```rust
  use axum::{
      extract::{FromRequest, FromRequestParts},
      http::{StatusCode, Request, request::Parts},
  };

  struct MyExtractor { /* ... */ }

  // implement `FromRequestParts` if you don't need to consume the request body
  impl<S> FromRequestParts<S> for MyExtractor
  where
      S: Send + Sync,
  {
      type Rejection = StatusCode;

      async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
          // ...
      }
  }

  // implement `FromRequest` if you do need to consume the request body
  impl<S, B> FromRequest<S, B> for MyExtractor
  where
      S: Send + Sync,
      B: Send + 'static,
  {
      type Rejection = StatusCode;

      async fn from_request(req: Request<B>, state: &S) -> Result<Self, Self::Rejection> {
          // ...
      }
  }
  ```

- **breaking:** `RequestParts` has been removed as part of the `FromRequest`
  rework ([#1272])
- **breaking:** `BodyAlreadyExtracted` has been removed ([#1272])
- **breaking:** The following types or traits have a new `S` type param
  which represents the state ([#1155]):
  - `Router`, defaults to `()`
  - `MethodRouter`, defaults to `()`
  - `FromRequest`, no default
  - `Handler`, no default
- **added:** Add `RequestExt` and `RequestPartsExt` which adds convenience
  methods for running extractors to `http::Request` and `http::request::Parts` ([#1301])

## Middleware

- **breaking:** Remove `extractor_middleware` which was previously deprecated.
  Use `axum::middleware::from_extractor` instead ([#1077])
- **added:** Support running extractors on `middleware::from_fn` functions ([#1088])
- **added:** Support any middleware response that implements `IntoResponse` ([#1152])
- **breaking:** Require middleware added with `Handler::layer` to have
  `Infallible` as the error type ([#1152])

## Misc

- **changed:** axum's MSRV is now 1.60 ([#1239])
- **changed:** For methods that accept some `S: Service`, the bounds have been
  relaxed so the response type must implement `IntoResponse` rather than being a
  literal `Response`
- **fixed:** Annotate panicking functions with `#[track_caller]` so the error
  message points to where the user added the invalid route, rather than
  somewhere internally in axum ([#1248])
- **added:** Add `ServiceExt` with methods for turning any `Service` into a
  `MakeService` similarly to `Router::into_make_service` ([#1302])

[#1077]: https://github.com/tokio-rs/axum/pull/1077
[#1086]: https://github.com/tokio-rs/axum/pull/1086
[#1088]: https://github.com/tokio-rs/axum/pull/1088
[#1102]: https://github.com/tokio-rs/axum/pull/1102
[#1119]: https://github.com/tokio-rs/axum/pull/1119
[#1152]: https://github.com/tokio-rs/axum/pull/1152
[#1155]: https://github.com/tokio-rs/axum/pull/1155
[#1239]: https://github.com/tokio-rs/axum/pull/1239
[#1248]: https://github.com/tokio-rs/axum/pull/1248
[#1272]: https://github.com/tokio-rs/axum/pull/1272
[#1301]: https://github.com/tokio-rs/axum/pull/1301
[#1302]: https://github.com/tokio-rs/axum/pull/1302
[#924]: https://github.com/tokio-rs/axum/pull/924

</details>

# 0.5.16 (10. September, 2022)

## Security

- **breaking:** Added default limit to how much data `Bytes::from_request` will
  consume. Previously it would attempt to consume the entire request body
  without checking its length. This meant if a malicious peer sent an large (or
  infinite) request body your server might run out of memory and crash.

  The default limit is at 2 MB and can be disabled by adding the new
  `DefaultBodyLimit::disable()` middleware. See its documentation for more
  details.

  This also applies to these extractors which used `Bytes::from_request`
  internally:
  - `Form`
  - `Json`
  - `String`

  ([#1346])

[#1346]: https://github.com/tokio-rs/axum/pull/1346

# 0.5.15 (9. August, 2022)

- **fixed:** Don't expose internal type names in `QueryRejection` response. ([#1171])
- **fixed:** Improve performance of JSON serialization ([#1178])
- **fixed:** Improve build times by generating less IR ([#1192])

[#1171]: https://github.com/tokio-rs/axum/pull/1171
[#1178]: https://github.com/tokio-rs/axum/pull/1178
[#1192]: https://github.com/tokio-rs/axum/pull/1192

# 0.5.14 (25. July, 2022)

Yanked, as it contained an accidental breaking change.

# 0.5.13 (15. July, 2022)

- **fixed:** If `WebSocketUpgrade` cannot upgrade the connection it will return a
  `WebSocketUpgradeRejection::ConnectionNotUpgradable` rejection ([#1135])
- **changed:** `WebSocketUpgradeRejection` has a new variant `ConnectionNotUpgradable`
  variant ([#1135])

[#1135]: https://github.com/tokio-rs/axum/pull/1135

# 0.5.12 (10. July, 2022)

- **added:** Added `debug_handler` which is an attribute macro that improves
  type errors when applied to handler function. It is re-exported from
  `axum-macros` ([#1144])

[#1144]: https://github.com/tokio-rs/axum/pull/1144

# 0.5.11 (02. July, 2022)

- **added:** Implement `TryFrom<http::Method>` for `MethodFilter` and use new
  `NoMatchingMethodFilter` error in case of failure ([#1130])
- **added:** Document how to run extractors from middleware ([#1140])

[#1130]: https://github.com/tokio-rs/axum/pull/1130
[#1140]: https://github.com/tokio-rs/axum/pull/1140

# 0.5.10 (28. June, 2022)

- **fixed:** Make `Router` cheaper to clone ([#1123])
- **fixed:** Fix possible panic when doing trailing slash redirect ([#1124])

[#1123]: https://github.com/tokio-rs/axum/pull/1123
[#1124]: https://github.com/tokio-rs/axum/pull/1124

# 0.5.9 (20. June, 2022)

- **fixed:** Fix compile error when the `headers` is enabled and the `form`
  feature is disabled ([#1107])

[#1107]: https://github.com/tokio-rs/axum/pull/1107

# 0.5.8 (18. June, 2022)

- **added:** Support resolving host name via `Forwarded` header in `Host`
  extractor ([#1078])
- **added:** Implement `IntoResponse` for `Form` ([#1095])
- **changed:** axum's MSRV is now 1.56 ([#1098])

[#1078]: https://github.com/tokio-rs/axum/pull/1078
[#1095]: https://github.com/tokio-rs/axum/pull/1095
[#1098]: https://github.com/tokio-rs/axum/pull/1098

# 0.5.7 (08. June, 2022)

- **added:** Implement `Default` for `Extension` ([#1043])
- **fixed:** Support deserializing `Vec<(String, String)>` in `extract::Path<_>` to get vector of
  key/value pairs ([#1059])
- **added:** Add `extract::ws::close_code` which contains constants for close codes ([#1067])
- **fixed:** Use `impl IntoResponse` less in docs ([#1049])

[#1043]: https://github.com/tokio-rs/axum/pull/1043
[#1049]: https://github.com/tokio-rs/axum/pull/1049
[#1059]: https://github.com/tokio-rs/axum/pull/1059
[#1067]: https://github.com/tokio-rs/axum/pull/1067

# 0.5.6 (15. May, 2022)

- **added:** Add `WebSocket::protocol` to return the selected WebSocket subprotocol, if there is one. ([#1022])
- **fixed:** Improve error message for `PathRejection::WrongNumberOfParameters` to hint at using
  `Path<(String, String)>` or `Path<SomeStruct>` ([#1023])
- **fixed:** `PathRejection::WrongNumberOfParameters` now uses `500 Internal Server Error` since
  it's a programmer error and not a client error ([#1023])
- **fixed:** Fix `InvalidFormContentType` mentioning the wrong content type

[#1022]: https://github.com/tokio-rs/axum/pull/1022
[#1023]: https://github.com/tokio-rs/axum/pull/1023

# 0.5.5 (10. May, 2022)

- **fixed:** Correctly handle `GET`, `HEAD`, and `OPTIONS` requests in `ContentLengthLimit`.
  Request with these methods are now accepted if they _do not_ have a `Content-Length` header, and
  the request body will not be checked. If they do have a `Content-Length` header they'll be
  rejected. This allows `ContentLengthLimit` to be used as middleware around several routes,
  including `GET` routes ([#989])
- **added:** Add `MethodRouter::{into_make_service, into_make_service_with_connect_info}` ([#1010])

[#989]: https://github.com/tokio-rs/axum/pull/989
[#1010]: https://github.com/tokio-rs/axum/pull/1010

# 0.5.4 (26. April, 2022)

- **added:** Add `response::ErrorResponse` and `response::Result` for
  `IntoResponse`-based error handling ([#921])
- **added:** Add `middleware::from_extractor` and deprecate `extract::extractor_middleware` ([#957])
- **changed:** Update to tower-http 0.3 ([#965])

[#921]: https://github.com/tokio-rs/axum/pull/921
[#957]: https://github.com/tokio-rs/axum/pull/957
[#965]: https://github.com/tokio-rs/axum/pull/965

# 0.5.3 (19. April, 2022)

- **added:** Add `AppendHeaders` for appending headers to a response rather than overriding them ([#927])
- **added:** Add `axum::extract::multipart::Field::chunk` method for streaming a single chunk from
  the field ([#901])
- **fixed:** Fix trailing slash redirection with query parameters ([#936])

[#901]: https://github.com/tokio-rs/axum/pull/901
[#927]: https://github.com/tokio-rs/axum/pull/927
[#936]: https://github.com/tokio-rs/axum/pull/936

# 0.5.2 (19. April, 2022)

Yanked, as it contained an accidental breaking change.

# 0.5.1 (03. April, 2022)

- **added:** Add `RequestParts::extract` which allows applying an extractor as a method call ([#897])

[#897]: https://github.com/tokio-rs/axum/pull/897

# 0.5.0 (31. March, 2022)

- **added:** Document sharing state between handler and middleware ([#783])
- **added:** `Extension<_>` can now be used in tuples for building responses, and will set an
  extension on the response ([#797])
- **added:** `extract::Host` for extracting the hostname of a request ([#827])
- **added:** Add `IntoResponseParts` trait which allows defining custom response
  types for adding headers or extensions to responses ([#797])
- **added:** `TypedHeader` implements the new `IntoResponseParts` trait so they
  can be returned from handlers as parts of a response ([#797])
- **changed:** `Router::merge` now accepts `Into<Router>` ([#819])
- **breaking:** `sse::Event` now accepts types implementing `AsRef<str>` instead of `Into<String>`
  as field values.
- **breaking:** `sse::Event` now panics if a setter method is called twice instead of silently
  overwriting old values.
- **breaking:** Require `Output = ()` on `WebSocketStream::on_upgrade` ([#644])
- **breaking:** Make `TypedHeaderRejectionReason` `#[non_exhaustive]` ([#665])
- **breaking:** Using `HeaderMap` as an extractor will no longer remove the headers and thus
  they'll still be accessible to other extractors, such as `axum::extract::Json`. Instead
  `HeaderMap` will clone the headers. You should prefer to use `TypedHeader` to extract only the
  headers you need ([#698])

  This includes these breaking changes:
    - `RequestParts::take_headers` has been removed.
    - `RequestParts::headers` returns `&HeaderMap`.
    - `RequestParts::headers_mut` returns `&mut HeaderMap`.
    - `HeadersAlreadyExtracted` has been removed.
    - The `HeadersAlreadyExtracted` variant has been removed from these rejections:
        - `RequestAlreadyExtracted`
        - `RequestPartsAlreadyExtracted`
        - `JsonRejection`
        - `FormRejection`
        - `ContentLengthLimitRejection`
        - `WebSocketUpgradeRejection`
    - `<HeaderMap as FromRequest<_>>::Rejection` has been changed to `std::convert::Infallible`.
- **breaking:** `axum::http::Extensions` is no longer an extractor (ie it
  doesn't implement `FromRequest`). The `axum::extract::Extension` extractor is
  _not_ impacted by this and works the same. This change makes it harder to
  accidentally remove all extensions which would result in confusing errors
  elsewhere ([#699])
  This includes these breaking changes:
    - `RequestParts::take_extensions` has been removed.
    - `RequestParts::extensions` returns `&Extensions`.
    - `RequestParts::extensions_mut` returns `&mut Extensions`.
    - `RequestAlreadyExtracted` has been removed.
    - `<Request as FromRequest>::Rejection` is now `BodyAlreadyExtracted`.
    - `<http::request::Parts as FromRequest>::Rejection` is now `Infallible`.
    - `ExtensionsAlreadyExtracted` has been removed.
    - The `ExtensionsAlreadyExtracted` removed variant has been removed from these rejections:
        - `ExtensionRejection`
        - `PathRejection`
        - `MatchedPathRejection`
        - `WebSocketUpgradeRejection`
- **breaking:** `Redirect::found` has been removed ([#800])
- **breaking:** `AddExtensionLayer` has been removed. Use `Extension` instead. It now implements
  `tower::Layer` ([#807])
- **breaking:** `AddExtension` has been moved from the root module to `middleware`
- **breaking:** `.nest("/foo/", Router::new().route("/bar", _))` now does the right thing and
  results in a route at `/foo/bar` instead of `/foo//bar` ([#824])
- **breaking:** Routes are now required to start with `/`. Previously routes such as `:foo` would
  be accepted but most likely result in bugs ([#823])
- **breaking:** `Headers` has been removed. Arrays of tuples directly implement
  `IntoResponseParts` so `([("x-foo", "foo")], response)` now works ([#797])
- **breaking:** `InvalidJsonBody` has been replaced with `JsonDataError` to clearly signal that the
  request body was syntactically valid JSON but couldn't be deserialized into the target type
- **breaking:** `Handler` is no longer an `#[async_trait]` but instead has an
  associated `Future` type. That allows users to build their own `Handler` types
  without paying the cost of `#[async_trait]` ([#879])
- **changed:** New `JsonSyntaxError` variant added to `JsonRejection`. This is returned when the
  request body contains syntactically invalid JSON
- **fixed:** Correctly set the `Content-Length` header for response to `HEAD`
  requests ([#734])
- **fixed:** Fix wrong `content-length` for `HEAD` requests to endpoints that returns chunked
  responses ([#755])
- **fixed:** Fixed several routing bugs related to nested "opaque" tower services (i.e.
  non-`Router` services) ([#841] and [#842])
- **changed:** Update to tokio-tungstenite 0.17 ([#791])
- **breaking:** `Redirect::{to, temporary, permanent}` now accept `&str` instead
  of `Uri` ([#889])
- **breaking:** Remove second type parameter from `Router::into_make_service_with_connect_info`
  and `Handler::into_make_service_with_connect_info` to support `MakeService`s
  that accept multiple targets ([#892])

[#644]: https://github.com/tokio-rs/axum/pull/644
[#665]: https://github.com/tokio-rs/axum/pull/665
[#698]: https://github.com/tokio-rs/axum/pull/698
[#699]: https://github.com/tokio-rs/axum/pull/699
[#734]: https://github.com/tokio-rs/axum/pull/734
[#755]: https://github.com/tokio-rs/axum/pull/755
[#783]: https://github.com/tokio-rs/axum/pull/783
[#791]: https://github.com/tokio-rs/axum/pull/791
[#797]: https://github.com/tokio-rs/axum/pull/797
[#800]: https://github.com/tokio-rs/axum/pull/800
[#807]: https://github.com/tokio-rs/axum/pull/807
[#819]: https://github.com/tokio-rs/axum/pull/819
[#823]: https://github.com/tokio-rs/axum/pull/823
[#824]: https://github.com/tokio-rs/axum/pull/824
[#827]: https://github.com/tokio-rs/axum/pull/827
[#841]: https://github.com/tokio-rs/axum/pull/841
[#842]: https://github.com/tokio-rs/axum/pull/842
[#879]: https://github.com/tokio-rs/axum/pull/879
[#889]: https://github.com/tokio-rs/axum/pull/889
[#892]: https://github.com/tokio-rs/axum/pull/892

# 0.4.8 (2. March, 2022)

- Use correct path for `AddExtensionLayer` and `AddExtension::layer` deprecation
  notes ([#812])

[#812]: https://github.com/tokio-rs/axum/pull/812

# 0.4.7 (1. March, 2022)

- **added:** Implement `tower::Layer` for `Extension` ([#801])
- **changed:** Deprecate `AddExtensionLayer`. Use `Extension` instead ([#805])

[#801]: https://github.com/tokio-rs/axum/pull/801
[#805]: https://github.com/tokio-rs/axum/pull/805

# 0.4.6 (22. February, 2022)

- **added:** `middleware::from_fn` for creating middleware from async functions.
  This previously lived in axum-extra but has been moved to axum ([#719])
- **fixed:** Set `Allow` header when responding with `405 Method Not Allowed` ([#733])

[#719]: https://github.com/tokio-rs/axum/pull/719
[#733]: https://github.com/tokio-rs/axum/pull/733

# 0.4.5 (31. January, 2022)

- Reference [axum-macros] instead of [axum-debug]. The latter has been superseded by
  axum-macros and is deprecated ([#738])

[#738]: https://github.com/tokio-rs/axum/pull/738
[axum-debug]: https://docs.rs/axum-debug
[axum-macros]: https://docs.rs/axum-macros

# 0.4.4 (13. January, 2022)

- **fixed:** Fix using incorrect path prefix when nesting `Router`s at `/` ([#691])
- **fixed:** Make `nest("", service)` work and mean the same as `nest("/", service)` ([#691])
- **fixed:** Replace response code `301` with `308` for trailing slash redirects. Also deprecates
  `Redirect::found` (`302`) in favor of `Redirect::temporary` (`307`) or `Redirect::to` (`303`).
  This is to prevent clients from changing non-`GET` requests to `GET` requests ([#682])

[#691]: https://github.com/tokio-rs/axum/pull/691
[#682]: https://github.com/tokio-rs/axum/pull/682

# 0.4.3 (21. December, 2021)

- **added:** `axum::AddExtension::layer` ([#607])
- **added:** Re-export the headers crate when the headers feature is active ([#630])
- **fixed:** `sse::Event` will no longer drop the leading space of data, event ID and name values
  that have it ([#600])
- **fixed:** `sse::Event` is more strict about what field values it supports, disallowing any SSE
  events that break the specification (such as field values containing carriage returns) ([#599])
- **fixed:** Improve documentation of `sse::Event` ([#601])
- **fixed:** Make `Path` fail with `ExtensionsAlreadyExtracted` if another extractor (such as
  `Request`) has previously taken the request extensions. Thus `PathRejection` now contains a
  variant with `ExtensionsAlreadyExtracted`. This is not a breaking change since `PathRejection` is
  marked as `#[non_exhaustive]` ([#619])
- **fixed:** Fix misleading error message for `PathRejection` if extensions had
  previously been extracted ([#619])
- **fixed:** Use `AtomicU32` internally, rather than `AtomicU64`, to improve portability ([#616])

[#599]: https://github.com/tokio-rs/axum/pull/599
[#600]: https://github.com/tokio-rs/axum/pull/600
[#601]: https://github.com/tokio-rs/axum/pull/601
[#607]: https://github.com/tokio-rs/axum/pull/607
[#616]: https://github.com/tokio-rs/axum/pull/616
[#619]: https://github.com/tokio-rs/axum/pull/619
[#619]: https://github.com/tokio-rs/axum/pull/619
[#630]: https://github.com/tokio-rs/axum/pull/630

# 0.4.2 (06. December, 2021)

- **fix:** Depend on the correct version of `axum-core` ([#592])

[#592]: https://github.com/tokio-rs/axum/pull/592

# 0.4.1 (06. December, 2021)

- **added:** `axum::response::Response` now exists as a shorthand for writing `Response<BoxBody>` ([#590])

[#590]: https://github.com/tokio-rs/axum/pull/590

# 0.4.0 (02. December, 2021)

- **breaking:** New `MethodRouter` that works similarly to `Router`:
  - Route to handlers and services with the same type
  - Add middleware to some routes more easily with `MethodRouter::layer` and
    `MethodRouter::route_layer`.
  - Merge method routers with `MethodRouter::merge`
  - Customize response for unsupported methods with `MethodRouter::fallback`
- **breaking:** The default for the type parameter in `FromRequest` and
  `RequestParts` has been removed. Use `FromRequest<Body>` and
  `RequestParts<Body>` to get the previous behavior ([#564])
- **added:** `FromRequest` and `IntoResponse` are now defined in a new called
  `axum-core`. This crate is intended for library authors to depend on, rather
  than `axum` itself, if possible. `axum-core` has a smaller API and will thus
  receive fewer breaking changes. `FromRequest` and `IntoResponse` are
  re-exported from `axum` in the same location so nothing is changed for `axum`
  users ([#564])
- **breaking:** The previously deprecated `axum::body::box_body` function has
  been removed. Use `axum::body::boxed` instead.
- **fixed:** Adding the same route with different methods now works ie
  `.route("/", get(_)).route("/", post(_))`.
- **breaking:** `routing::handler_method_router` and
  `routing::service_method_router` has been removed in favor of
  `routing::{get, get_service, ..., MethodRouter}`.
- **breaking:** `HandleErrorExt` has been removed in favor of
  `MethodRouter::handle_error`.
- **breaking:** `HandleErrorLayer` now requires the handler function to be
  `async` ([#534])
- **added:** `HandleErrorLayer` now supports running extractors.
- **breaking:** The `Handler<B, T>` trait is now defined as `Handler<T, B =
  Body>`. That is the type parameters have been swapped and `B` defaults to
  `axum::body::Body` ([#527])
- **breaking:** `Router::merge` will panic if both routers have fallbacks.
  Previously the left side fallback would be silently discarded ([#529])
- **breaking:** `Router::nest` will panic if the nested router has a fallback.
  Previously it would be silently discarded ([#529])
- Update WebSockets to use tokio-tungstenite 0.16 ([#525])
- **added:** Default to return `charset=utf-8` for text content type. ([#554])
- **breaking:** The `Body` and `BodyError` associated types on the
  `IntoResponse` trait have been removed - instead, `.into_response()` will now
  always return `Response<BoxBody>` ([#571])
- **breaking:** `PathParamsRejection` has been renamed to `PathRejection` and its
  variants renamed to `FailedToDeserializePathParams` and `MissingPathParams`. This
  makes it more consistent with the rest of axum ([#574])
- **added:** `Path`'s rejection type now provides data about exactly which part of
  the path couldn't be deserialized ([#574])

[#525]: https://github.com/tokio-rs/axum/pull/525
[#527]: https://github.com/tokio-rs/axum/pull/527
[#529]: https://github.com/tokio-rs/axum/pull/529
[#534]: https://github.com/tokio-rs/axum/pull/534
[#554]: https://github.com/tokio-rs/axum/pull/554
[#564]: https://github.com/tokio-rs/axum/pull/564
[#571]: https://github.com/tokio-rs/axum/pull/571
[#574]: https://github.com/tokio-rs/axum/pull/574

# 0.3.4 (13. November, 2021)

- **changed:** `box_body` has been renamed to `boxed`. `box_body` still exists
  but is deprecated ([#530])

[#530]: https://github.com/tokio-rs/axum/pull/530

# 0.3.3 (13. November, 2021)

- Implement `FromRequest` for [`http::request::Parts`] so it can be used an
  extractor ([#489])
- Implement `IntoResponse` for `http::response::Parts` ([#490])

[#489]: https://github.com/tokio-rs/axum/pull/489
[#490]: https://github.com/tokio-rs/axum/pull/490
[`http::request::Parts`]: https://docs.rs/http/latest/http/request/struct.Parts.html

# 0.3.2 (08. November, 2021)

- **added:** Add `Router::route_layer` for applying middleware that
  will only run on requests that match a route. This is useful for middleware
  that return early, such as authorization ([#474])

[#474]: https://github.com/tokio-rs/axum/pull/474

# 0.3.1 (06. November, 2021)

- **fixed:** Implement `Clone` for `IntoMakeServiceWithConnectInfo` ([#471])

[#471]: https://github.com/tokio-rs/axum/pull/471

# 0.3.0 (02. November, 2021)

- Overall:
  - **fixed:** All known compile time issues are resolved, including those with
    `boxed` and those introduced by Rust 1.56 ([#404])
  - **breaking:** The router's type is now always `Router` regardless of how many routes or
    middleware are applied ([#404])

    This means router types are all always nameable:

    ```rust
    fn my_routes() -> Router {
        Router::new().route(
            "/users",
            post(|| async { "Hello, World!" }),
        )
    }
    ```
  - **breaking:** Added feature flags for HTTP1 and JSON. This enables removing a
    few dependencies if your app only uses HTTP2 or doesn't use JSON. This is only a
    breaking change if you depend on axum with `default_features = false`. ([#286])
  - **breaking:** `Route::boxed` and `BoxRoute` have been removed as they're no longer
    necessary ([#404])
  - **breaking:** `Nested`, `Or` types are now private. They no longer had to be
    public because `Router` is internally boxed ([#404])
  - **breaking:** Remove `routing::Layered` as it didn't actually do anything and
    thus wasn't necessary
  - **breaking:** Vendor `AddExtensionLayer` and `AddExtension` to reduce public
    dependencies
  - **breaking:** `body::BoxBody` is now a type alias for
    `http_body::combinators::UnsyncBoxBody` and thus is no longer `Sync`. This
    is because bodies are streams and requiring streams to be `Sync` is
    unnecessary.
  - **added:** Implement `IntoResponse` for `http_body::combinators::UnsyncBoxBody`.
  - **added:** Add `Handler::into_make_service` for serving a handler without a
    `Router`.
  - **added:** Add `Handler::into_make_service_with_connect_info` for serving a
    handler without a `Router`, and storing info about the incoming connection.
  - **breaking:** axum's minimum supported rust version is now 1.56
- Routing:
  - Big internal refactoring of routing leading to several improvements ([#363])
    - **added:** Wildcard routes like `.route("/api/users/*rest", service)` are now supported.
    - **fixed:** The order routes are added in no longer matters.
    - **fixed:** Adding a conflicting route will now cause a panic instead of silently making
      a route unreachable.
    - **fixed:** Route matching is faster as number of routes increases.
    - **breaking:** Handlers for multiple HTTP methods must be added in the same
      `Router::route` call. So `.route("/", get(get_handler).post(post_handler))` and
      _not_ `.route("/", get(get_handler)).route("/", post(post_handler))`.
  - **fixed:** Correctly handle trailing slashes in routes:
    - If a route with a trailing slash exists and a request without a trailing
      slash is received, axum will send a 301 redirection to the route with the
      trailing slash.
    - Or vice versa if a route without a trailing slash exists and a request
      with a trailing slash is received.
    - This can be overridden by explicitly defining two routes: One with and one
      without a trailing slash.
  - **breaking:** Method routing for handlers has been moved from `axum::handler`
    to `axum::routing`. So `axum::handler::get` now lives at `axum::routing::get`
    ([#405])
  - **breaking:** Method routing for services has been moved from `axum::service`
    to `axum::routing::service_method_routing`. So `axum::service::get` now lives at
    `axum::routing::service_method_routing::get`, etc. ([#405])
  - **breaking:** `Router::or` renamed to `Router::merge` and will now panic on
    overlapping routes. It now only accepts `Router`s and not general `Service`s.
    Use `Router::fallback` for adding fallback routes ([#408])
  - **added:** `Router::fallback` for adding handlers for request that didn't
    match any routes. `Router::fallback` must be use instead of `nest("/", _)` ([#408])
  - **breaking:** `EmptyRouter` has been renamed to `MethodNotAllowed` as it's only
    used in method routers and not in path routers (`Router`)
  - **breaking:** Remove support for routing based on the `CONNECT` method. An
    example of combining axum with and HTTP proxy can be found [here][proxy] ([#428])
- Extractors:
  - **fixed:** Expand accepted content types for JSON requests ([#378])
  - **fixed:** Support deserializing `i128` and `u128` in `extract::Path`
  - **breaking:** Automatically do percent decoding in `extract::Path`
    ([#272])
  - **breaking:** Change `Connected::connect_info` to return `Self` and remove
    the associated type `ConnectInfo` ([#396])
  - **added:** Add `extract::MatchedPath` for accessing path in router that
    matched the request ([#412])
- Error handling:
  - **breaking:** Simplify error handling model ([#402]):
    - All services part of the router are now required to be infallible.
    - Error handling utilities have been moved to an `error_handling` module.
    - `Router::check_infallible` has been removed since routers are always
      infallible with the error handling changes.
    - Error handling closures must now handle all errors and thus always return
      something that implements `IntoResponse`.

    With these changes handling errors from fallible middleware is done like so:

    ```rust,no_run
    use axum::{
        routing::get,
        http::StatusCode,
        error_handling::HandleErrorLayer,
        response::IntoResponse,
        Router, BoxError,
    };
    use tower::ServiceBuilder;
    use std::time::Duration;

    let middleware_stack = ServiceBuilder::new()
        // Handle errors from middleware
        //
        // This middleware most be added above any fallible
        // ones if you're using `ServiceBuilder`, due to how ordering works
        .layer(HandleErrorLayer::new(handle_error))
        // Return an error after 30 seconds
        .timeout(Duration::from_secs(30));

    let app = Router::new()
        .route("/", get(|| async { /* ... */ }))
        .layer(middleware_stack);

    fn handle_error(_error: BoxError) -> impl IntoResponse {
        StatusCode::REQUEST_TIMEOUT
    }
    ```

    And handling errors from fallible leaf services is done like so:

    ```rust
    use axum::{
        Router, service,
        body::Body,
        routing::service_method_routing::get,
        response::IntoResponse,
        http::{Request, Response},
        error_handling::HandleErrorExt, // for `.handle_error`
    };
    use std::{io, convert::Infallible};
    use tower::service_fn;

    let app = Router::new()
        .route(
            "/",
            get(service_fn(|_req: Request<Body>| async {
                let contents = tokio::fs::read_to_string("some_file").await?;
                Ok::<_, io::Error>(Response::new(Body::from(contents)))
            }))
            .handle_error(handle_io_error),
        );

    fn handle_io_error(error: io::Error) -> impl IntoResponse {
        // ...
    }
    ```
- Misc:
  - `InvalidWebsocketVersionHeader` has been renamed to `InvalidWebSocketVersionHeader` ([#416])
  - `WebsocketKeyHeaderMissing` has been renamed to `WebSocketKeyHeaderMissing` ([#416])

[#339]: https://github.com/tokio-rs/axum/pull/339
[#286]: https://github.com/tokio-rs/axum/pull/286
[#272]: https://github.com/tokio-rs/axum/pull/272
[#378]: https://github.com/tokio-rs/axum/pull/378
[#363]: https://github.com/tokio-rs/axum/pull/363
[#396]: https://github.com/tokio-rs/axum/pull/396
[#402]: https://github.com/tokio-rs/axum/pull/402
[#404]: https://github.com/tokio-rs/axum/pull/404
[#405]: https://github.com/tokio-rs/axum/pull/405
[#408]: https://github.com/tokio-rs/axum/pull/408
[#412]: https://github.com/tokio-rs/axum/pull/412
[#416]: https://github.com/tokio-rs/axum/pull/416
[#428]: https://github.com/tokio-rs/axum/pull/428
[proxy]: https://github.com/tokio-rs/axum/blob/main/examples/http-proxy/src/main.rs

# 0.2.8 (07. October, 2021)

- Document debugging handler type errors with "axum-debug" ([#372])

[#372]: https://github.com/tokio-rs/axum/pull/372

# 0.2.7 (06. October, 2021)

- Bump minimum version of async-trait ([#370])

[#370]: https://github.com/tokio-rs/axum/pull/370

# 0.2.6 (02. October, 2021)

- Clarify that `handler::any` and `service::any` only accepts standard HTTP
  methods ([#337])
- Document how to customize error responses from extractors ([#359])

[#337]: https://github.com/tokio-rs/axum/pull/337
[#359]: https://github.com/tokio-rs/axum/pull/359

# 0.2.5 (18. September, 2021)

- Add accessors for `TypedHeaderRejection` fields ([#317])
- Improve docs for extractors ([#327])

[#317]: https://github.com/tokio-rs/axum/pull/317
[#327]: https://github.com/tokio-rs/axum/pull/327

# 0.2.4 (10. September, 2021)

- Document using `StreamExt::split` with `WebSocket` ([#291])
- Document adding middleware to multiple groups of routes ([#293])

[#291]: https://github.com/tokio-rs/axum/pull/291
[#293]: https://github.com/tokio-rs/axum/pull/293

# 0.2.3 (26. August, 2021)

- **fixed:** Fix accidental breaking change introduced by internal refactor.
  `BoxRoute` used to be `Sync` but was accidental made `!Sync` ([#273](https://github.com/tokio-rs/axum/pull/273))

# 0.2.2 (26. August, 2021)

- **fixed:** Fix URI captures matching empty segments. This means requests with
  URI `/` will no longer be matched by `/:key` ([#264](https://github.com/tokio-rs/axum/pull/264))
- **fixed:** Remove needless trait bounds from `Router::boxed` ([#269](https://github.com/tokio-rs/axum/pull/269))

# 0.2.1 (24. August, 2021)

- **added:** Add `Redirect::to` constructor ([#255](https://github.com/tokio-rs/axum/pull/255))
- **added:** Document how to implement `IntoResponse` for custom error type ([#258](https://github.com/tokio-rs/axum/pull/258))

# 0.2.0 (23. August, 2021)

- Overall:
  - **fixed:** Overall compile time improvements. If you're having issues with compile time
    please file an issue! ([#184](https://github.com/tokio-rs/axum/pull/184)) ([#198](https://github.com/tokio-rs/axum/pull/198)) ([#220](https://github.com/tokio-rs/axum/pull/220))
  - **changed:** Remove `prelude`. Explicit imports are now required ([#195](https://github.com/tokio-rs/axum/pull/195))
- Routing:
  - **added:** Add dedicated `Router` to replace the `RoutingDsl` trait ([#214](https://github.com/tokio-rs/axum/pull/214))
  - **added:** Add `Router::or` for combining routes ([#108](https://github.com/tokio-rs/axum/pull/108))
  - **fixed:** Support matching different HTTP methods for the same route that aren't defined
    together. So `Router::new().route("/", get(...)).route("/", post(...))` now
    accepts both `GET` and `POST`. Previously only `POST` would be accepted ([#224](https://github.com/tokio-rs/axum/pull/224))
  - **fixed:** `get` routes will now also be called for `HEAD` requests but will always have
    the response body removed ([#129](https://github.com/tokio-rs/axum/pull/129))
  - **changed:** Replace `axum::route(...)` with `axum::Router::new().route(...)`. This means
    there is now only one way to create a new router. Same goes for
    `axum::routing::nest`. ([#215](https://github.com/tokio-rs/axum/pull/215))
  - **changed:** Implement `routing::MethodFilter` via [`bitflags`](https://crates.io/crates/bitflags) ([#158](https://github.com/tokio-rs/axum/pull/158))
  - **changed:** Move `handle_error` from `ServiceExt` to `service::OnMethod` ([#160](https://github.com/tokio-rs/axum/pull/160))

  With these changes this app using 0.1:

  ```rust
  use axum::{extract::Extension, prelude::*, routing::BoxRoute, AddExtensionLayer};

  let app = route("/", get(|| async { "hi" }))
      .nest("/api", api_routes())
      .layer(AddExtensionLayer::new(state));

  fn api_routes() -> BoxRoute<Body> {
      route(
          "/users",
          post(|Extension(state): Extension<State>| async { "hi from nested" }),
      )
      .boxed()
  }
  ```

  Becomes this in 0.2:

  ```rust
  use axum::{
      extract::Extension,
      handler::{get, post},
      routing::BoxRoute,
      Router,
  };

  let app = Router::new()
      .route("/", get(|| async { "hi" }))
      .nest("/api", api_routes());

  fn api_routes() -> Router<BoxRoute> {
      Router::new()
          .route(
              "/users",
              post(|Extension(state): Extension<State>| async { "hi from nested" }),
          )
          .boxed()
  }
  ```
- Extractors:
  - **added:** Make `FromRequest` default to being generic over `body::Body` ([#146](https://github.com/tokio-rs/axum/pull/146))
  - **added:** Implement `std::error::Error` for all rejections ([#153](https://github.com/tokio-rs/axum/pull/153))
  - **added:** Add `OriginalUri` for extracting original request URI in nested services ([#197](https://github.com/tokio-rs/axum/pull/197))
  - **added:** Implement `FromRequest` for `http::Extensions` ([#169](https://github.com/tokio-rs/axum/pull/169))
  - **added:** Make `RequestParts::{new, try_into_request}` public so extractors can be used outside axum ([#194](https://github.com/tokio-rs/axum/pull/194))
  - **added:** Implement `FromRequest` for `axum::body::Body` ([#241](https://github.com/tokio-rs/axum/pull/241))
  - **changed:** Removed `extract::UrlParams` and `extract::UrlParamsMap`. Use `extract::Path` instead ([#154](https://github.com/tokio-rs/axum/pull/154))
  - **changed:** `extractor_middleware` now requires `RequestBody: Default` ([#167](https://github.com/tokio-rs/axum/pull/167))
  - **changed:** Convert `RequestAlreadyExtracted` to an enum with each possible error variant ([#167](https://github.com/tokio-rs/axum/pull/167))
  - **changed:** `extract::BodyStream` is no longer generic over the request body ([#234](https://github.com/tokio-rs/axum/pull/234))
  - **changed:** `extract::Body` has been renamed to `extract::RawBody` to avoid conflicting with `body::Body` ([#233](https://github.com/tokio-rs/axum/pull/233))
  - **changed:** `RequestParts` changes ([#153](https://github.com/tokio-rs/axum/pull/153))
      - `method` new returns an `&http::Method`
      - `method_mut` new returns an `&mut http::Method`
      - `take_method` has been removed
      - `uri` new returns an `&http::Uri`
      - `uri_mut` new returns an `&mut http::Uri`
      - `take_uri` has been removed
  - **changed:** Remove several rejection types that were no longer used ([#153](https://github.com/tokio-rs/axum/pull/153)) ([#154](https://github.com/tokio-rs/axum/pull/154))
- Responses:
  - **added:** Add `Headers` for easily customizing headers on a response ([#193](https://github.com/tokio-rs/axum/pull/193))
  - **added:** Add `Redirect` response ([#192](https://github.com/tokio-rs/axum/pull/192))
  - **added:** Add `body::StreamBody` for easily responding with a stream of byte chunks ([#237](https://github.com/tokio-rs/axum/pull/237))
  - **changed:** Add associated `Body` and `BodyError` types to `IntoResponse`. This is
    required for returning responses with bodies other than `hyper::Body` from
    handlers. See the docs for advice on how to implement `IntoResponse` ([#86](https://github.com/tokio-rs/axum/pull/86))
  - **changed:** `tower::util::Either` no longer implements `IntoResponse` ([#229](https://github.com/tokio-rs/axum/pull/229))

  This `IntoResponse` from 0.1:
  ```rust
  use axum::{http::Response, prelude::*, response::IntoResponse};

  struct MyResponse;

  impl IntoResponse for MyResponse {
      fn into_response(self) -> Response<Body> {
          Response::new(Body::empty())
      }
  }
  ```

  Becomes this in 0.2:
  ```rust
  use axum::{body::Body, http::Response, response::IntoResponse};

  struct MyResponse;

  impl IntoResponse for MyResponse {
      type Body = Body;
      type BodyError = <Self::Body as axum::body::HttpBody>::Error;

      fn into_response(self) -> Response<Self::Body> {
          Response::new(Body::empty())
      }
  }
  ```
- SSE:
  - **added:** Add `response::sse::Sse`. This implements SSE using a response rather than a service ([#98](https://github.com/tokio-rs/axum/pull/98))
  - **changed:** Remove `axum::sse`. It has been replaced by `axum::response::sse` ([#98](https://github.com/tokio-rs/axum/pull/98))

  Handler using SSE in 0.1:
  ```rust
  use axum::{
      prelude::*,
      sse::{sse, Event},
  };
  use std::convert::Infallible;

  let app = route(
      "/",
      sse(|| async {
          let stream = futures::stream::iter(vec![Ok::<_, Infallible>(
              Event::default().data("hi there!"),
          )]);
          Ok::<_, Infallible>(stream)
      }),
  );
  ```

  Becomes this in 0.2:

  ```rust
  use axum::{
      handler::get,
      response::sse::{Event, Sse},
      Router,
  };
  use std::convert::Infallible;

  let app = Router::new().route(
      "/",
      get(|| async {
          let stream = futures::stream::iter(vec![Ok::<_, Infallible>(
              Event::default().data("hi there!"),
          )]);
          Sse::new(stream)
      }),
  );
  ```
- WebSockets:
  - **changed:** Change WebSocket API to use an extractor plus a response ([#121](https://github.com/tokio-rs/axum/pull/121))
  - **changed:** Make WebSocket `Message` an enum ([#116](https://github.com/tokio-rs/axum/pull/116))
  - **changed:** `WebSocket` now uses `Error` as its error type ([#150](https://github.com/tokio-rs/axum/pull/150))

  Handler using WebSockets in 0.1:

  ```rust
  use axum::{
      prelude::*,
      ws::{ws, WebSocket},
  };

  let app = route(
      "/",
      ws(|socket: WebSocket| async move {
          // do stuff with socket
      }),
  );
  ```

  Becomes this in 0.2:

  ```rust
  use axum::{
      extract::ws::{WebSocket, WebSocketUpgrade},
      handler::get,
      Router,
  };

  let app = Router::new().route(
      "/",
      get(|ws: WebSocketUpgrade| async move {
          ws.on_upgrade(|socket: WebSocket| async move {
              // do stuff with socket
          })
      }),
  );
  ```
- Misc
  - **added:** Add default feature `tower-log` which exposes `tower`'s `log` feature. ([#218](https://github.com/tokio-rs/axum/pull/218))
  - **changed:** Replace `body::BoxStdError` with `axum::Error`, which supports downcasting ([#150](https://github.com/tokio-rs/axum/pull/150))
  - **changed:** `EmptyRouter` now requires the response body to implement `Send + Sync + 'static'` ([#108](https://github.com/tokio-rs/axum/pull/108))
  - **changed:** `Router::check_infallible` now returns a `CheckInfallible` service. This
    is to improve compile times ([#198](https://github.com/tokio-rs/axum/pull/198))
  - **changed:** `Router::into_make_service` now returns `routing::IntoMakeService` rather than
    `tower::make::Shared` ([#229](https://github.com/tokio-rs/axum/pull/229))
  - **changed:** All usage of `tower::BoxError` has been replaced with `axum::BoxError` ([#229](https://github.com/tokio-rs/axum/pull/229))
  - **changed:** Several response future types have been moved into dedicated
    `future` modules ([#133](https://github.com/tokio-rs/axum/pull/133))
  - **changed:** `EmptyRouter`, `ExtractorMiddleware`, `ExtractorMiddlewareLayer`,
    and `QueryStringMissing` no longer implement `Copy` ([#132](https://github.com/tokio-rs/axum/pull/132))
  - **changed:** `service::OnMethod`, `handler::OnMethod`, and `routing::Nested` have new response future types ([#157](https://github.com/tokio-rs/axum/pull/157))

# 0.1.3 (06. August, 2021)

- Fix stripping prefix when nesting services at `/` ([#91](https://github.com/tokio-rs/axum/pull/91))
- Add support for WebSocket protocol negotiation ([#83](https://github.com/tokio-rs/axum/pull/83))
- Use `pin-project-lite` instead of `pin-project` ([#95](https://github.com/tokio-rs/axum/pull/95))
- Re-export `http` crate and `hyper::Server` ([#110](https://github.com/tokio-rs/axum/pull/110))
- Fix `Query` and `Form` extractors giving bad request error when query string is empty. ([#117](https://github.com/tokio-rs/axum/pull/117))
- Add `Path` extractor. ([#124](https://github.com/tokio-rs/axum/pull/124))
- Fixed the implementation of `IntoResponse` of `(HeaderMap, T)` and `(StatusCode, HeaderMap, T)` would ignore headers from `T` ([#137](https://github.com/tokio-rs/axum/pull/137))
- Deprecate `extract::UrlParams` and `extract::UrlParamsMap`. Use `extract::Path` instead ([#138](https://github.com/tokio-rs/axum/pull/138))

# 0.1.2 (01. August, 2021)

- Implement `Stream` for `WebSocket` ([#52](https://github.com/tokio-rs/axum/pull/52))
- Implement `Sink` for `WebSocket` ([#52](https://github.com/tokio-rs/axum/pull/52))
- Implement `Deref` most extractors ([#56](https://github.com/tokio-rs/axum/pull/56))
- Return `405 Method Not Allowed` for unsupported method for route ([#63](https://github.com/tokio-rs/axum/pull/63))
- Add extractor for remote connection info ([#55](https://github.com/tokio-rs/axum/pull/55))
- Improve error message of `MissingExtension` rejections ([#72](https://github.com/tokio-rs/axum/pull/72))
- Improve documentation for routing ([#71](https://github.com/tokio-rs/axum/pull/71))
- Clarify required response body type when routing to `tower::Service`s ([#69](https://github.com/tokio-rs/axum/pull/69))
- Add `axum::body::box_body` to converting an `http_body::Body` to `axum::body::BoxBody` ([#69](https://github.com/tokio-rs/axum/pull/69))
- Add `axum::sse` for Server-Sent Events ([#75](https://github.com/tokio-rs/axum/pull/75))
- Mention required dependencies in docs ([#77](https://github.com/tokio-rs/axum/pull/77))
- Fix WebSockets failing on Firefox ([#76](https://github.com/tokio-rs/axum/pull/76))

# 0.1.1 (30. July, 2021)

- Misc readme fixes.

# 0.1.0 (30. July, 2021)

- Initial release.
