# Ratatui-termion

<!-- cargo-rdme start -->

This module provides the [`TermionBackend`] implementation for the [`Backend`] trait. It uses
the [Termion] crate to interact with the terminal.

[`Backend`]: ratatui_core::backend::Backend
[Termion]: https://docs.rs/termion

## Crate Organization

`ratatui-termion` is part of the Ratatui workspace that was modularized in version 0.30.0.
This crate provides the [Termion] backend implementation for Ratatui.

**When to use `ratatui-termion`:**

- You need fine-grained control over dependencies
- Building a widget library that needs backend functionality
- You want to use only the Termion backend without other backends
- You prefer Termion's Unix-focused approach

**When to use the main [`ratatui`] crate:**

- Building applications (recommended - includes termion backend when enabled)
- You want the convenience of having everything available

For detailed information about the workspace organization, see [ARCHITECTURE.md].

[`ratatui`]: https://crates.io/crates/ratatui
[ARCHITECTURE.md]: https://github.com/ratatui/ratatui/blob/main/ARCHITECTURE.md

<!-- cargo-rdme end -->
