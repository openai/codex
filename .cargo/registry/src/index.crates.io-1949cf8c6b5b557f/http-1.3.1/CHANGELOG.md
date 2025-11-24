# 1.3.1 (March 11, 2025)

* Fix validation that all characters are UTF-8 in URI path and query.

# 1.3.0 (March 11, 2025)

* Allow most UTF-8 characters in URI path and query.
* Fix `HeaderMap::reserve()` to allocate sufficient capacity.

# 1.2.0 (December 3, 2024)

* Add `StatusCode::TOO_EARLY` constant for 425 status.
* Loosen `TryFrom<HashMap>` for `HeaderMap` to work with any state generic.
* Change `Builder` methods to use `TryInto` instead of `TryFrom` arguments.
* Make `StatusCode::as_u16` a `const` function.
* Fix `Method` parsing to allow `#$%&'` characters.
* Fix `HeaderName` parsing to reject `"` characters.
* Fix off by 1 error in `Method::from_bytes` that could cause extra allocations.

# 1.1.0 (March 4, 2024)

* Add methods to allow trying to allocate in the `HeaderMap`, returning an error if oversize instead of panicking.
* Add `Extensions::get_or_insert()` method.
* Implement `From<Uri>` for `uri::Builder`.
* Fix `HeaderName::from_lowercase` that could allow NUL bytes in some cases.

# 1.0.0 (November 15, 2023)

- Implement `Clone` for `Request`, `Response`, and `Extensions`. This breaking change requires
  that all extensions now implement `Clone`.
- Add a default-on `std` feature. Disabling it currently is not supported.
- Fix MIRI warnings in `HeaderMap::iter()`.

# 0.2.10 (November 10, 2023)

* Fix parsing of `Authority` to handle square brackets in incorrect order.
* Fix `HeaderMap::with_capacity()` to handle arithmetic overflow.

# 0.2.9 (February 17, 2023)

* Add `HeaderName` constants for `cache-status` and `cdn-cache-control`.
* Implement `Hash` for `PathAndQuery`.
* Re-export `HeaderName` at crate root.

# 0.2.8 (June 6, 2022)

* Fix internal usage of uninitialized memory to use `MaybeUninit` inside `HeaderName`.

# 0.2.7 (April 28, 2022)

* MSRV bumped to `1.49`.
* Add `extend()` method to `Extensions`.
* Add `From<Authority>` and `From<PathAndQuery>` impls for `Uri`.
* Make `HeaderName::from_static` a `const fn`.

# 0.2.6 (December 30, 2021)

* Upgrade internal `itoa` dependency to 1.0.

# 0.2.5 (September 21, 2021)

* Add `is_empty()` and `len()` methods to `Extensions`.
* Add `version_ref()` method to `request::Builder`.
* Implement `TryFrom<Vec<u8>>` and `TryFrom<String>` for `Authority`, `Uri`, `PathAndQuery`, and `HeaderName`.
* Make `HeaderValue::from_static` a `const fn`.

# 0.2.4 (April 4, 2021)

* Fix `Uri` parsing to allow `{`, `"`, and `}` in paths.

# 0.2.3 (January 7, 2021)

* Upgrade internal (private) `bytes` dependency to 1.0.

# 0.2.2 (December 14, 2020)

