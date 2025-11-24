terminal-size
=============


[Documentation](https://docs.rs/terminal_size)


Rust library for getting the size of your terminal.

Works on Linux, macOS, Windows, and illumos.

```rust
use terminal_size::{Width, Height, terminal_size};

let size = terminal_size();
if let Some((Width(w), Height(h))) = size {
    println!("Your terminal is {} cols wide and {} lines tall", w, h);
} else {
    println!("Unable to get terminal size");
}
```

## Minimum Rust Version

This crate requires a minimum Rust version of 1.63.0 (2022-08-11).

## License

Licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or https://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or https://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you, as defined in the Apache-2.0
license, shall be dual licensed as above, without any additional terms or
conditions.
