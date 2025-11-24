# cfg-if

[Documentation](https://docs.rs/cfg-if)

A macro to ergonomically define an item depending on a large number of #[cfg]
parameters. Structured like an if-else chain, the first matching branch is the
item that gets emitted.

```toml
[dependencies]
cfg-if = "1.0"
```

## Example

```rust
cfg_if::cfg_if! {
    if #[cfg(unix)] {
        fn foo() { /* unix specific functionality */ }
    } else if #[cfg(target_pointer_width = "32")] {
        fn foo() { /* non-unix, 32-bit functionality */ }
    } else {
        fn foo() { /* fallback implementation */ }
    }
}

fn main() {
    foo();
}
```
The `cfg_if!` block above is expanded to:
```rust
#[cfg(unix)]
fn foo() { /* unix specific functionality */ }
#[cfg(all(target_pointer_width = "32", not(unix)))]
fn foo() { /* non-unix, 32-bit functionality */ }
#[cfg(not(any(unix, target_pointer_width = "32")))]
fn foo() { /* fallback implementation */ }
```

# License

This project is licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or
   https://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or
   https://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in `cfg-if` by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
