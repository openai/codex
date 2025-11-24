# [hyper](https://hyper.rs)

[![crates.io](https://img.shields.io/crates/v/hyper.svg)](https://crates.io/crates/hyper)
[![Released API docs](https://docs.rs/hyper/badge.svg)](https://docs.rs/hyper)
[![MIT licensed](https://img.shields.io/badge/license-MIT-blue.svg)](./LICENSE)
[![CI](https://github.com/hyperium/hyper/workflows/CI/badge.svg)](https://github.com/hyperium/hyper/actions?query=workflow%3ACI)
[![Discord chat][discord-badge]][discord-url]

A protective and efficient HTTP library for all.

- HTTP/1 and HTTP/2
- Asynchronous design
- Leading in performance
- Tested and **correct**
- Extensive production use
- Client and Server APIs

**Get started** by looking over the [guides](https://hyper.rs/guides/1/).

## "Low-level"

hyper is a relatively low-level library, meant to be a building block for
libraries and applications.

If you are looking for a convenient HTTP client, then you may wish to consider
[reqwest](https://github.com/seanmonstar/reqwest).

If you are not sure what HTTP server to choose, then you may want to consider
[axum](https://github.com/tokio-rs/axum) or
[warp](https://github.com/seanmonstar/warp), the latter taking a more functional
approach. Both are built on top of this library.

## Contributing

To get involved, take a look at [CONTRIBUTING](CONTRIBUTING.md).

If you prefer chatting, there is an active community in the [Discord server][discord-url].

## License

hyper is provided under the MIT license. See [LICENSE](LICENSE).

[discord-badge]: https://img.shields.io/discord/500028886025895936.svg?logo=discord
[discord-url]: https://discord.gg/kkwpueZ
