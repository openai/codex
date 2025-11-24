# Contributing to rustix

Rustix is a [Bytecode Alliance] project. It follows the Bytecode Alliance's
[Code of Conduct] and [Organizational Code of Conduct].

## Testing

To keep compile times low, most features in rustix's API are behind cargo
features. A special feature, `all-apis` enables all APIs, which is useful
for testing.

```console
cargo test --features=all-apis
```

And, rustix has two backends, linux_raw and libc, and only one is used in
any given build. To test with the libc backend explicitly, additionally
enable the `use-libc` feature:

```console
cargo test --features=all-apis,use-libc
```

Beyond that, rustix's CI tests many targets and configurations. Asking for
help is always welcome, and it's especially encouraged when the issue is
getting all the `cfg`s lined up to get everything compiling on all the
configurations on CI.
