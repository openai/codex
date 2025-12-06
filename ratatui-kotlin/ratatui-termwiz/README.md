# Ratatui-termwiz

<!-- cargo-rdme start -->

This module provides the [`TermwizBackend`] implementation for the [`Backend`] trait. It uses
the [Termwiz] crate to interact with the terminal.

[`Backend`]: trait.Backend.html
[Termwiz]: https://crates.io/crates/termwiz

## Crate Organization

`ratatui-termwiz` is part of the Ratatui workspace that was modularized in version 0.30.0.
This crate provides the [Termwiz] backend implementation for Ratatui.

**When to use `ratatui-termwiz`:**

- You need fine-grained control over dependencies
- Building a widget library that needs backend functionality
- You want to use only the Termwiz backend without other backends
- You need Termwiz's advanced terminal capabilities

**When to use the main [`ratatui`] crate:**

- Building applications (recommended - includes termwiz backend when enabled)
- You want the convenience of having everything available

For detailed information about the workspace organization, see [ARCHITECTURE.md].

[`ratatui`]: https://crates.io/crates/ratatui
[ARCHITECTURE.md]: https://github.com/ratatui/ratatui/blob/main/ARCHITECTURE.md

<!-- cargo-rdme end -->
