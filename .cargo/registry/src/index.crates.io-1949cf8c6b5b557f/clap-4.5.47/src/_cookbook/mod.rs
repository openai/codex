// Contributing
//
// New examples:
// - Building: They must be added to `Cargo.toml` with the appropriate `required-features`.
// - Testing: Ensure there is a markdown file with [trycmd](https://docs.rs/trycmd) syntax
// - Link the `.md` file from here

//! # Documentation: Cookbook
//!
//! Typed arguments: [derive][typed_derive]
//! - Topics:
//!   - Custom `parse()`
//!
//! Custom cargo command: [builder][cargo_example], [derive][cargo_example_derive]
//! - Topics:
//!   - Subcommands
//!   - Cargo plugins
//!   - custom terminal [styles][crate::Command::styles] (colors)
//!
//! find-like interface: [builder][find]
//! - Topics:
//!   - Position-sensitive flags
//!
//! git-like interface: [builder][git], [derive][git_derive]
//! - Topics:
//!   - Subcommands
//!   - External subcommands
//!   - Optional subcommands
//!   - Default subcommands
//!   - [`last`][crate::Arg::last]
//!
//! pacman-like interface: [builder][pacman]
//! - Topics:
//!   - Flag subcommands
//!   - Conflicting arguments
//!
//! Escaped positionals with `--`: [builder][escaped_positional], [derive][escaped_positional_derive]
//!
//! Multi-call
//! - busybox: [builder][multicall_busybox]
//!   - Topics:
//!     - Subcommands
//! - hostname: [builder][multicall_hostname]
//!   - Topics:
//!     - Subcommands
//!
//! repl: [builder][repl], [derive][repl_derive]
//! - Topics:
//!   - Read-Eval-Print Loops / Custom command lines

pub mod cargo_example;
pub mod cargo_example_derive;
pub mod escaped_positional;
pub mod escaped_positional_derive;
pub mod find;
pub mod git;
pub mod git_derive;
pub mod multicall_busybox;
pub mod multicall_hostname;
pub mod pacman;
pub mod repl;
pub mod repl_derive;
pub mod typed_derive;
