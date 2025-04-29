# codex-rs

April 24, 2025

Today, Codex CLI is written in TypeScript and requires Node.js 22+ to run it. For a number of users, this runtime requirement inhibits adoption: they would be better served by a standalone executable. As maintainers, we want Codex to run efficiently in a wide range of environments with minimal overhead. We also want to take advantage of operating system-specific APIs to provide better sandboxing, where possible.

To that end, we are moving forward with a Rust implementation of Codex CLI contained in this folder, which has the following benefits:

- The CLI compiles to small, standalone, platform-specific binaries.
- Can make direct, native calls to [seccomp](https://man7.org/linux/man-pages/man2/seccomp.2.html) and [landlock](https://man7.org/linux/man-pages/man7/landlock.7.html) in order to support sandboxing on Linux.
- No runtime garbage collection, resulting in lower memory consumption and better, more predictable performance.

Currently, the Rust implementation is materially behind the TypeScript implementation in functionality, so continue to use the TypeScript implmentation for the time being. We will publish native executables via GitHub Releases as soon as we feel the Rust version is usable.

## Code Organization

This folder is the root of a Cargo workspace. It contains quite a bit of experimental code, but here are the key crates:

- [`core/`](./core) contains the business logic for Codex. Ultimately, we hope this to be a library crate that is generally useful for building other Rust/native applications that use Codex.
- [`exec/`](./exec) "headless" CLI for use in automation.
- [`tui/`](./tui) CLI that launches a fullscreen TUI built with [Ratatui](https://ratatui.rs/).
- [`repl/`](./repl) CLI that launches a lightweight REPL similar to the Python or Node.js REPL.
- [`cli/`](./cli) CLI multitool that provides the aforementioned CLIs via subcommands.
