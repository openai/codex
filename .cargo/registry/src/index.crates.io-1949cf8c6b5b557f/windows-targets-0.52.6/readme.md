## Import libs for Windows

The [windows-targets](https://crates.io/crates/windows-targets) crate includes import libs, supports semantic versioning, and optional support for raw-dylib. 

* [Getting started](https://kennykerr.ca/rust-getting-started/)
* [Samples](https://github.com/microsoft/windows-rs/tree/0.58.0/crates/samples)
* [Releases](https://github.com/microsoft/windows-rs/releases)

Start by adding the following to your Cargo.toml file:

```toml
[dependencies.windows-targets]
version = "0.52"
```

Use the `link` macro to define the external functions you wish to call:

```rust,no_run
windows_targets::link!("kernel32.dll" "system" fn SetLastError(code: u32));
windows_targets::link!("kernel32.dll" "system" fn GetLastError() -> u32);

fn main() {
    unsafe {
        SetLastError(1234);
        assert_eq!(GetLastError(), 1234);
    }
}
```
