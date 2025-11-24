# Landlock changelog

## [v0.4.2](https://github.com/landlock-lsm/rust-landlock/releases/tag/v0.4.2)

### New API

- Added support for Landlock ABI 6: control abstract UNIX sockets and signal scoping with the new [`Ruleset::scope()`](https://landlock.io/rust-landlock/landlock/struct.Ruleset.html#method.scope) method taking a [`Scope`](https://landlock.io/rust-landlock/landlock/enum.Scope.html) enum ([PR #96](https://github.com/landlock-lsm/rust-landlock/pull/96) and [PR #98](https://github.com/landlock-lsm/rust-landlock/pull/98)).
- Added `From<RulesetCreated>` implementation for `Option<OwnedFd>` ([PR #104](https://github.com/landlock-lsm/rust-landlock/pull/104))
- Introduced a new [`HandledAccess`](https://landlock.io/rust-landlock/landlock/trait.HandledAccess.html) trait specific to `AccessFs` and `AccessNet` (commit [554217dda0b7](https://github.com/landlock-lsm/rust-landlock/commit/554217dda0b775756e38db71f471dd414b199234)).
- Added a new [`Errno`](https://landlock.io/rust-landlock/landlock/struct.Errno.html) type to improve FFI support ([PR #86](https://github.com/landlock-lsm/rust-landlock/pull/86) and [PR #102](https://github.com/landlock-lsm/rust-landlock/pull/102)).
- Exposed `From<i32>` implementation for [`ABI`](https://landlock.io/rust-landlock/landlock/enum.ABI.html) ([PR #88](https://github.com/landlock-lsm/rust-landlock/pull/88)).

### Documentation

- Added clarifying notes about `AccessFs::WriteFile` behavior and `path_beneath_rules` usage ([PR #80](https://github.com/landlock-lsm/rust-landlock/pull/80)).
- Introduced [CONTRIBUTING.md](CONTRIBUTING.md) with testing workflow explanations ([PR #76](https://github.com/landlock-lsm/rust-landlock/pull/76)).

### Testing

- Enhanced test coverage for new API and added testing against Linux 6.12 ([PR #96](https://github.com/landlock-lsm/rust-landlock/pull/96)).
- Updated CI configuration to use the latest Ubuntu versions ([PR #87](https://github.com/landlock-lsm/rust-landlock/pull/87) and [PR #97](https://github.com/landlock-lsm/rust-landlock/pull/97)).
- Modified default `LANDLOCK_CRATE_TEST_ABI` to match the current kernel for more convenient local testing ([PR #76](https://github.com/landlock-lsm/rust-landlock/pull/76)).

### Example

- Synchronized the sandboxer example with the C version ([PR #101](https://github.com/landlock-lsm/rust-landlock/pull/101)): improved error handling for inaccessible file paths and enhanced help documentation.

## [v0.4.1](https://github.com/landlock-lsm/rust-landlock/releases/tag/v0.4.1)

### New API

Add support for Landlock ABI 5: control IOCTL commands on character and block devices with the new [`AccessFs::IoctlDev`](https://landlock.io/rust-landlock/landlock/enum.AccessFs.html#variant.IoctlDev) right ([PR #74](https://github.com/landlock-lsm/rust-landlock/pull/74)).

### Testing

Improved the CI to better test against different kernel versions ([PR #72](https://github.com/landlock-lsm/rust-landlock/pull/72)).


## [v0.4.0](https://github.com/landlock-lsm/rust-landlock/releases/tag/v0.4.0)

### New API

Add support for Landlock ABI 4: control TCP binding and connection according to specified network ports.
This is now possible with the [`AccessNet`](https://landlock.io/rust-landlock/landlock/enum.AccessNet.html) rights and
the [`NetPort`](https://landlock.io/rust-landlock/landlock/struct.NetPort.html) rule
([PR #55](https://github.com/landlock-lsm/rust-landlock/pull/55)).

### Breaking change

The `from_read()` and `from_write()` methods moved from the `Access` trait to the `AccessFs` struct
([commit 68f066eba571](https://github.com/landlock-lsm/rust-landlock/commit/68f066eba571c1f9212f5a07016aac9ffb0d1c27)).

### Compatibility management

Improve compatibility consistency and prioritize runtime errors against compatibility errors
([PR #67](https://github.com/landlock-lsm/rust-landlock/pull/67)).

Fixed a corner case where a ruleset was created on a kernel not supporting Landlock, while requesting to add a rule with an access right handled by the ruleset (`BestEffort`).
When trying to enforce this ruleset, this led to a runtime error (i.e. wrong file descriptor) instead of a compatibility error.

To simplify compatibility management, always call `prctl(PR_SET_NO_NEW_PRIVS, 1)` by default (see `set_no_new_privs()`).
This was required to get a consistent compatibility management and it should not be an issue given that this feature is supported by all LTS kernels
([commit d99f75155bec](https://github.com/landlock-lsm/rust-landlock/commit/d99f75155bec2040cf4ce1532007cd3b8a23e2fb)).


## [v0.3.1](https://github.com/landlock-lsm/rust-landlock/releases/tag/v0.3.1)

### New API

Add [`RulesetCreated::try_clone()`](https://landlock.io/rust-landlock/landlock/struct.RulesetCreated.html#method.try_clone) ([PR #38](https://github.com/landlock-lsm/rust-landlock/pull/38)).


## [v0.3.0](https://github.com/landlock-lsm/rust-landlock/releases/tag/v0.3.0)

### New API

Add support for Landlock ABI 3: control truncate operations with the new
[`AccessFs::Truncate`](https://landlock.io/rust-landlock/landlock/enum.AccessFs.html#variant.Truncate)
right ([PR #40](https://github.com/landlock-lsm/rust-landlock/pull/40)).

Revamp the compatibility handling and add a new
[`set_compatibility()`](https://landlock.io/rust-landlock/landlock/trait.Compatible.html#method.set_compatibility)
method for `Ruleset`, `RulesetCreated`, and `PathBeneath`.
We can now fine-tune the compatibility behavior according to the running kernel
and then the supported features thanks to three compatible levels:
best effort, soft requirement and hard requirement
([PR #12](https://github.com/landlock-lsm/rust-landlock/pull/12)).

Add a new [`AccessFs::from_file()`](https://landlock.io/rust-landlock/landlock/enum.AccessFs.html#method.from_file)
helper ([commit 0b3238c6dd70](https://github.com/landlock-lsm/rust-landlock/commit/0b3238c6dd70)).

### Deprecated API

Deprecate the [`set_best_effort()`](https://landlock.io/rust-landlock/landlock/trait.Compatible.html#method.set_best_effort)
method and replace it with `set_compatibility()`
([PR #12](https://github.com/landlock-lsm/rust-landlock/pull/12)).

Deprecate [`Ruleset::new()`](https://landlock.io/rust-landlock/landlock/struct.Ruleset.html#method.new)
and replace it with `Ruleset::default()`
([PR #44](https://github.com/landlock-lsm/rust-landlock/pull/44)).

### Breaking changes

We now check that a ruleset really handles at least one access right,
which can now cause `Ruleset::create()` to return an error if the ruleset compatibility level is
`HardRequirement` or `set_best_effort(false)`
([commit 95addc13b4a8](https://github.com/landlock-lsm/rust-landlock/commit/95addc13b4a8)).

We now check that access rights passed to `add_rule()` make sense according to the file type.
To handle most use cases,
`path_beneath_rules()` now automatically check and downgrade access rights for files
(i.e. remove superfluous directory-only access rights,
 [commit 8e47940b3722](https://github.com/landlock-lsm/rust-landlock/commit/8e47940b3722)).

### Testing

Test coverage in the CI is greatly improved by running all tests on all relevant kernel versions:
Linux 5.10, 5.15, 6.1, and 6.4
([PR #41](https://github.com/landlock-lsm/rust-landlock/pull/41)).

Run each test in a dedicated thread to avoid inconsistent behavior
([PR #46](https://github.com/landlock-lsm/rust-landlock/pull/46)).


## [v0.2.0](https://github.com/landlock-lsm/rust-landlock/releases/tag/v0.2.0)

This is the first major release of this crate.
It brings a high-level interface to the Landlock kernel interface.
