# zbus_names

[![](https://docs.rs/zbus_names/badge.svg)](https://docs.rs/zbus_names/) [![](https://img.shields.io/crates/v/zbus_names)](https://crates.io/crates/zbus_names)

This crate provides collection of types for various [D-Bus bus names][dbn].

This is used by [`zbus`] (and in future by [`zbus_macros`] as well) crate. Other D-Bus crates are also
encouraged to use this API in the spirit of cooperation. :)

For convenience, `zbus` re-exports this crate as `names`, so you do not need to depend directly on
this crate if you already depend on `zbus`.

**Status:** Stable.

[dbn]: https://dbus.freedesktop.org/doc/dbus-specification.html#message-protocol-names
[`zbus`]: https://crates.io/crates/zbus
[`zbus_macros`]: https://crates.io/crates/zbus_macros
