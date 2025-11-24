<p align="center">
  <a href="https://sentry.io/?utm_source=github&utm_medium=logo" target="_blank">
    <img src="https://sentry-brand.storage.googleapis.com/sentry-wordmark-dark-280x84.png" alt="Sentry" width="280" height="84">
  </a>
</p>

# Sentry Rust SDK: sentry

This crate provides support for logging events and errors / panics to the
[Sentry] error logging service. It integrates with the standard panic
system in Rust as well as a few popular error handling setups.

[Sentry]: https://sentry.io/

## Quickstart

The most convenient way to use this library is via the [`sentry::init`] function,
which starts a sentry client with a default set of integrations, and binds
it to the current [`Hub`]. More Information on how to use Sentry in parallel,
concurrent and async scenarios can be found on the [`Hub`] docs as well.

The [`sentry::init`] function returns a guard that when dropped will flush Events that were not
yet sent to the sentry service. It has a two second deadline for this so shutdown of
applications might slightly delay as a result of this. Keep the guard around or sending events
will not work.

```rust
let _guard = sentry::init("https://key@sentry.io/42");
sentry::capture_message("Hello World!", sentry::Level::Info);
// when the guard goes out of scope here, the client will wait up to two
// seconds to send remaining events to the service.
```

More complex examples on how to use sentry can also be found in [examples]. Extended instructions
may also be found on [Sentry itself].

[`sentry::init`]: https://docs.rs/sentry/0.34.0/sentry/fn.init.html
[`Hub`]: https://docs.rs/sentry/0.34.0/sentry/struct.Hub.html
[examples]: https://github.com/getsentry/sentry-rust/tree/master/sentry/examples
[Sentry itself]: https://docs.sentry.io/platforms/rust

## Integrations

What makes this crate useful are its various integrations. Some of them are enabled by
default; See [Features]. Uncommon integrations or integrations for deprecated parts of
the ecosystem require a feature flag. For available integrations and how to use them, see
[integrations] and [apply_defaults].

[Features]: #features
[integrations]: https://docs.rs/sentry/0.34.0/sentry/integrations/index.html
[apply_defaults]: https://docs.rs/sentry/0.34.0/sentry/fn.apply_defaults.html

## Minimal API

This crate comes fully-featured. If the goal is to instrument libraries for usage
with sentry, or to extend sentry with a custom [`Integration`] or a [`Transport`],
one should use the [`sentry-core`] crate instead.

[`Integration`]: https://docs.rs/sentry/0.34.0/sentry/trait.Integration.html
[`Transport`]: https://docs.rs/sentry/0.34.0/sentry/trait.Transport.html
[`sentry-core`]: https://crates.io/crates/sentry-core

## Features

Additional functionality and integrations are enabled via feature flags. Some features require
extra setup to function properly.

| Feature           | Default | Is Integration | Deprecated | Additional notes                                                                         |
| --------------    | ------- | -------------- | ---------- | ---------------------------------------------------------------------------------------- |
| `backtrace`       | ✅      | 🔌             |            |                                                                                          |
| `contexts`        | ✅      | 🔌             |            |                                                                                          |
| `panic`           | ✅      | 🔌             |            |                                                                                          |
| `transport`       | ✅      |                |            |                                                                                          |
| `anyhow`          |         | 🔌             |            |                                                                                          |
| `test`            |         |                |            |                                                                                          |
| `debug-images`    | ✅      | 🔌             |            |                                                                                          |
| `log`             |         | 🔌             |            | Requires extra setup; See [`sentry-log`]'s documentation.                                |
| `debug-logs`      |         |                | ❗         | Requires extra setup; See [`sentry-log`]'s documentation.                                |
| `slog`            |         | 🔌             |            | Requires extra setup; See [`sentry-slog`]'s documentation.                               |
| `reqwest`         | ✅      |                |            |                                                                                          |
| `native-tls`      | ✅      |                |            | `reqwest` must be enabled.                                                               |
| `rustls`          |         |                |            | `reqwest` must be enabled. `native-tls` must be disabled via `default-features = false`. |
| `curl`            |         |                |            |                                                                                          |
| `surf`            |         |                |            |                                                                                          |
| `tower`           |         | 🔌             |            | Requires extra setup; See [`sentry-tower`]'s documentation.                              |
| `ureq`            |         |                |            | `ureq` transport support using `rustls` by default                                       |
| `ureq-native-tls` |         |                |            |                                                                                          |

[`sentry-log`]: https://crates.io/crates/sentry-log
[`sentry-slog`]: https://crates.io/crates/sentry-slog
[`sentry-tower`]: https://crates.io/crates/sentry-tower

### Default features
- `backtrace`: Enables backtrace support.
- `contexts`: Enables capturing device, OS, and Rust contexts.
- `panic`: Enables support for capturing panics.
- `transport`: Enables the default transport, which is currently `reqwest` with `native-tls`.
- `debug-images`: Enables capturing metadata about the loaded shared libraries.

### Debugging/Testing
- `anyhow`: Enables support for the `anyhow` crate.
- `test`: Enables testing support.

### Logging
- `log`: Enables support for the `log` crate.
- `slog`: Enables support for the `slog` crate.
- `debug-logs`: **Deprecated**. Uses the `log` crate for internal logging.

### Transports
- `reqwest`: **Default**. Enables the `reqwest` transport.
- `native-tls`: **Default**. Uses the `native-tls` crate. This only affects the `reqwest` transport.
- `rustls`: Enables `rustls` support for `reqwest`. Please note that `native-tls` is a default
  feature, and `default-features = false` must be set to completely disable building `native-tls`
  dependencies.
- `curl`: Enables the `curl` transport.
- `surf`: Enables the `surf` transport.
- `ureq`: Enables the `ureq` transport using `rustls`.
- `ureq-native-tls`: Enables the `ureq` transport using `native-tls`.

### Integrations
- `tower`: Enables support for the `tower` crate and those using it.

## Resources

License: Apache-2.0

- [Discord](https://discord.gg/ez5KZN7) server for project discussions.
- Follow [@getsentry](https://twitter.com/getsentry) on Twitter for updates
