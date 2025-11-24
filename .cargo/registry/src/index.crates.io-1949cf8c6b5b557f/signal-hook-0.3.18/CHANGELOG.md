# signal-hook-1.3.18

* Release the special-case removal of AIX for top-level signal-hook too (#169,
  #176).

# signal-hook-async-std-0.3.0

* Bump async-std to 0.2 (#172).

# signal-hook-registry-1.4.5

* Fix windows build (#174).

# signal-hook-registry-1.4.4

* Get rid of a warning.

# signal-hook-registry-1.4.3

* Remove special-case for AIX (#169).

# signal-hook-mio-0.2.4

* Support for mio 1.0

# signal-hook-registry-1.4.2

* NQX support (experimental/not guaranteed to work) (#158)
  - By disabling `SA_RESTART` on that platform, not supported there.

# 0.3.17

* Fix race condition leading into a panic in SignalsInfo::forever (#148).

# 0.3.16

* Fix compilation on OpenBSD (#147).

# 0.3.15
# signal-hook-registry-1.4.1

* AIX support (experimental/not guaranteed to work).

# 0.3.14

* Added the SIGINFO signal (where available).

# signal-hook-mio-0.2.3

* Support for mio 0.8

# signal-hook-async-std-0.2.2
# signal-hook-tokio-0.3.1

* Fix support for SignalsInfo with non-default info extractors.

# 0.3.13

* Add haiku support.

# 0.3.12

* Fix accidentally broken windows build.

# 0.3.11

* Provide fallback sigaddset, sigemptyset on certain androids, as they are
  missing them.

# 0.3.10

* Doc link fixes.

# 0.3.9

* Deliver SIGCHLD even on stop/continue.

# 0.3.8

* Fix docs.rs build.

# 0.3.7

* Unmask a signal in default emulation if it is termination.

# mio-0.2.2

* The same fix, but for the 0.6 support 😇.

# mio-0.2.1

* Fix example: handle ErrorKind::Interrupted inside poll. It's very likely to
  happen, when we are waiting for signals there.

# 0.3.6

* Fix the labels on docs.rs :-|.

# 0.3.5

* Doc: include the features & these little labels inside docs.

# signal-hook-async-std-0.2.1

* Dependency updates ‒ no longer depends on the whole async-std, but only on
  some smaller dependencies of it (`async-io`, `futures-lite`). This might make
  it work even outside of async-std context.

# signal-hook-tokio-0.3.0

* Support for tokio 1.0.

# 0.3.4

* Fix feature dependencies (`iterator` depends on `channel`).

# 0.3.3

* `low_level::emulate_default_handler` to emulate whatever default handler would
  do.
* `low_level::signal_name` to look up human readable name.
* The `Origin`'s debug output now contains the human readable name of the
  signal.

# 0.3.2

* Allow extracting Origin from the raw `siginfo_t` structure by hand, without
  needing an iterator.
* Folding the signal-hook-sys inline (but still compiling C code only
  conditionally).
* `WithRawSiginfo` extractor (to get hands on the raw `siginfo_t`).
* Bugfix: Don't leak on WithOrigin destruction.

# 0.3.1

* Use caret dependencies where appropriate (to allow upgrades on
  signal-hook-registry).

# async-std-0.2.0

* No longer depends on `futures`.

# 0.3.0

* The `cleanup` module is gone, it was not a good API. Replaced by conditional
  termination in `flag`.
* Some abstractions/patterns are moved to `low_level` submodule, as they are
  considered building blocks, not for direct use (`register`, `pipe`,
  `channel`).
* The signal constants are moved to a submodule (`consts`), together with few
  more constants, to not clutter the root.
* The forever iterator no longer consumes.

# registry-1.3.0

* The `unregister_signal` in is deprecated, without a replacement.

# 0.2.2

* Extractor for the origin of a signal (PID, UID, what caused it).
* Fixing some doc links on re-exports.

# 0.2.1

* Allow turning the iterator module off (the `iterator` feature, part of default
  features). This would allow compiling the crate on 1.31.0.

# 0.2.0

* Bump minimal rustc version to 1.36.0 (signal-hook-registry still builds with
  1.26.0).
* (Breaking) Support for exfiltrators ‒ ability to return more than just the
  signal number from the iterator and streams. Nothing more is implemented yet,
  but the place to put it is reserved in the API.
* (Breaking) `pipe::register_raw` now takes ownership and tries to use send
  first, falls back to `O_NONBLOCK` and `write` on failure.
* (Breaking) All async support is pulled out into separate crates, to decouple
  from the async runtime release cycles on the main `signal-hook` crate.
* Inner parts of the `Iterator` are now exposed in
  `signal_hook::iterator::backend`, to support the async crates.

# registry-1.2.2

* Drop dependency on arc-swap (only very small subset used and arc-swap would
  like to drop that part anyway).

# registry-1.2.1

* Abort instead of panicking if the OS gives us NULL as siginfo (which is
  illegal). Panicking would be UB.

# 0.1.16

* Fix possible blocking in signal handler registered by `Signals`.

# 0.1.15

* Make `Signals` work in edge-triggered mode in mio too, by always draining
  everything from the socket. Needed, because mio 0.7 doesn't have
  level-triggered any more.

# 0.1.14

* `mio-0_7-support` feature for use with mio 0.7.0+.
* Bump minimal rustc version to 1.31.0 (signal-hook-registry can still build
  with 1.26.0).

# 0.1.13

* Some doc clarifications.

# 0.1.12

* `cleanup` module to register resetting signals to default.

# registry-1.2.0

* `unregister_signal`, to remove all hooks of one signal.

# 0.1.11

* Docs improvements.
* Fix registering pipes as well as sockets into the pipe module (#27).

# registry-1.1.1

* Update deps.

# registry-1.1.0

* Adding Windows support (thanks to @qnighy).

# 0.1.10

* Fix busy loop in Iterator::forever when the mio-support feature is enabled
  (#16).

# registry-1.0.1

* Include the registry files in the crates.io tarball.

# 0.1.9
# registry-1.0.0

* Split into backend signal-hook-registry and the frontend. The backend is much
  less likely to have breaking changes so it contains the things that can be in
  the application just once.

# 0.1.8

* The `Signals` iterator can now be closed (from another instance or thread),
  which can be used to shut down the thread handling signals from the main
  thread.

# 0.1.7

* The `Signals` iterator allows adding signals after creation.
* Fixed a bug where `Signals` registrations could be unregirestered too soon if
  the `Signals` was cloned previously.

# 0.1.6

* The internally used ArcSwap thing doesn't block other ArcSwaps now (has
  independent generation lock).

# 0.1.5

* Re-exported signal constants, so users no longer need libc.

# 0.1.4

* Compilation fix for android-aarch64

# 0.1.3

* Tokio support.
* Mio support.
* Dependency updates.

# 0.1.2

* Dependency updates.

# 0.1.1

* Get rid of `catch_unwind` inside the signal handler.
* Link to the nix crate.

# 0.1.0

* Initial basic implementation.
* Flag helpers.
* Pipe helpers.
* High-level iterator helper.
