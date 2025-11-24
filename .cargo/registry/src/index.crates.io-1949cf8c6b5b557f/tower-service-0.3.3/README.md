# Tower Service

The foundational `Service` trait that [Tower] is based on.

[![Crates.io][crates-badge]][crates-url]
[![Documentation][docs-badge]][docs-url]
[![Documentation (master)][docs-master-badge]][docs-master-url]
[![MIT licensed][mit-badge]][mit-url]
[![Build Status][actions-badge]][actions-url]
[![Discord chat][discord-badge]][discord-url]

[crates-badge]: https://img.shields.io/crates/v/tower-service.svg
[crates-url]: https://crates.io/crates/tower-service
[docs-badge]: https://docs.rs/tower-service/badge.svg
[docs-url]: https://docs.rs/tower-service
[docs-master-badge]: https://img.shields.io/badge/docs-master-blue
[docs-master-url]: https://tower-rs.github.io/tower/tower_service
[mit-badge]: https://img.shields.io/badge/license-MIT-blue.svg
[mit-url]: LICENSE
[actions-badge]: https://github.com/tower-rs/tower/workflows/CI/badge.svg
[actions-url]:https://github.com/tower-rs/tower/actions?query=workflow%3ACI
[discord-badge]: https://img.shields.io/discord/500028886025895936?logo=discord&label=discord&logoColor=white
[discord-url]: https://discord.gg/EeF3cQw

## Overview

The [`Service`] trait provides the foundation upon which [Tower] is built. It is a
simple, but powerful trait. At its heart, `Service` is just an asynchronous
function of request to response.

```
async fn(Request) -> Result<Response, Error>
```

Implementations of `Service` take a request, the type of which varies per
protocol, and returns a future representing the eventual completion or failure
of the response.

Services are used to represent both clients and servers. An *instance* of
`Service` is used through a client; a server *implements* `Service`.

By using standardizing the interface, middleware can be created. Middleware
*implement* `Service` by passing the request to another `Service`. The
middleware may take actions such as modify the request.

[`Service`]: https://docs.rs/tower-service/latest/tower_service/trait.Service.html
[Tower]: https://crates.io/crates/tower
## License

This project is licensed under the [MIT license](LICENSE).

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in Tower by you, shall be licensed as MIT, without any additional
terms or conditions.
