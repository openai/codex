# pin-project-lite

[![crates.io](https://img.shields.io/crates/v/pin-project-lite?style=flat-square&logo=rust)](https://crates.io/crates/pin-project-lite)
[![docs.rs](https://img.shields.io/badge/docs.rs-pin--project--lite-blue?style=flat-square&logo=docs.rs)](https://docs.rs/pin-project-lite)
[![license](https://img.shields.io/badge/license-Apache--2.0_OR_MIT-blue?style=flat-square)](#license)
[![msrv](https://img.shields.io/badge/msrv-1.37-blue?style=flat-square&logo=rust)](https://www.rust-lang.org)
[![github actions](https://img.shields.io/github/actions/workflow/status/taiki-e/pin-project-lite/ci.yml?branch=main&style=flat-square&logo=github)](https://github.com/taiki-e/pin-project-lite/actions)

<!-- tidy:crate-doc:start -->
A lightweight version of [pin-project] written with declarative macros.

## Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
pin-project-lite = "0.2"
```

## Examples

[`pin_project!`] macro creates a projection type covering all the fields of
struct.

```rust
use std::pin::Pin;

use pin_project_lite::pin_project;

pin_project! {
    struct Struct<T, U> {
        #[pin]
        pinned: T,
        unpinned: U,
    }
}

impl<T, U> Struct<T, U> {
    fn method(self: Pin<&mut Self>) {
        let this = self.project();
        let _: Pin<&mut T> = this.pinned; // Pinned reference to the field
        let _: &mut U = this.unpinned; // Normal reference to the field
    }
}
```

To use [`pin_project!`] on enums, you need to name the projection type
returned from the method.

```rust
use std::pin::Pin;

use pin_project_lite::pin_project;

pin_project! {
    #[project = EnumProj]
    enum Enum<T, U> {
        Variant { #[pin] pinned: T, unpinned: U },
    }
}

impl<T, U> Enum<T, U> {
    fn method(self: Pin<&mut Self>) {
        match self.project() {
            EnumProj::Variant { pinned, unpinned } => {
                let _: Pin<&mut T> = pinned;
                let _: &mut U = unpinned;
            }
        }
    }
}
```

## [pin-project] vs pin-project-lite

Here are some similarities and differences compared to [pin-project].

### Similar: Safety

pin-project-lite guarantees safety in much the same way as [pin-project].
Both are completely safe unless you write other unsafe code.

### Different: Minimal design

This library does not tackle as expansive of a range of use cases as
[pin-project] does. If your use case is not already covered, please use
[pin-project].

### Different: No proc-macro related dependencies

This is the **only** reason to use this crate. However, **if you already
have proc-macro related dependencies in your crate's dependency graph, there
is no benefit from using this crate.** (Note: There is almost no difference
in the amount of code generated between [pin-project] and pin-project-lite.)

### Different: No useful error messages

This macro does not handle any invalid input. So error messages are not to
be useful in most cases. If you do need useful error messages, then upon
error you can pass the same input to [pin-project] to receive a helpful
description of the compile error.

### Different: No support for custom Unpin implementation

pin-project supports this by [`UnsafeUnpin`][unsafe-unpin]. (`!Unpin` is supported by both [pin-project][not-unpin] and [pin-project-lite][not-unpin-lite].)

### Different: No support for tuple structs and tuple variants

pin-project supports this.

[not-unpin]: https://docs.rs/pin-project/latest/pin_project/attr.pin_project.html#unpin
[pin-project]: https://github.com/taiki-e/pin-project
[unsafe-unpin]: https://docs.rs/pin-project/latest/pin_project/attr.pin_project.html#unsafeunpin

<!-- tidy:crate-doc:end -->

[not-unpin-lite]: https://docs.rs/pin-project-lite/latest/pin_project_lite/macro.pin_project.html#unpin
[`pin_project!`]: https://docs.rs/pin-project-lite/latest/pin_project_lite/macro.pin_project.html

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.
