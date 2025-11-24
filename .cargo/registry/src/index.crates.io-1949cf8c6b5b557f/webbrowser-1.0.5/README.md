# webbrowser

[![Current Crates.io Version](https://img.shields.io/crates/v/webbrowser.svg)](https://crates.io/crates/webbrowser)
[![Crates.io Downloads](https://img.shields.io/crates/d/webbrowser.svg)](https://crates.io/crates/webbrowser)
[![License](https://img.shields.io/crates/l/webbrowser.svg)](#license)

![Linux Build](https://github.com/amodm/webbrowser-rs/workflows/Linux/badge.svg?branch=main&x=1)
![Windows Build](https://github.com/amodm/webbrowser-rs/workflows/Windows/badge.svg?branch=main&x=1)
![MacOS Build](https://github.com/amodm/webbrowser-rs/workflows/MacOS/badge.svg?branch=main&x=1)
![iOS Build](https://github.com/amodm/webbrowser-rs/workflows/iOS/badge.svg?branch=main&x=1)
![Android Build](https://github.com/amodm/webbrowser-rs/workflows/Android/badge.svg?branch=main&x=1)
![WASM Build](https://github.com/amodm/webbrowser-rs/workflows/WASM/badge.svg?branch=main&x=1)

Rust library to open URLs and local files in the web browsers available on a platform, with guarantees of [Consistent Behaviour](#consistent-behaviour).

Inspired by the [webbrowser](https://docs.python.org/2/library/webbrowser.html) python library

## Documentation

- [API Reference](https://docs.rs/webbrowser)
- [Release Notes](CHANGELOG.md)

## Examples

```rust
use webbrowser;

if webbrowser::open("http://github.com").is_ok() {
    // ...
}
```

## Platform Support

| Platform | Supported | Browsers | Test status |
|----------|-----------|----------|-------------|
| macos    | ✅        | default + [others](https://docs.rs/webbrowser/latest/webbrowser/enum.Browser.html) | ✅ |
| windows  | ✅        | default only | ✅ |
| linux/wsl | ✅       | default only (respects $BROWSER env var, so can be used with other browsers) | ✅ |
| android  | ✅        | default only | ✅ |
| ios/tvOS/visionOS    | ✅        | default only | ✅ |
| wasm     | ✅        | default only | ✅ |
| unix (*bsd, aix etc.) | ✅        | default only (respects $BROWSER env var, so can be used with other browsers) | Manual |

## Consistent Behaviour
`webbrowser` defines consistent behaviour on all platforms as follows:
* **Browser guarantee** - This library guarantees that the browser is opened, even for local files - the only crate to make such guarantees
at the time of this writing. Alternative libraries rely on existing system commands, which may lead to an editor being opened (instead
of the browser) for local html files, leading to an inconsistent behaviour for users.
* **Non-Blocking** for GUI based browsers (e.g. Firefox, Chrome etc.), while **Blocking** for text based browser (e.g. lynx etc.)
* **Suppressed output** by default for GUI based browsers, so that their stdout/stderr don't pollute the main program's output. This can be
overridden by `webbrowser::open_browser_with_options`.

## Crate Features
`webbrowser` optionally allows the following features to be configured:
* `hardened` - this disables handling of non-http(s) urls (e.g. `file:///`) as a hard security precaution
* `disable-wsl` - this disables WSL `file` implementation (`http` still works)
* `wasm-console` - this enables logging to wasm console (valid only on wasm platform)

## Looking to contribute?

PRs invited for

* Bugs
* Supporting non-default browser invocation on any platform

Important note (while testing):

* This library requires availability of browsers and a graphical environment during runtime
* `cargo test` will actually open the browser locally

When contributing, please note that your work will be dual licensed as MIT + Apache-2.0 (see below).

## License

`SPDX-License-Identifier: Apache-2.0 OR MIT`

Licensed under either of

* Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
* MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any
additional terms or conditions.
