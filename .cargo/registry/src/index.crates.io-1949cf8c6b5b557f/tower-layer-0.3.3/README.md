# Tower Layer

Decorates a [Tower] `Service`, transforming either the request or the response.

[![Crates.io][crates-badge]][crates-url]
[![Documentation][docs-badge]][docs-url]
[![Documentation (master)][docs-master-badge]][docs-master-url]
[![MIT licensed][mit-badge]][mit-url]
[![Build Status][actions-badge]][actions-url]
[![Discord chat][discord-badge]][discord-url]

[crates-badge]: https://img.shields.io/crates/v/tower-layer.svg
[crates-url]: https://crates.io/crates/tower-layer
[docs-badge]: https://docs.rs/tower-layer/badge.svg
[docs-url]: https://docs.rs/tower-layer
[docs-master-badge]: https://img.shields.io/badge/docs-master-blue
[docs-master-url]: https://tower-rs.github.io/tower/tower_layer
[mit-badge]: https://img.shields.io/badge/license-MIT-blue.svg
[mit-url]: LICENSE
[actions-badge]: https://github.com/tower-rs/tower/workflows/CI/badge.svg
[actions-url]:https://github.com/tower-rs/tower/actions?query=workflow%3ACI
[discord-badge]: https://img.shields.io/discord/500028886025895936?logo=discord&label=discord&logoColor=white
[discord-url]: https://discord.gg/EeF3cQw

## Overview

Often, many of the pieces needed for writing network applications can be
reused across multiple services. The `Layer` trait can be used to write
reusable components that can be applied to very different kinds of services;
for example, it can be applied to services operating on different protocols,
and to both the client and server side of a network transaction.

## License

This project is licensed under the [MIT license](LICENSE).

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in Tower by you, shall be licensed as MIT, without any additional
terms or conditions.

[Tower]: https://crates.io/crates/tower