* Fix (potential double) panic of (`HeaderMap`) `OccupiedEntry::remove_entry` and
  `remove_entry_mult` when multiple values are present. ([#446], [#449] dekellum)
* Safety audits of (priv) `ByteStr` and refactor of `Authority` ([#408], [#414] sbosnick)
* Fix `HeaderName` to error instead of panic when input is too long ([#432] [#433] acfoltzer)
* Allow `StatusCode` to encode values 100-999 without error. Use of the
  unclassified range 600-999 remains discouraged. ([#144], [#438], [#443] quininer dekellum)
* Add `String` and `&String` fallible conversions to `PathAndQuery` ([#450] mkindahl)
* Fix `Authority` (and `Uri`) to error instead of panic on unbalanced brackets
  ([#435], [#445] aeryz)

# 0.2.1 (March 25, 2020)

* Add `extensions_ref` and `extensions_mut` to `request::Builder` and `response::Builder`.

# 0.2.0 (December 2, 2019)

* Add `Version::HTTP_3` constant.
* Add `HeaderValue::from_maybe_shared`, `HeaderValue::from_maybe_shared_unchecked`, `Uri::from_maybe_shared`, `Authority::from_maybe_shared`, and `PathAndQuery::from_maybe_shared`.
* Change `request::Builder`, `response::Builder`, and `uri::Builder` to use by-value methods instead of by-ref.
* Change from `HttpTryFrom` trait to `std::convert::TryFrom`.
* Change `HeaderMap::entry` to no longer return a `Result`.
* Change `HeaderMap::drain` iterator to match the behavior of `IntoIter`.
* Change `Authority::port` to return an `Option<Port>` instead of `Option<u16>`.
* Change `Uri::scheme` to return `Option<&Scheme>` instead of `Option<&str>`.
* Change `Uri::authority` to return `Option<&Authority>` instead of `Option<&str>`.
* Remove `InvalidUriBytes`, `InvalidHeaderNameBytes`, and `InvalidHeaderValueBytes` error types.
* Remove `HeaderValue::from_shared`, `HeaderValue::from_shared_unchecked`, `Uri::from_shared`, `Authority::from_shared`, `Scheme::from_shared`, and `PathAndQuery::from_shared`.
* Remove `Authority::port_part`.
* Remove `Uri::scheme_part` and `Uri::authority_part`.

# 0.1.20 (November 26, 2019)

* Fix possible double-free if `header::Drain` iterator is `std::mem::forgot`en (#357).
* Fix possible data race if multiple `header::ValueDrain`s are iterated on different threads (#362).
* Fix `HeaderMap::reserve` capacity overflows (#360).
* Fix parsing long authority-form `Uri`s (#351).

# 0.1.19 (October 15, 2019)

* Allow `%` in IPv6 addresses in `Uri` (#343).

# 0.1.18 (July 26, 2019)

* Fix compilation of `HeaderName` parsing on WASM targets (#324).
* Implement `HttpTryFrom<HashMap>` for `HeaderMap` (#326).
* Export `http::header::HeaderValue` as `http::HeaderValue`.

# 0.1.17 (April 5, 2019)

* Add `Error::inner_ref()` to view the kind of error (#303)
* Add `headers_ref()` and `headers_mut()` methods to `request::Builder` and `response::Builder` (#293)

# 0.1.16 (February 19, 2019)

* Fix `Uri` to permit more characters in the `path` (#296)

# 0.1.15 (January 22, 2019)

* Fix `Uri::host()` to include brackets of IPv6 literals (#292)
* Add `scheme_str` and `port_u16` methods to `Uri` (#287)
* Add `method_ref`, `uri_ref`, and `headers_ref` to `request::Builder` (#284)

# 0.1.14 (November 21, 2018)

* Add `Port` struct (#252, #255, #265)
* Introduce `Uri` builder (#219)
* Empty `Method` no longer considered valid (#262)
* Fix `Uri` equality when terminating question mark is present (#270)
* Allow % character in userinfo (#269)
* Support additional tokens for header names (#271)
* Export `http::headers::{IterMut, ValuesMut}` (#278)

# 0.1.13 (September 14, 2018)

* impl `fmt::Display` for `HeaderName` (#249)
* Fix `uri::Authority` parsing when there is no host after an `@` (#248)
* Fix `Uri` parsing to allow more characters in query strings (#247)

# 0.1.12 (September 7, 2018)

* Fix `HeaderValue` parsing to allow HTABs (#244)

# 0.1.11 (September 5, 2018)

* Add `From<&Self>` for `HeaderValue`, `Method`, and `StatusCode` (#238)
* Add `Uri::from_static` (#240)

# 0.1.10 (August 8, 2018)

* `impl HttpTryFrom<String>` for HeaderValue (#236)

# 0.1.9 (August 7, 2018)

* Fix double percent encoding (#233)
* Add additional HttpTryFrom impls (#234)

# 0.1.8 (July 23, 2018)

* Add fuller set of `PartialEq` for `Method` (#221)
* Reduce size of `HeaderMap` by using `Box<[Entry]>` instea of `Vec` (#224)
* Reduce size of `Extensions` by storing as `Option<Box<AnyMap>>` (#227)
* Implement `Iterator::size_hint` for most iterators in `header` (#226)

# 0.1.7 (June 22, 2018)

* Add `From<uN> for HeaderValue` for most integer types (#218).
* Add `Uri::into_parts()` inherent method (same as `Parts::from(uri)`) (#214).
* Fix converting `Uri`s in authority-form to `Parts` and then back into `Uri` (#216).
* Fix `Authority` parsing to reject multiple port sections (#215).
* Fix parsing 1 character authority-form `Uri`s into illegal forms (#220).

# 0.1.6 (June 13, 2018)

* Add `HeaderName::from_static()` constructor (#195).
* Add `Authority::from_static()` constructor (#186).
* Implement `From<HeaderName>` for `HeaderValue` (#184).
* Fix duplicate keys when iterating over `header::Keys` (#201).

# 0.1.5 (February 28, 2018)

* Add websocket handshake related header constants (#162).
* Parsing `Authority` with an empty string now returns an error (#164).
* Implement `PartialEq<u16>` for `StatusCode` (#153).
* Implement `HttpTryFrom<&Uri>` for `Uri` (#165).
* Implement `FromStr` for `Method` (#167).
* Implement `HttpTryFrom<String>` for `Uri` (#171).
* Add `into_body` fns to `Request` and `Response` (#172).
* Fix `Request::options` (#177).

# 0.1.4 (January 4, 2018)

* Add PathAndQuery::from_static (#148).
* Impl PartialOrd / PartialEq for Authority and PathAndQuery (#150).
* Add `map` fn to `Request` and `Response` (#151).

# 0.1.3 (December 11, 2017)

* Add `Scheme` associated consts for common protos.

# 0.1.2 (November 29, 2017)

* Add Uri accessor for scheme part.
* Fix Uri parsing bug (#134)

# 0.1.1 (October 9, 2017)

* Provide Uri accessors for parts (#129)
* Add Request builder helpers. (#123)
* Misc performance improvements (#126)

# 0.1.0 (September 8, 2017)

* Initial release.

[#144]: https://github.com/hyperium/http/issues/144
[#408]: https://github.com/hyperium/http/pull/408
[#414]: https://github.com/hyperium/http/pull/414
[#432]: https://github.com/hyperium/http/issues/432
[#433]: https://github.com/hyperium/http/pull/433
[#438]: https://github.com/hyperium/http/pull/438
[#443]: https://github.com/hyperium/http/pull/443
[#446]: https://github.com/hyperium/http/issues/446
[#449]: https://github.com/hyperium/http/pull/449
[#450]: https://github.com/hyperium/http/pull/450
[#435]: https://github.com/hyperium/http/issues/435
[#445]: https://github.com/hyperium/http/pull/445

