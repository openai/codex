# zvariant_derive

[![](https://docs.rs/zvariant_derive/badge.svg)](https://docs.rs/zvariant_derive/) [![](https://img.shields.io/crates/v/zvariant_derive)](https://crates.io/crates/zvariant_derive)

This crate provides derive macros helpers for [`zvariant`]. The `zvariant` crate re-exports these
macros for your convenience so you do not need to use this crate directly.

**Status:** Stable.

## Example code

```rust
use zvariant::{serialized::Context, to_bytes, Type, LE};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Type, PartialEq, Debug)]
struct Struct<'s> {
    field1: u16,
    field2: i64,
    field3: &'s str,
}

assert_eq!(Struct::signature(), "(qxs)");
let s = Struct {
    field1: 42,
    field2: i64::max_value(),
    field3: "hello",
};
let ctxt = Context::new_dbus(LE, 0);
let encoded = to_bytes(ctxt, &s).unwrap();
let decoded: Struct = encoded.deserialize().unwrap().0;
assert_eq!(decoded, s);
```

[`zvariant`]: https://crates.io/crates/zvariant
