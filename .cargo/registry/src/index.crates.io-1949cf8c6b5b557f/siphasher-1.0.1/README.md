SipHash implementation for Rust
===============================

This crates implements SipHash-2-4 and SipHash-1-3 in Rust.

It is based on the original implementation from rust-core and exposes the
same API.

It also implements SipHash variants returning 128-bit tags.

The `sip` module implements the standard 64-bit mode, whereas the `sip128`
module implements the 128-bit mode.

Usage
-----

In `Cargo.toml`:

```toml
[dependencies]
siphasher = "1"
```

If you want [serde](https://github.com/serde-rs/serde) support, include the feature like this:

```toml
[dependencies]
siphasher = { version = "1", features = ["serde"] }
```

64-bit mode:

```rust
use siphasher::sip::{SipHasher, SipHasher13, SipHasher24};

// one-shot:

let array: &[u8] = &[1, 2, 3];
let key: &[u8; 16] = &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
let hasher = SipHasher13::new_with_key(key);
let h = hasher.hash(array);

// incremental:

use core::hash::Hasher;

let array1: &[u8] = &[1, 2, 3];
let array2: &[u8] = &[4, 5, 6];
let key: &[u8; 16] = &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
let mut hasher = SipHasher13::new_with_key(key);
hasher.write(array1);
hasher.write(array2);
let h = hasher.finish();
```

128-bit mode:

```rust
use siphasher::sip128::{Hasher128, SipHasher, SipHasher13, SipHasher24};

// one-shot:

let array: &[u8] = &[1, 2, 3];
let key: &[u8; 16] = &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
let hasher = SipHasher13::new_with_key(key);
let h = hasher.hash(array).as_bytes();

// incremental:

use core::hash::Hasher;

let array1: &[u8] = &[1, 2, 3];
let array2: &[u8] = &[4, 5, 6];
let key: &[u8; 16] = &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
let mut hasher = SipHasher13::new_with_key(key);
hasher.write(array1);
hasher.write(array2);
let h = hasher.finish128().as_bytes();
```

[API documentation](https://docs.rs/siphasher/)
-----------------------------------------------

Note
----

Due to a confusing and not well documented API, methods from the `Hasher` trait of the standard library (`std::hash::Hasher`, `core::hash::Hasher`) produce non-portable results.

This is not specific to SipHash, and affects all hash functions.

The only safe methods in that trait are `write()` and `finish()`.

It is thus recommended to use SipHash (and all other hash functions, actually) as documented above.
