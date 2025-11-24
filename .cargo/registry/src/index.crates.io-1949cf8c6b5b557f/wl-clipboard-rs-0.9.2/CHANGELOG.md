# Changelog

## Unreleased

## v0.9.2 (14th Mar 2025)

- Added support for the `ext-data-control` protocol. It will be used instead of `wlr-data-control` when available.
- Updated dependencies.

## v0.9.1 (6th Oct 2024)

- Added man page and shell completion generation to `wl-clipboard-rs-tools`.
- Updated dependencies.

## v0.9.0 (19th June 2024)

- **Breaking** Removed `utils::copy_data`. It forked into a `/usr/bin/env cat`
  for copying. All internal uses of the function have been changed to simply
  use `std::io::copy` instead.
- Replaced `nix` with `rustix`, following `wayland-rs`.
- Replaced the deprecated `structopt` with `clap` itself.
- Updated dependencies.

## v0.8.1 (7th Mar 2024)

- Updated dependencies, notably `nix`, which fixes building on LoongArch.

## v0.8.0 (3rd Oct 2023)

- Added `copy::Options::omit_additional_text_mime_types` to disable
  wl-clipboard-rs offering several known text MIME types when a text MIME type
  is copied.
- Updated `wayland-rs` to 0.31.
  - **Breaking** This changed the error types slightly. However, most uses of
    wl-clipboard-rs should be completely unaffected.
- Updated other dependencies.

## v0.7.0 (23rd Sep 2022)

- Fixed `paste::get_contents()` leaving behind zombie `cat` processes.
- Changed debug logging from `info!` to `trace!`.
- Bumped `nix` dependency to `0.24` to match that of the wayland-rs crates.
- Replaced `derive_more` with `thiserror`.

## v0.6.0 (20th Mar 2022)

- Fixed `wl-copy` and `wl-clip` hangs when followed by a pipe (e.g. `wl-copy
  hello | cat`).
- Removed the deprecated `failure` dependency from both the library and the
  tools. The standard `Error` trait is now used.
- Replaced underscores back with dashes in the tool binary names.
- Renamed `wl-clipboard-tools` subcrate to `wl-clipboard-rs-tools`.

## v0.5.0 (13th Mar 2022)

- Split binaries from the main crate `wl-clipboard-rs` into a new sub-crate
  `wl-clipboard-tools`. This removes a few dependencies that were only used in
  the binaries (like `structopt`).
  - This change also unintentionally replaced dashes with underscores in tool
    binary names.
- Replaced `tree_magic` (which went unmaintained) with `tree_magic_mini`.
- Changed the `fork` code which runs during the copy operation to exec
  `/usr/bin/env cat` instead of just `cat`. This was done to remove
  a non-async-signal-safe call in the child process.
- Updated dependencies.

## v0.4.1 (1st Sep 2020)

- Updated `nix` to 0.18 and `wayland-rs` to 0.27.

## v0.4.0 (13th Dec 2019)

- **Breaking** Copying in non-foreground mode no longer forks (which was
  **unsafe** in multi-threaded programs). Instead, it spawns a background
  thread to serve copy requests.
- Added `copy::prepare_copy()` and `copy::prepare_copy_multi()` (and respective
  functions in `copy::Options`) to accommodate workflows which depended on the
  forking behavior, such as `wl-copy`. See `wl-copy` for example usage.
- **Breaking** Changed `copy::Source` and `copy::Seat` to own the contained
  data rather than borrow it. As a consequence, those types, as well as
  `copy::MimeSource` and `copy::Options`, have dropped their lifetime generic
  parameter.

## v0.3.1 (27th Nov 2019)

- Reduced the `wl_seat` version requirement from 6 to 2.
- Added `copy::copy_multi()` for offering multiple data sources under multiple
  different MIME types.

## v0.3.0 (4th Apr 2019)

- **Breaking** Moved `ClipboardType` into `copy::` and `paste::`.
- **Breaking** Renamed `utils::Error` into `utils::CopyDataError`.
- Added `copy::ClipboardType::Both` for operating both clipboards at once.
- Added `utils::is_primary_selection_supported()`.
- [wl-copy]: added `--regular`, which, when set together with `--primary`,
  makes `wl-copy` operate on both clipboards at once.

## v0.2.0 (17th Feb 2019)

- **Breaking** Changed `copy::Options::paste_once` to `serve_requests` which
  allows to specify the number of paste requests to serve.
- Marked `copy::Seat` and `copy::Options` as `Copy`.
- Updated `data-control`, it's now merged into `wlr-protocols` so no further
  changes without a version bump.
- [wl-copy, wl-paste]: replaced `env_logger` with `stderrlog` which made the
  binaries much smaller.
- Implemented `wl-clip`, a Wayland version of `xclip`.

## v0.1.0 (12th Feb 2019)

- Initial release.
