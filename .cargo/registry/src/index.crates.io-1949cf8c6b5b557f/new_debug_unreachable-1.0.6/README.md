# new_debug_unreachable

> unreachable!() in debug, std::intrinsics::unreachable() in release.

This is a fork of [`debug_unreachable`](https://crates.io/crates/debug_unreachable).

## [Documentation](https://docs.rs/new_debug_unreachable)

## Usage

Use the crates.io repository; add this to your `Cargo.toml` along
with the rest of your dependencies:

```toml
[dependencies]
new_debug_unreachable = "1.0"
```

In your Rust code, the library name is still `debug_unreachable`:

```rust
use debug_unreachable::debug_unreachable;

fn main() {
    if 0 > 100 {
        // Can't happen!
        unsafe { debug_unreachable!() }
    } else {
        println!("Good, 0 <= 100.");
    }
}
```

## Author

[Jonathan Reem](https://medium.com/@jreem) is the original author of debug-unreachable.

[Matt Brubeck](https://limpet.net/mbrubeck/) is the maintainer of this fork.

## License

MIT
