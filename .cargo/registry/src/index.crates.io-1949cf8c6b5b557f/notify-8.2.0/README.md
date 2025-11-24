# Notify

[![» Crate](https://flat.badgen.net/crates/v/notify)][crate]
[![» Docs](https://flat.badgen.net/badge/api/docs.rs/df3600)][notify-docs]
[![» CI](https://flat.badgen.net/github/checks/notify-rs/notify/main)][build]
[![» Downloads](https://flat.badgen.net/crates/d/notify)][crate]
[![» Conduct](https://flat.badgen.net/badge/contributor/covenant/5e0d73)][coc]
[![» Public Domain](https://flat.badgen.net/badge/license/CC0-1.0/purple)][cc0]

_Cross-platform filesystem notification library for Rust._


- [Notify Documentation][notify-docs]
- [Notify Types Documentation][notify-types-docs]
- [Mini Debouncer Documentation][debouncer-mini-docs]
- [Full Debouncer Documentation][debouncer-full-docs]
- [File ID][file-id-docs]
- [Examples][examples]
- [Changelog][changelog]
- [Upgrading notify from v4](UPGRADING_V4_TO_V5.md)
- Minimum supported Rust version: **1.77**

As used by: [alacritty], [cargo watch], [cobalt], [deno], [docket], [mdBook],
[rust-analyzer], [watchexec], [watchfiles], [xi-editor],
and others.

(Looking for desktop notifications instead? Have a look at [notify-rust] or
[alert-after]!)

## Platforms

- Linux / Android: inotify
- macOS: FSEvents or kqueue, see features
- Windows: ReadDirectoryChangesW
- iOS / FreeBSD / NetBSD / OpenBSD / DragonflyBSD: kqueue
- All platforms: polling

## License

notify is licensed under the [CC Zero 1.0][cc0].    
notify-types is licensed under the [MIT] or [Apache-2.0][apache] license.  
notify-debouncer-mini is licensed under the [MIT] or [Apache-2.0][apache] license.  
notify-debouncer-full is licensed under the [MIT] or [Apache-2.0][apache] license.  
file-id is licensed under the [MIT] or [Apache-2.0][apache] license.

## Origins

Inspired by Go's [fsnotify] and Node.js's [Chokidar], born out of need for
[cargo watch], and general frustration at the non-existence of C/Rust
cross-platform notify libraries.

Originally created by [Félix Saparelli] and awesome [contributors].

[Chokidar]: https://github.com/paulmillr/chokidar
[FileSystemEventSecurity]: https://developer.apple.com/library/mac/documentation/Darwin/Conceptual/FSEvents_ProgGuide/FileSystemEventSecurity/FileSystemEventSecurity.html
[debouncer-full-docs]: https://docs.rs/notify-debouncer-full/latest/notify_debouncer_full/
[debouncer-mini-docs]: https://docs.rs/notify-debouncer-mini/latest/notify_debouncer_mini/
[Félix Saparelli]: https://passcod.name
[alacritty]: https://github.com/jwilm/alacritty
[alert-after]: https://github.com/frewsxcv/alert-after
[build]: https://github.com/notify-rs/notify/actions
[cargo watch]: https://github.com/passcod/cargo-watch
[cc0]: ./notify/LICENSE-CC0
[MIT]: ./file-id/LICENSE-MIT
[apache]: ./file-id/LICENSE-APACHE
[changelog]: ./CHANGELOG.md
[cobalt]: https://github.com/cobalt-org/cobalt.rs
[coc]: http://contributor-covenant.org/version/1/4/
[contributors]: https://github.com/notify-rs/notify/graphs/contributors
[crate]: https://crates.io/crates/notify
[deno]: https://github.com/denoland/deno
[docket]: https://iwillspeak.github.io/docket/
[notify-docs]: https://docs.rs/notify/latest/notify/
[notify-types-docs]: https://docs.rs/notify-types/latest/notify-types/
[file-id-docs]: https://docs.rs/file-id/latest/file_id/
[fsnotify]: https://github.com/fsnotify/fsnotify
[handlebars-iron]: https://github.com/sunng87/handlebars-iron
[hotwatch]: https://github.com/francesca64/hotwatch
[mdBook]: https://github.com/rust-lang-nursery/mdBook
[notify-rust]: https://github.com/hoodie/notify-rust
[rust-analyzer]: https://github.com/rust-analyzer/rust-analyzer
[serde]: https://serde.rs/
[watchexec]: https://github.com/mattgreen/watchexec
[wiki]: https://github.com/notify-rs/notify/wiki
[xi-editor]: https://xi-editor.io/
[watchfiles]: https://watchfiles.helpmanual.io/
[examples]: examples/
