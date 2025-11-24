# `findshlibs`

[![](https://img.shields.io/crates/v/findshlibs.svg)](https://crates.io/crates/findshlibs)
[![](https://docs.rs/findshlibs/badge.svg)](https://docs.rs/findshlibs)
[![Build Status](https://github.com/gimli-rs/findshlibs/workflows/CI/badge.svg)](https://github.com/gimli-rs/findshlibs/actions)

Find the shared libraries loaded in the current process with a cross platform
API.

## Documentation

[📚 Documentation on docs.rs 📚](https://docs.rs/findshlibs)

## Example

Here is an example program that prints out each shared library that is
loaded in the process and the addresses where each of its segments are
mapped into memory.

```rust
extern crate findshlibs;
use findshlibs::{Segment, SharedLibrary, TargetSharedLibrary};

fn main() {
    TargetSharedLibrary::each(|shlib| {
        println!("{}", shlib.name().to_string_lossy());

        for seg in shlib.segments() {
            println!("    {}: segment {}",
                     seg.actual_virtual_memory_address(shlib),
                     seg.name().to_string_lossy());
        }
    });
}
```

## Supported OSes

These are the OSes that `findshlibs` currently supports:

* Linux
* macOS
* Windows
* Android
* iOS

If a platform is not supported then a fallback implementation is used that
does nothing.  To see if your platform does something at runtime the
`TARGET_SUPPORTED` constant can be used.

Is your OS missing here? Send us a pull request!
