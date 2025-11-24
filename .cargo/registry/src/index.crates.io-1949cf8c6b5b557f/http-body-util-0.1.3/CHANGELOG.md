# v0.1.3

- Add `Channel`, a body type backed by an async channel.
- Make `Empty::new()` to be `const fn`.

# v0.1.2

- Add `BodyDataStream` type to convert a body to a stream of its data.

# v0.1.1

- Add `BodyExt::with_trailers()` combinator.
- Improve performance of `BodyExt::collect().to_bytes()`.

# v0.1.0

- Update `http` to 1.0.
- Update `http-body` to 1.0.

# v0.1.0-rc.3

- Fix `BodyExt::collect()` from panicking on an empty frame.

# v0.1.0-rc.2

- Update to `http-body` rc.2.

# v0.1.0-rc.1

- Initial release, split from http-body 0.4.5.
