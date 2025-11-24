//! Composable wrappers over process::Command.
//!
//! # Quick start
//!
//! ```toml
//! [dependencies]
//! process-wrap = { version = "8.2.1", features = ["std"] }
//! ```
//!
//! ```rust,no_run
//! # fn main() -> std::io::Result<()> {
//! use std::process::Command;
//! use process_wrap::std::*;
//!
//! let mut command = StdCommandWrap::with_new("watch", |command| { command.arg("ls"); });
//! #[cfg(unix)] { command.wrap(ProcessGroup::leader()); }
//! #[cfg(windows)] { command.wrap(JobObject); }
//! let mut child = command.spawn()?;
//! let status = child.wait()?;
//! dbg!(status);
//! # Ok(()) }
//! ```
//!
//! ## Migrating from command-group
//!
//! The above example is equivalent to the `command-group` 5.x usage. To migrate from versions 4.x
//! and below, replace `ProcessGroup::leader()` with `ProcessSession`.
//!
//! # Overview
//!
//! This crate provides a composable set of wrappers over `process::Command` (either from std or
//! from Tokio). It is a more flexible and composable successor to the `command-group` crate, and is
//! meant to be adaptable to additional use cases: for example spawning processes in PTYs currently
//! requires a different crate (such as `pty-process`) which won't function with `command-group`.
//! Implementing a PTY wrapper for `process-wrap` would instead keep the same API and be composable
//! with the existing process group/session implementations.
//!
//! # Usage
//!
//! The core API is [`StdCommandWrap`](std::StdCommandWrap) and [`TokioCommandWrap`](tokio::TokioCommandWrap),
//! which can be constructed either directly from an existing `process::Command`:
//!
//! ```rust
//! use process_wrap::std::*;
//! use std::process::Command;
//! let mut command = Command::new("ls");
//! command.arg("-l");
//! let mut command = StdCommandWrap::from(command);
//! #[cfg(unix)] { command.wrap(ProcessGroup::leader()); }
//! #[cfg(windows)] { command.wrap(JobObject); }
//! ```
//!
//! ...or with a somewhat more ergonomic closure pattern:
//!
//! ```rust
//! use process_wrap::std::*;
//! let mut command = StdCommandWrap::with_new("ls", |command| { command.arg("-l"); });
//! #[cfg(unix)] { command.wrap(ProcessGroup::leader()); }
//! #[cfg(windows)] { command.wrap(JobObject); }
//! ```
//!
//! If targetting a single platform, then a fluent style is possible:
//!
//! ```rust
//! use process_wrap::std::*;
//! StdCommandWrap::with_new("ls", |command| { command.arg("-l"); })
//!    .wrap(ProcessGroup::leader());
//! ```
//!
//! The `wrap` method can be called multiple times to add multiple wrappers. The order of the
//! wrappers can be important, as they are applied in the order they are added. The documentation
//! for each wrapper will specify ordering concerns.
//!
//! The `spawn` method is used to spawn the process, after which the `Child` can be interacted with.
//! Methods on `Child` mimic those on `process::Child`, but may be customised by the wrappers. For
//! example, `kill` will send a signal to the process group if the `ProcessGroup` wrapper is used.
//!
//! # KillOnDrop and CreationFlags
//!
//! The options set on an underlying `Command` are not queryable from library or user code. In most
//! cases this is not an issue; however on Windows, the `JobObject` wrapper needs to know the value
//! of `.kill_on_drop()` and any `.creation_flags()` set. The `KillOnDrop` and `CreationFlags` are
//! "shims" that _should_ be used instead of the aforementioned methods on `Command`. They will
//! internally set the values on the `Command` and also store them in the wrapper, so that wrappers
//! are able to access them.
//!
//! In practice:
//!
//! ## Instead of `.kill_on_drop(true)` (Tokio-only):
//!
//! ```rust
//! use process_wrap::tokio::*;
//! let mut command = TokioCommandWrap::with_new("ls", |command| { command.arg("-l"); });
//! command.wrap(KillOnDrop);
//! ```
//!
//! ## Instead of `.creation_flags(CREATE_NO_WINDOW)` (Windows-only):
//!
//! ```rust,ignore
//! use process_wrap::std::*;
//! let mut command = StdCommandWrap::with_new("ls", |command| { command.arg("-l"); });
//! command.wrap(CreationFlags(CREATE_NO_WINDOW));
//! ```
//!
//! Internally the `JobObject` wrapper always sets the `CREATE_SUSPENDED` flag, but as it is able to
//! access the `CreationFlags` value it will either resume the process after setting up, or leave it
//! suspended if `CREATE_SUSPENDED` was explicitly set.
//!
//! # Extension
//!
//! The crate is designed to be extensible, and new wrappers can be added by implementing the
//! required traits. The std and Tokio sides are completely separate, due to the different
//! underlying APIs. Of course you can (and should) re-use/share code wherever possible if
//! implementing both.
//!
//! At minimum, you must implement [`StdCommandWrapper`](crate::std::StdCommandWrapper) and/or
//! [`TokioCommandWrapper`](crate::tokio::TokioCommandWrapper). These provide the same functionality
//! (and indeed internally are generated using a common macro), but differ in the exact types used.
//! Here's the most basic impl (shown for Tokio):
//!
//! ```rust
//! use process_wrap::tokio::*;
//! #[derive(Debug)]
//! pub struct YourWrapper;
//! impl TokioCommandWrapper for YourWrapper {}
//! ```
//!
//! The trait provides extension or hook points into the lifecycle of a `Command`:
//!
//! - **`fn extend(&mut self, other: Box<dyn TokioCommandWrapper>)`** is called if
//!   `.wrap(YourWrapper)` is done twice. Only one of a wrapper type can exist, so this gives the
//!   opportunity to incorporate all or part of the second wrapper instance into the first. By
//!   default, this does nothing (ie only the first registered wrapper instance of a type applies).
//!
//! - **`fn pre_spawn(&mut self, command: &mut Command, core: &TokioCommandWrap)`** is called before
//!   the command is spawned, and gives mutable access to it. It also gives mutable access to the
//!   wrapper instance, so state can be stored if needed. The `core` reference gives access to data
//!   from other wrappers; for example, that's how `CreationFlags` on Windows works along with
//!   `JobObject`. By default does nothing.
//!
//! - **`fn post_spawn(&mut self, child: &mut tokio::process::Child, core: &TokioCommandWrap)`** is
//!   called after spawn, and should be used for any necessary cleanups. It is offered for
//!   completeness but is expected to be less used than `wrap_child()`. By default does nothing.
//!
//! - **`fn wrap_child(&mut self, child: Box<dyn TokioChildWrapper>, core: &TokioCommandWrap)`** is
//!   called after all `post_spawn()`s have run. If your wrapper needs to override the methods on
//!   Child, then it should create an instance of its own type implementing `TokioChildWrapper` and
//!   return it here. Child wraps are _in order_: you may end up with a `Foo(Bar(Child))` or a
//!   `Bar(Foo(Child))` depending on if `.wrap(Foo).wrap(Bar)` or `.wrap(Bar).wrap(Foo)` was called.
//!   If your functionality is order-dependent, make sure to specify so in your documentation! By
//!   default does nothing: no wrapping is performed and the input `child` is returned as-is.
//!
//! # Features
//!
//! ## Frontends
//!
//! The default features do not enable a frontend, so you must choose one of the following:
//!
//! - `std`: enables the std-based API.
//! - `tokio1`: enables the Tokio-based API.
//!
//! Both can exist at the same time, but generally you'll want to use one or the other.
//!
//! ## Wrappers
//!
//! - `creation-flags`: **default**, enables the creation flags wrapper (Windows-only).
//! - `job-object`: **default**, enables the job object wrapper (Windows-only).
//! - `kill-on-drop`: **default**, enables the kill on drop wrapper (Tokio-only).
//! - `process-group`: **default**, enables the process group wrapper (Unix-only).
//! - `process-session`: **default**, enables the process session wrapper (Unix-only).
//! - `reset-sigmask`: enables the sigmask reset wrapper (Unix-only).
//!
#![doc(html_favicon_url = "https://watchexec.github.io/logo:command-group.svg")]
#![doc(html_logo_url = "https://watchexec.github.io/logo:command-group.svg")]
#![cfg_attr(docsrs, feature(doc_auto_cfg))]
#![warn(missing_docs)]

pub(crate) mod generic_wrap;

#[cfg(feature = "std")]
pub mod std;

#[cfg(feature = "tokio1")]
pub mod tokio;

#[cfg(all(
	windows,
	feature = "job-object",
	any(feature = "std", feature = "tokio1")
))]
mod windows;

/// Internal memoization of the exit status of a child process.
#[allow(dead_code)] // easier than listing exactly which featuresets use it
#[derive(Debug)]
pub(crate) enum ChildExitStatus {
	Running,
	Exited(::std::process::ExitStatus),
}
