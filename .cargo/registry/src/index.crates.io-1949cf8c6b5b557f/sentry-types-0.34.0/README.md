<p align="center">
  <a href="https://sentry.io/?utm_source=github&utm_medium=logo" target="_blank">
    <img src="https://sentry-brand.storage.googleapis.com/sentry-wordmark-dark-280x84.png" alt="Sentry" width="280" height="84">
  </a>
</p>

# Sentry Rust SDK: sentry-types

This crate provides common types for working with the Sentry protocol or the
Sentry server.  It's used by the Sentry Relay infrastructure as well as the
rust Sentry client.

Most of the types in this crate are serializable in one form or another.
The types in the `protocol` module are generally really only serializable
to JSON as other formats are not supported by Sentry at this date.

### Contents

The crate provides a bunch of common types for working with Sentry as
such (DSN, ProjectIDs, authentication headers) as well as types for
the Sentry event protocol.

Right now only `v7` of the protocol is implemented but it's versioned
so later versions might be added later.

### API Concepts

Most types are directly serializable or deserializable and try to implement
the `Default` type.  This means that objects can be created conviently
and missing attributes can be filled in:

```rust
use sentry_types::protocol::v7;

let event = v7::Event {
    message: Some("Hello World!".to_string()),
    culprit: Some("foo in bar".to_string()),
    level: v7::Level::Info,
    ..Default::default()
};
```

## Resources

License: Apache-2.0

- [Discord](https://discord.gg/ez5KZN7) server for project discussions.
- Follow [@getsentry](https://twitter.com/getsentry) on Twitter for updates
