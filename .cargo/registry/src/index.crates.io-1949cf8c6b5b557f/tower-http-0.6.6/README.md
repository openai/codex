# Tower HTTP

Tower middleware and utilities for HTTP clients and servers.

[![Build status](https://github.com/tower-rs/tower-http/workflows/CI/badge.svg)](https://github.com/tower-rs/tower-http/actions)
[![Crates.io](https://img.shields.io/crates/v/tower-http)](https://crates.io/crates/tower-http)
[![Documentation](https://docs.rs/tower-http/badge.svg)](https://docs.rs/tower-http)
[![Crates.io](https://img.shields.io/crates/l/tower-http)](tower-http/LICENSE)

More information about this crate can be found in the [crate documentation][docs].

## Middleware

Tower HTTP contains lots of middleware that are generally useful when building
HTTP servers and clients. Some of the highlights are:

- `Trace` adds high level logging of requests and responses. Supports both
  regular HTTP requests as well as gRPC.
- `Compression` and `Decompression` to compress/decompress response bodies.
- `FollowRedirect` to automatically follow redirection responses.

See the [docs] for the complete list of middleware.

Middleware uses the [http] crate as the HTTP interface so they're compatible
with any library or framework that also uses [http]. For example [hyper].

The middleware were originally extracted from one of [@EmbarkStudios] internal
projects.

## Examples

The [examples] folder contains various examples of how to use Tower HTTP:

- [warp-key-value-store]: A key/value store with an HTTP API built with warp.
- [tonic-key-value-store]: A key/value store with a gRPC API and client built with tonic.
- [axum-key-value-store]: A key/value store with an HTTP API built with axum.

## Minimum supported Rust version

tower-http's MSRV is 1.66.

## Getting Help

If you're new to tower its [guides] might help. In the tower-http repo we also
have a [number of examples][examples] showing how to put everything together.
You're also welcome to ask in the [`#tower` Discord channel][chat] or open an
[issue] with your question.

## Contributing

:balloon: Thanks for your help improving the project! We are so happy to have
you! We have a [contributing guide][guide] to help you get involved in the Tower
HTTP project.

[guide]: CONTRIBUTING.md

## License

This project is licensed under the [MIT license](tower-http/LICENSE).

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in Tower HTTP by you, shall be licensed as MIT, without any
additional terms or conditions.

[@EmbarkStudios]: https://github.com/EmbarkStudios
[examples]: https://github.com/tower-rs/tower-http/tree/master/examples
[http]: https://crates.io/crates/http
[tonic-key-value-store]: https://github.com/tower-rs/tower-http/tree/master/examples/tonic-key-value-store
[warp-key-value-store]: https://github.com/tower-rs/tower-http/tree/master/examples/warp-key-value-store
[axum-key-value-store]: https://github.com/tower-rs/tower-http/tree/master/examples/axum-key-value-store
[chat]: https://discord.gg/tokio
[docs]: https://docs.rs/tower-http
[hyper]: https://github.com/hyperium/hyper
[issue]: https://github.com/tower-rs/tower-http/issues/new
[milestone]: https://github.com/tower-rs/tower-http/milestones
[examples]: https://github.com/tower-rs/tower-http/tree/master/examples
[guides]: https://github.com/tower-rs/tower/tree/master/guides
