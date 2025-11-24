# Changelog

All notable changes to this project will be documented in this file.

See [conventional commits](https://www.conventionalcommits.org/) for commit guidelines.

---
## [8.2.1](https://github.com/watchexec/process-wrap/compare/v8.2.0..8.2.1) - 2025-05-15


- **Deps:** Update nix to 0.30 (#18) - ([fa80dc6](https://github.com/watchexec/process-wrap/commit/fa80dc61f8cdedaf537242100b0e48737ff872c7))
- **Deps:** Update lockfile - ([854f074](https://github.com/watchexec/process-wrap/commit/854f0742e31653ead84ffa57623b283dcbff92f1))
- **Documentation:** Mention ordering requirement - ([573f2ba](https://github.com/watchexec/process-wrap/commit/573f2ba98d48134793b24f522196f0d7abf4d87d))

---
## [8.2.0](https://github.com/watchexec/process-wrap/compare/v8.1.0..8.2.0) - 2025-01-12


- **Feature:** Add downcasting feature which adds Any trait bound to ChildWrappers - ([3e48bf8](https://github.com/watchexec/process-wrap/commit/3e48bf86e1bff9c21f8f8cbd68ef77537e3ccf2e))

---
## [8.1.0](https://github.com/watchexec/process-wrap/compare/v8.0.2..8.1.0) - 2025-01-12


- **Bugfix:** Cfg attribute scope (#10) - ([04a41f3](https://github.com/watchexec/process-wrap/commit/04a41f3c3ce1c632e5854ddfbe2fe9e0c5c7101f))
- **Deps:** Push tokio requirement to 1.38 to be able to use stable process_group() - ([ff834da](https://github.com/watchexec/process-wrap/commit/ff834da00187d294a6f779f0dc5d28363acb58df))
- **Deps:** Update lockfile - ([d8c3e1f](https://github.com/watchexec/process-wrap/commit/d8c3e1fa8d52e9df1e7b715c52ff44fc37f80b56))
- **Deps:** Upgrade nix to 0.29 - ([067ff80](https://github.com/watchexec/process-wrap/commit/067ff80779296b2aec047cab3c40d9570197901d))
- **Deps:** Upgrade windows to 0.59 - ([067ff80](https://github.com/watchexec/process-wrap/commit/067ff80779296b2aec047cab3c40d9570197901d))
- **Deps:** Bump MSRV to 1.77 - ([6ec3a36](https://github.com/watchexec/process-wrap/commit/6ec3a3631f93b98d2e95d7a4aba5ab492e3a82a2))
- **Documentation:** Add notes to indicate which deps can be bumped safely - ([067ff80](https://github.com/watchexec/process-wrap/commit/067ff80779296b2aec047cab3c40d9570197901d))
- **Feature:** Add try_clone to the child wrapper traits - ([3162a4a](https://github.com/watchexec/process-wrap/commit/3162a4abfbe3440037540bca858b8f912fb419c8))
- **Feature:** Use process_group() from std (available since Rust 1.64) - ([76de1d5](https://github.com/watchexec/process-wrap/commit/76de1d5118b04dcd7a7046e3592880e38d6a9672))
- **Tweak:** Wrap some windows handles that are safe to Send - ([657d4e3](https://github.com/watchexec/process-wrap/commit/657d4e302a399836b092452da588afb54486a1e7))

---
## [8.0.2](https://github.com/watchexec/process-wrap/compare/v8.0.1..8.0.2) - 2024-05-31


- **Deps:** Add tracing feature to remove tracing dep if wanted - ([9f64922](https://github.com/watchexec/process-wrap/commit/9f649226dd409f951a0f985cf2717cbe489deba6))

---
## [8.0.1](https://github.com/watchexec/process-wrap/compare/v8.0.0..8.0.1) - 2024-05-31


- **Deps:** Turn futures into an optional dependency (#7) - ([0c8b54b](https://github.com/watchexec/process-wrap/commit/0c8b54b0a1fcc56a63bc44ba0bebb5c1da3022f9))

---
## [8.0.0](https://github.com/watchexec/process-wrap/compare/v7.1.0..8.0.0) - 2024-04-20


- **API change:** Add explicit Send + Sync bounds - ([6580a62](https://github.com/watchexec/process-wrap/commit/6580a62ce7e67a9c24a4ff7670573865de850737))

---
## [7.1.0](https://github.com/watchexec/process-wrap/compare/v7.0.1..7.1.0) - 2024-04-20


- **Feature:** Add Clone and Copy derives where possible - ([bfd45e0](https://github.com/watchexec/process-wrap/commit/bfd45e0e400551bcd749e76149961ccb56e532fc))
- **Feature:** Add reset-sigmask wrapper - ([19d27d6](https://github.com/watchexec/process-wrap/commit/19d27d630cf136bb5d24f08974f736429677cdc8))

---
## [7.0.1](https://github.com/watchexec/process-wrap/compare/v7.0.0..7.0.1) - 2024-04-16


- **API change:** Remove re-export of nix Signal - ([a1066a7](https://github.com/watchexec/process-wrap/commit/a1066a795fe3279d7e43071ffc536082041fb16a))
- **Documentation:** Yank 7.0.0 - ([f7bbb36](https://github.com/watchexec/process-wrap/commit/f7bbb36c77618a91947fa84029cd47e7241e6fd7))
- **Documentation:** Fix doctests - ([191b8d6](https://github.com/watchexec/process-wrap/commit/191b8d61a51acbd43339aeae2f60aeb278ccef15))

---
## ~~[7.0.0](https://github.com/watchexec/process-wrap/compare/v6.0.1..7.0.0) - 2024-04-16~~ yanked


- **API change:** Remove nix types from public API - ([7de6ac0](https://github.com/watchexec/process-wrap/commit/7de6ac0a3d331d471e6cf09f0d565547cc49708f))
- **Deps:** Bump the cargo group with 1 update - ([d542c6e](https://github.com/watchexec/process-wrap/commit/d542c6ed922c8508f6af8366801c59d144330ead))
- **Deps:** Bump the cargo group with 1 update (#4) - ([fa44e91](https://github.com/watchexec/process-wrap/commit/fa44e91d0f8ba8d463812896a634a2380f4ef7f9))
- **Deps:** Upgrade windows to 0.56 - ([7f2a280](https://github.com/watchexec/process-wrap/commit/7f2a28098bb3cf028609d59a24ebd7f91a45e22c))
- **Documentation:** Fix progress->process in repo name in changelog - ([e8fb3a1](https://github.com/watchexec/process-wrap/commit/e8fb3a1ce81587f809a6773ce17297f1935e42f8))
- **Feature:** Add underlying Command accessors to *CommandWrap - ([2e2f548](https://github.com/watchexec/process-wrap/commit/2e2f548083cf2fe895c4cb03648138a89c569e2a))
- **Feature:** Add pgid accessor to ProcessGroupChild - ([2e2f548](https://github.com/watchexec/process-wrap/commit/2e2f548083cf2fe895c4cb03648138a89c569e2a))
- **Refactor:** Don't require nix::Pid on the public API - ([fd5f5e3](https://github.com/watchexec/process-wrap/commit/fd5f5e32fb9ae902e6cec2d18fe74f54b05c6dac))
- **Test:** Multiprocess behaviour - ([bbe9eed](https://github.com/watchexec/process-wrap/commit/bbe9eede1b6bb6fb522bcc80ebd21210f42c0882))
- **Test:** Multiproc tests for linux specifically - ([bd268d2](https://github.com/watchexec/process-wrap/commit/bd268d228cf0083decce6c72ff13a7ed60b74d4d))
- **Test:** Multiproc for std - ([35270b8](https://github.com/watchexec/process-wrap/commit/35270b82b9f1e5b5aa569103cfafd07ed85e9f74))

---
## [6.0.1](https://github.com/watchexec/process-wrap/compare/v6.0.0..6.0.1) - 2024-03-11


- **Bugfix:** Std doesn't have kill-on-drop - ([5eaef93](https://github.com/watchexec/process-wrap/commit/5eaef93d770ebd2c2307347f1d9a35b25a1dc2c1))
- **Bugfix:** Enable kill-on-drop (tokio only) by default - ([5eaef93](https://github.com/watchexec/process-wrap/commit/5eaef93d770ebd2c2307347f1d9a35b25a1dc2c1))
- **Documentation:** Document that kill-on-drop is Tokio-only - ([7437914](https://github.com/watchexec/process-wrap/commit/74379146c327d8ff68ac64ce224ec0c213af34c0))
- **Documentation:** Fix changelog style for 6.0.0 - ([fc158c9](https://github.com/watchexec/process-wrap/commit/fc158c90ece5ecb47f4fe014e02085d80302f660))

---
## [6.0.0](https://github.com/watchexec/process-wrap/compare/v5.0.1..6.0.0) - 2024-03-11


- **Deps:** Upgrade nix to 0.28 and windows to 0.54 - ([0d24385](https://github.com/watchexec/process-wrap/commit/0d243853cf99324a46f32cc6f08a2a6b27c9b91d))
- **Documentation:** Enable doc_auto_cfg for docsrs - ([d143b09](https://github.com/watchexec/process-wrap/commit/d143b090207608a7ec1c93df125bb096a15d2e8a))
- **Documentation:** Correct COPYRIGHT and CITATION.cff for new name - ([a04124f](https://github.com/watchexec/process-wrap/commit/a04124f0597a41ee01c63634592a971b955e659d))
- **Feature:** Instrument internals (with tracing) - ([5ee79a7](https://github.com/watchexec/process-wrap/commit/5ee79a722efcdda10776cf2c70563cc4b00cc33b))
- **Repo:** Do versions with cargo-release and git-cliff - ([869dbf8](https://github.com/watchexec/process-wrap/commit/869dbf8477f1448fb17738bf6e46785f2e8b1044))
- **Repo:** Add dependabot config - ([5a881b2](https://github.com/watchexec/process-wrap/commit/5a881b2b87ec3752f221be5c46146432b3ced3e8))
- **Repo**: Rename to `process-wrap` and rearchitect.
- **Repo**: Require Rust 1.75 for async trait in trait.
- **Tweak:** Restore Signal re-export on unix - ([fe62ff2](https://github.com/watchexec/process-wrap/commit/fe62ff22bf24a079569a081d34f7c60e068d6e54))
- **Tweak**: Windows: setting `CREATE_SUSPENDED` leaves the process suspended after spawn.

## v5.0.1 (2023-11-18)

- Use [std's `process_group()`](doc.rust-lang.org/std/os/unix/process/trait.CommandExt.html#tymethod.process_group) ([#16](https://github.com/watchexec/command-group/issues/16)).

## v5.0.0 (2023-11-18)

- Change `UnixChildExt::signal` to take `&self` instead of `&mut self`.
- Grouped child `wait`s using upstream `::wait` and `::try_wait` in addition to the internal pgid-based logic, to help with cancellation.
- Optimisations in `tokio::Child::wait()`. ([#25](https://github.com/watchexec/command-group/issues/25), [#26](https://github.com/watchexec/command-group/issues/26))

## v4.1.0 (2023-11-05)

- Add `ErasedChild::id()` method.

## v4.0.0 (2023-11-05)

- Clarify why and in which situations `AsyncGroupChild::wait` may not behave as expected when cancelled.
- Add `AsyncGroupChild::start_kill` to align with Tokio's `Child::start_kill`.
- Change `AsyncGroupChild::kill` to also `wait()` on the child, to align with Tokio's `Child::kill`.
- Add `ErasedChild` abstraction to allow using the same type for grouped and ungrouped children.

## v3.0.0 (2023-10-30)

- Update `nix` to 0.27.
- Increase MSRV to 1.68 (within policy).
- Add note to documentation for Tokio `Child::wait` wrt cancel safety. ([#21](https://github.com/watchexec/command-group/issues/21))

## v2.1.1 (2023-10-30)

(Same as 3.0.0, but yanked due to breakage.)

## v2.1.0 (2023-03-04)

- Add new `.group()` builder API to allow setting Windows flags and use `kill_on_drop`. ([#15](https://github.com/watchexec/command-group/issues/15), [#17](https://github.com/watchexec/command-group/issues/17), [#18](https://github.com/watchexec/command-group/issues/18))

## v2.0.1 (2022-12-28)

- Fix bug on Windows where the wrong pointer was being null checked, leading to timeout errors. ([#13](https://github.com/watchexec/command-group/pull/13))

## v2.0.0 (2022-12-04)

- Increase MSRV to 1.60.0 and change policy for increasing it (no longer a breaking change).
- Wait for all processes in the process group, avoiding zombies. ([#7](https://github.com/watchexec/command-group/pull/7))
- Update `nix` to 0.26 and limit features. ([#8](https://github.com/watchexec/command-group/pull/8))

## v1.0.8 (2021-10-16)

- Bugfix: compiling would fail when Tokio was missing the `io-util` feature (not `io-std`).

## v1.0.7 (2021-10-16) (yanked)

- Bugfix: compiling would fail when Tokio was missing the `io-std` feature.

## v1.0.6 (2021-08-26)

- Correctly handle timeouts on Windows. ([#2](https://github.com/watchexec/command-group/issues/2), [#3](https://github.com/watchexec/command-group/pull/3))

## v1.0.5 (2021-08-13)

- Internal: change usage of `feature = "tokio"` to `feature = "with-tokio"`.
- Documentation: remove wrong mention of blocking reads on `AsyncGroupChild::wait_with_output()`.

## v1.0.4 (2021-07-26)

New: Tokio implementation, gated on the `with-tokio` feature.

## v1.0.3 (2021-07-21)

Bugfix: `GroupChild::try_wait()` would error if called after a child exited by itself.

## v1.0.2 (2021-07-21)

Bugfix: `GroupChild::try_wait()` and `::wait()` could not be called twice.

## v1.0.1 (2021-07-21)

Implement `Send`+`Sync` on `GroupChild` on Windows, and add a `Drop` implementation to close handles
too (whoops). Do our best when `.into_inner()` is used...

## v1.0.0 (2021-07-20)

Initial release
