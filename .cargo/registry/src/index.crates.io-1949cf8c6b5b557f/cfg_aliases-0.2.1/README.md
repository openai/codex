# CFG Aliases

CFG Aliases is a tiny utility to help save you a lot of effort with long winded `#[cfg()]` checks. This crate provides a single [`cfg_aliases!`] macro that doesn't have any dependencies and specifically avoids pulling in `syn` or `quote` so that the impact on your comile times should be negligible.

You use the the [`cfg_aliases!`] macro in your `build.rs` script to define aliases such as `x11` that could then be used in the `cfg` attribute or macro for conditional compilation: `#[cfg(x11)]`.

## Example

**Cargo.toml:**

```toml
[build-dependencies]
cfg_aliases = "0.1.0"
```

**build.rs:**

```rust
use cfg_aliases::cfg_aliases;

fn main() {
    // Setup cfg aliases
    cfg_aliases! {
        // Platforms
        wasm: { target_arch = "wasm32" },
        android: { target_os = "android" },
        macos: { target_os = "macos" },
        linux: { target_os = "linux" },
        // Backends
        surfman: { all(unix, feature = "surfman", not(wasm)) },
        glutin: { all(feature = "glutin", not(wasm)) },
        wgl: { all(windows, feature = "wgl", not(wasm)) },
        dummy: { not(any(wasm, glutin, wgl, surfman)) },
    }
}
```

Now that we have our aliases setup we can use them just like you would expect:

```rust
#[cfg(wasm)]
println!("This is running in WASM");

#[cfg(surfman)]
{
    // Do stuff related to surfman
}

#[cfg(dummy)]
println!("We're in dummy mode, specify another feature if you want a smarter app!");
```

This greatly improves what would otherwise look like this without the aliases:

```rust
#[cfg(target_arch = "wasm32")]
println!("We're running in WASM");

#[cfg(all(unix, feature = "surfman", not(target_arch = "wasm32")))]
{
    // Do stuff related to surfman
}

#[cfg(not(any(
    target_arch = "wasm32",
    all(unix, feature = "surfman", not(target_arch = "wasm32")),
    all(windows, feature = "wgl", not(target_arch = "wasm32")),
    all(feature = "glutin", not(target_arch = "wasm32")),
)))]
println!("We're in dummy mode, specify another feature if you want a smarter app!");
```

You can also use the `cfg!` macro or combine your aliases with other checks using `all()`, `not()`, and `any()`. Your aliases are genuine `cfg` flags now!

```rust
if cfg!(glutin) {
    // use glutin
} else {
    // Do something else
}

#[cfg(all(glutin, surfman))]
compile_error!("You cannot specify both `glutin` and `surfman` features");
```

## Syntax and Error Messages

The aliase names are restricted to the same rules as rust identifiers which, for one, means that they cannot have dashes ( `-` ) in them. Additionally, if you get certain syntax elements wrong, such as the alias name, the macro will error saying that the recursion limit was reached instead of giving a clear indication of what actually went wrong. This is due to a nuance with the macro parser and it might be fixed in a later release of this crate. It is also possible that aliases with dashes in the name might be supported in a later release. Open an issue if that is something that you would like implemented.

Finally, you can also induce an infinite recursion by having rules that both reference each-other, but this isn't a real limitation because that doesn't make logical sense anyway:

```rust,ignore
// This causes an error!
cfg_aliases! {
    test1: { not(test2) },
    test2: { not(test1) },
}
```

## Attribution and Thanks

- Thanks to my God and Father who led me through figuring this out and to whome I owe everything.
- Thanks to @Yandros on the Rust forum for [showing me][sm] some crazy macro hacks!
- Thanks to @sfackler for [pointing out][po] the way to make cargo add the cfg flags.
- Thanks to the authors of the [`tectonic_cfg_support::target_cfg`] macro from which most of the cfg attribute parsing logic is taken from. Also thanks to @ratmice for [bringing it up][bip] on the Rust forum.

[`tectonic_cfg_support::target_cfg`]: https://docs.rs/tectonic_cfg_support/0.0.1/src/tectonic_cfg_support/lib.rs.html#166-298
[po]: https://users.rust-lang.org/t/any-such-thing-as-cfg-aliases/40100/2
[bip]: https://users.rust-lang.org/t/any-such-thing-as-cfg-aliases/40100/13
[sm]: https://users.rust-lang.org/t/any-such-thing-as-cfg-aliases/40100/3
