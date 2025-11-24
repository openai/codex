[![Crate release version](https://flat.badgen.net/crates/v/process-wrap)](https://crates.io/crates/process-wrap)
[![Crate license: Apache 2.0 or MIT](https://flat.badgen.net/badge/license/Apache%202.0%20or%20MIT)][copyright]
[![CI status](https://github.com/watchexec/process-wrap/actions/workflows/test.yml/badge.svg)](https://github.com/watchexec/process-wrap/actions/workflows/test.yml)

# process-wrap

- **[API documentation][docs]**.
- [Dual-licensed][copyright] with Apache 2.0 and MIT.
- Successor to [command-group](https://github.com/watchexec/command-group).
- Minimum Supported Rust Version: 1.77.0.
  - Only the latest stable rustc version is supported.
  - MSRV increases will not incur major version bumps.

[copyright]: ./COPYRIGHT
[docs]: https://docs.rs/process-wrap

Unlike command-group, process-wrap doesn't implement a single cross-platform API.
Instead, it provides composable wrappers which implement a single concern each.
It is left to the developer to use the appropriate wrapper(s) for their use-case and platform.

As the successor to (and containing a lot of the code of) command-group, versioning starts at 6.0.0.
You can think of it as a breaking change to command-group, though the paradigm is quite different.
The full test suite from command-group was retained: process-wrap has parity on functionality as a starting point.

## Quick start

```toml
[dependencies]
process-wrap = { version = "8.2.1", features = ["tokio1"] }
```

By default, the crate does nothing, you need to enable either the std or Tokio "frontend". A default
set of wrappers are enabled; you may choose to only compile those you need, see [the features list].

```rust
use tokio::process::Command;
use process_wrap::tokio::*;

let mut child = TokioCommandWrap::with_new("watch", |command| { command.arg("ls"); })
  .wrap(ProcessGroup::leader())
  .spawn()?;
let status = Box::into_pin(child.wait()).await?;
dbg!(status);
```

### or on Windows

```rust
use tokio::process::Command;
use process_wrap::tokio::*;

let mut child = TokioCommandWrap::with_new("watch", |command| { command.arg("ls"); })
  .wrap(JobObject::new())
  .spawn()?;
let status = Box::into_pin(child.wait()).await?;
dbg!(status);
```

### or with sessions

```rust
use tokio::process::Command;
use process_wrap::tokio::*;

let mut child = TokioCommandWrap::with_new("watch", |command| { command.arg("ls"); })
  .wrap(ProcessSession)
  .spawn()?;
let status = Box::into_pin(child.wait()).await?;
dbg!(status);
```

### or with multiple wrappers

```rust
use tokio::process::Command;
use process_wrap::tokio::*;

let mut child = TokioCommandWrap::with_new("watch", |command| { command.arg("ls"); })
  .wrap(ProcessSession)
  .wrap(KillOnDrop)
  .spawn()?;
let status = Box::into_pin(child.wait()).await?;
dbg!(status);
```

### or with std

```toml
[dependencies]
process-wrap = { version = "8.2.1", features = ["std"] }
```

```rust
use std::process::Command;
use process_wrap::std::*;

let mut child = StdCommandWrap::with_new("watch", |command| { command.arg("ls"); })
  .wrap(ProcessGroup::leader())
  .spawn()?;
let status = child.wait()?;
dbg!(status);
```

## Wrappers

### Job object

- Platforms: Windows
- Like command-group.
- Feature: `job-object` (default)

```rust
TokioCommandWrap::with_new("watch", |command| { command.arg("ls"); })
  .wrap(JobObject)
  .spawn()?;
```

When both `CreationFlags` and `JobObject` are used together, `CreationFlags` must come first.

### Process group

- Platforms: POSIX (Linux, Mac, BSDs...)
- Like command-group >=5.0.0.
- Feature: `process-group` (default)

```rust
TokioCommandWrap::with_new("watch", |command| { command.arg("ls"); })
  .wrap(ProcessGroup::leader())
  .spawn()?;
```

Or join a different group instead:

```rust
TokioCommandWrap::with_new("watch", |command| { command.arg("ls"); })
  .wrap(ProcessGroup::attach_to(pgid))
  .spawn()?;
```

For Windows process groups, use `CreationFlags::NEW_PROCESS_GROUP` and/or `JobObject::new()`.

### Process session

- Platforms: POSIX (Linux, Mac, BSDs...)
- Like command-group <5.0.0.
- Feature: `process-session` (default)

This combines creating a new session and a new group, and setting this process as leader.
To join the session from another process, use `ProcessGroup::attach_to()` instead.

```rust
TokioCommandWrap::with_new("watch", |command| { command.arg("ls"); })
  .wrap(ProcessSession)
  .spawn()?;
```

### Reset signal mask

- Platforms: POSIX (Linux, Mac, BSDs...)
- Feature: `reset-sigmask`

This resets the [signal mask] of the process instead of inheriting it from the parent.

[signal mask]: https://www.man7.org/linux/man-pages/man2/sigprocmask.2.html

```rust
TokioCommandWrap::with_new("watch", |command| { command.arg("ls"); })
  .wrap(ResetSigmask)
  .spawn()?;
```

### Creation flags

- Platforms: Windows
- Like command-group.
- Feature: `creation-flags` (default)

This is a shim to allow setting Windows process creation flags with this API, as otherwise they'd be overwritten.

```rust
use windows::Win32::System::Threading::*;
TokioCommandWrap::with_new("watch", |command| { command.arg("ls"); })
  .wrap(CreationFlags(CREATE_NO_WINDOW | CREATE_DETACHED))
  .wrap(JobObject)
  .spawn()?;
```

When both `CreationFlags` and `JobObject` are used together, `CreationFlags` must come first.

### Kill on drop

- Platforms: all (Tokio-only)
- Like command-group.
- Feature: `kill-on-drop` (default)

This is a shim to allow wrappers to handle the kill-on-drop flag, as it can't be read from Command.

```rust
let child = TokioCommandWrap::with_new("watch", |command| { command.arg("ls"); })
  .wrap(KillOnDrop)
  .wrap(ProcessGroup::leader())
  .spawn()?;
drop(child);
```

### Your own

Implementing a wrapper is done via a set of traits.
The std and Tokio sides are completely separate, due to the different underlying APIs.
Of course you can (and should) re-use/share code wherever possible if implementing both.

At minimum, you must implement `StdCommandWrapper` and/or `TokioCommandWrapper`.
These provide the same functionality, but differ in the exact types specified.
Here's the most basic impl (shown for Tokio):

```rust
#[derive(Debug)]
pub struct YourWrapper;
impl TokioCommandWrapper for YourWrapper {}
```

That's right, all member methods are optional.
The trait provides extension or hook points into the lifecycle of a `Command`:

- **`fn extend(&mut self, other: Box<dyn TokioCommandWrapper>)`** is called if `.wrap(YourWrapper)`
  is done twice. Only one of a wrapper type can exist, so this gives the opportunity to incorporate
  all or part of the second wrapper instance into the first. By default, this does nothing (ie only
  the first registered wrapper instance of a type does anything).

- **`fn pre_spawn(&mut self, command: &mut Command, core: &TokioCommandWrap)`** is called before the
  command is spawned, and gives mutable access to it. It also gives mutable access to the wrapper
  instance, so state can be stored if needed. The `core` reference gives access to data from other
  wrappers; for example, that's how `CreationFlags` on Windows works along with `JobObject`. Noop by
  default.

- **`fn post_spawn(&mut self, child: &mut tokio::process::Child, core: &TokioCommandWrap)`** is
  called after spawn, and should be used for any necessary cleanups. It is offered for completedness
  but is expected to be less used than `wrap_child()`. Noop by default.

- **`fn wrap_child(&mut self, child: Box<dyn TokioChildWrapper>, core: &TokioCommandWrap)`** is
  called after all `post_spawn()`s have run. If your wrapper needs to override the methods on Child,
  then it should create an instance of its own type implementing `TokioChildWrapper` and return it
  here. Child wraps are _in order_: you may end up with a `Foo(Bar(Child))` or a `Bar(Foo(Child))`
  depending on if `.wrap(Foo).wrap(Bar)` or `.wrap(Bar).wrap(Foo)` was called. If your functionality
  is order-dependent, make sure to specify so in your documentation! Default is noop: no wrapping is
  performed and the input `child` is returned as-is.

Refer to [the API documentation][docs] for more detail and the specifics of child wrapper traits.

## Features
[the features list]: #features

### Frontends

- `std`: enables the std-based API.
- `tokio1`: enables the Tokio-based API.

Both can exist at the same time, but generally you should use one or the other.

### Wrappers

- `creation-flags`: **default**, enables the [creation flags](#creation-flags) wrapper.
- `job-object`: **default**, enables the [job object](#job-object) wrapper.
- `kill-on-drop`: **default**, enables the [kill on drop](#kill-on-drop) wrapper.
- `process-group`: **default**, enables the [process group](#process-group) wrapper.
- `process-session`: **default**, enables the [process session](#process-session) wrapper.
- `reset-sigmask`: enables the [reset signal mask](#reset-signal-mask) wrapper.
