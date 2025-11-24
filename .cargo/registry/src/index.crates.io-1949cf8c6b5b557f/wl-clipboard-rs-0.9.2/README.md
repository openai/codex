# wl-clipboard-rs

[![crates.io](https://img.shields.io/crates/v/wl-clipboard-rs.svg)](https://crates.io/crates/wl-clipboard-rs)
[![Build Status](https://github.com/YaLTeR/wl-clipboard-rs/actions/workflows/ci.yml/badge.svg?branch=master)](https://github.com/YaLTeR/wl-clipboard-rs/actions/workflows/ci.yml?query=branch%3Amaster)
[![Documentation](https://docs.rs/wl-clipboard-rs/badge.svg)](https://docs.rs/wl-clipboard-rs)

[Documentation (master)](https://yalter.github.io/wl-clipboard-rs/wl_clipboard_rs/)

A safe Rust crate for working with the Wayland clipboard.

This crate is intended to be used by terminal applications, clipboard managers and other
utilities which don't spawn Wayland surfaces (windows). If your application has a window,
please use the appropriate Wayland protocols for interacting with the Wayland clipboard
(`wl_data_device` from the core Wayland protocol, the `primary_selection` protocol for the
primary selection), for example via the
[smithay-clipboard](https://crates.io/crates/smithay-clipboard) crate.

The protocol used for clipboard interaction is `ext-data-control` or `wlr-data-control`. When
using the regular clipboard, the compositor must support any version of either protocol. When
using the "primary" clipboard, the compositor must support any version of `ext-data-control`,
or the second version of the `wlr-data-control` protocol.

For example applications using these features, see `wl-clipboard-rs-tools/src/bin/wl_copy.rs`
and `wl-clipboard-rs-tools/src/bin/wl_paste.rs` which implement terminal apps similar to
[wl-clipboard](https://github.com/bugaevc/wl-clipboard) or
`wl-clipboard-rs-tools/src/bin/wl_clip.rs` which implements a Wayland version of `xclip`.

The Rust implementation of the Wayland client is used by default; use the `native_lib` feature
to link to `libwayland-client.so` for communication instead. A `dlopen` feature is also
available for loading `libwayland-client.so` dynamically at runtime rather than linking to it.

The code of the crate itself (and the code of the example utilities) is 100% safe Rust. This
doesn't include the dependencies.

## Examples

Copying to the regular clipboard:
```rust
use wl_clipboard_rs::copy::{MimeType, Options, Source};

let opts = Options::new();
opts.copy(Source::Bytes("Hello world!".to_string().into_bytes().into()), MimeType::Autodetect)?;
```

Pasting plain text from the regular clipboard:
```rust
use std::io::Read;
use wl_clipboard_rs::{paste::{get_contents, ClipboardType, Error, MimeType, Seat}};

let result = get_contents(ClipboardType::Regular, Seat::Unspecified, MimeType::Text);
match result {
    Ok((mut pipe, _)) => {
        let mut contents = vec![];
        pipe.read_to_end(&mut contents)?;
        println!("Pasted: {}", String::from_utf8_lossy(&contents));
    }

    Err(Error::NoSeats) | Err(Error::ClipboardEmpty) | Err(Error::NoMimeType) => {
        // The clipboard is empty or doesn't contain text, nothing to worry about.
    }

    Err(err) => Err(err)?
}
```

Checking if the "primary" clipboard is supported (note that this might be unnecessary depending
on your crate usage, the regular copying and pasting functions do report if the primary
selection is unsupported when it is requested):

```rust
use wl_clipboard_rs::utils::{is_primary_selection_supported, PrimarySelectionCheckError};

match is_primary_selection_supported() {
    Ok(supported) => {
        // We have our definitive result. False means that ext/wlr-data-control is present
        // and did not signal the primary selection support, or that only wlr-data-control
        // version 1 is present (which does not support primary selection).
    },
    Err(PrimarySelectionCheckError::NoSeats) => {
        // Impossible to give a definitive result. Primary selection may or may not be
        // supported.

        // The required protocol (ext-data-control, or wlr-data-control version 2) is there,
        // but there are no seats. Unfortunately, at least one seat is needed to check for the
        // primary clipboard support.
    },
    Err(PrimarySelectionCheckError::MissingProtocol) => {
        // The data-control protocol (required for wl-clipboard-rs operation) is not
        // supported by the compositor.
    },
    Err(_) => {
        // Some communication error occurred.
    }
}
```

## Included terminal utilities

- `wl-paste`: implements `wl-paste` from
  [wl-clipboard](https://github.com/bugaevc/wl-clipboard).
- `wl-copy`: implements `wl-copy` from [wl-clipboard](https://github.com/bugaevc/wl-clipboard).
- `wl-clip`: a Wayland version of `xclip`.

Stuff that would be neat to add:
- Utility that mimics `xsel` commandline flags.

License: MIT/Apache-2.0
