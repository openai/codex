# Ratatui-crossterm

<!-- cargo-rdme start -->

This crate provides [`CrosstermBackend`], an implementation of the [`Backend`] trait for the
[Ratatui] library. It uses the [Crossterm] library for all terminal manipulation.

### Crossterm Version and Re-export

`ratatui-crossterm` requires you to specify a version of the [Crossterm] library to be used.
This is managed via feature flags. The highest enabled feature flag of the available
`crossterm_0_xx` features (e.g., `crossterm_0_28`, `crossterm_0_29`) takes precedence. These
features determine which version of Crossterm is compiled and used by the backend. Feature
unification may mean that any crate in your dependency graph that chooses to depend on a
specific version of Crossterm may be affected by the feature flags you enable.

Ratatui will support at least the two most recent versions of Crossterm (though we may increase
this if crossterm release cadence increases). We will remove support for older versions in major
(0.x) releases of `ratatui-crossterm`, and we may add support for newer versions in minor
(0.x.y) releases.

To promote interoperability within the [Ratatui] ecosystem, the selected Crossterm crate is
re-exported as `ratatui_crossterm::crossterm`. This re-export is essential for authors of widget
libraries or any applications that need to perform direct Crossterm operations while ensuring
compatibility with the version used by `ratatui-crossterm`. By using
`ratatui_crossterm::crossterm` for such operations, developers can avoid version conflicts and
ensure that all parts of their application use a consistent set of Crossterm types and
functions.

For example, if your application's `Cargo.toml` enables the `crossterm_0_29` feature for
`ratatui-crossterm`, then any code using `ratatui_crossterm::crossterm` will refer to the 0.29
version of Crossterm.

For more information on how to use the backend, see the documentation for the
[`CrosstermBackend`] struct.

[Ratatui]: https://ratatui.rs
[Crossterm]: https://crates.io/crates/crossterm
[`Backend`]: ratatui_core::backend::Backend

## Crate Organization

`ratatui-crossterm` is part of the Ratatui workspace that was modularized in version 0.30.0.
This crate provides the [Crossterm] backend implementation for Ratatui.

**When to use `ratatui-crossterm`:**

- You need fine-grained control over dependencies
- Building a widget library that needs backend functionality
- You want to use only the Crossterm backend without other backends

**When to use the main [`ratatui`] crate:**

- Building applications (recommended - includes crossterm backend by default)
- You want the convenience of having everything available

For detailed information about the workspace organization, see [ARCHITECTURE.md].

[`ratatui`]: https://crates.io/crates/ratatui
[ARCHITECTURE.md]: https://github.com/ratatui/ratatui/blob/main/ARCHITECTURE.md

<!-- cargo-rdme end -->
