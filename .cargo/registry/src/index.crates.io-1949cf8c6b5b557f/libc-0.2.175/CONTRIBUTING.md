# Contributing to `libc`

Welcome! If you are reading this document, it means you are interested in
contributing to the `libc` crate.

## v1.0 Roadmap

`libc` has two active branches: `main` and `libc-0.2`. `main` is for active
development of the upcoming v1.0 release, and should be the target of all pull
requests. `libc-0.2` is for updates to the currently released version.

If a pull request to `main` is a good candidate for inclusion in an `0.2.x`
release, include `@rustbot label stable-nominated` in a comment to propose this.
Good candidates will usually meet the following:

1. The included changes are non-breaking.
2. The change applies cleanly to both branches.
3. There is a usecase that justifies inclusion in a stable release (all
   additions should always have a usecase, hopefully).

Once a `stable-nominated` PR targeting `main` has merged, it can be cherry
picked to the `libc-0.2` branch. A maintainer will likely do these cherry picks
in a batch.

Alternatively, you can start this process yourself by creating a new branch
based on `libc-0.2` and running `git cherry-pick -xe commit-sha-on-main`
(`git
cherry-pick -xe start-sha^..end-sha` if a range of commits is needed).
`git` will automatically add the "cherry picked from commit" note, but try to
add a backport note so the original PR gets crosslinked:

```
# ... original commit message ...

(backport <https://github.com/rust-lang/libc/pull/1234>)             # add manually
(cherry picked from commit 104b6a4ae31c726814c36318dc718470cc96e167) # added by git
```

Once the cherry-pick is complete, open a PR targeting `libc-0.2`.

See the [tracking issue](https://github.com/rust-lang/libc/issues/3248) for
details.

## Adding an API

Want to use an API which currently isn't bound in `libc`? It's quite easy to add
one!

The internal structure of this crate is designed to minimize the number of
`#[cfg]` attributes in order to easily be able to add new items which apply to
all platforms in the future. As a result, the crate is organized hierarchically
based on platform. Each module has a number of `#[cfg]`'d children, but only one
is ever actually compiled. Each module then reexports all the contents of its
children.

This means that for each platform that libc supports, the path from a leaf
module to the root will contain all bindings for the platform in question.
Consequently, this indicates where an API should be added! Adding an API at a
particular level in the hierarchy means that it is supported on all the child
platforms of that level. For example, when adding a Unix API it should be added
to `src/unix/mod.rs`, but when adding a Linux-only API it should be added to
`src/unix/linux_like/linux/mod.rs`.

If you're not 100% sure at what level of the hierarchy an API should be added
at, fear not! This crate has CI support which tests any binding against all
platforms supported, so you'll see failures if an API is added at the wrong
level or has different signatures across platforms.

New symbol(s) (i.e. functions, constants etc.) should also be added to the
symbols list(s) found in the `libc-test/semver` directory. These lists keep
track of what symbols are public in the libc crate and ensures they remain
available between changes to the crate. If the new symbol(s) are available on
all supported Unixes it should be added to `unix.txt` list<sup>1</sup>,
otherwise they should be added to the OS specific list(s).

With that in mind, the steps for adding a new API are:

1. Determine where in the module hierarchy your API should be added.
2. Add the API, including adding new symbol(s) to the semver lists.
3. Send a PR to this repo.
4. Wait for CI to pass, fixing errors.
5. Wait for a merge!

<sup>1</sup>: Note that this list has nothing to do with any Unix or Posix
standard, it's just a list shared among all OSs that declare `#[cfg(unix)]`.

## Test before you commit

We have two automated tests running on
[GitHub Actions](https://github.com/rust-lang/libc/actions):

1. [`libc-test`](https://github.com/gnzlbg/ctest)
  - `cd libc-test && cargo test`
  - Use the `skip_*()` functions in `build.rs` if you really need a workaround.
2. Style checker
  - [`./ci/style.sh`](https://github.com/rust-lang/libc/blob/main/ci/style.sh)

## Breaking change policy

Sometimes an upstream adds a breaking change to their API e.g. removing outdated
items, changing the type signature, etc. And we probably should follow that
change to build the `libc` crate successfully. It's annoying to do the
equivalent of semver-major versioning for each such change. Instead, we mark the
item as deprecated and do the actual change after a certain period. The steps
are:

1. Add `#[deprecated(since = "", note="")]` attribute to the item.
  - The `since` field should have a next version of `libc` (e.g., if the current
    version is `0.2.1`, it should be `0.2.2`).
  - The `note` field should have a reason to deprecate and a tracking issue to
    call for comments (e.g., "We consider removing this as the upstream removed
    it. If you're using it, please comment on #XXX").
2. If we don't see any concerns for a while, do the change actually.

## Supported target policy

When Rust removes a support for a target, the libc crate also may remove the
support at any time.

## Releasing your change to crates.io

This repository uses [release-plz] to handle releases. Once your pull request
has been merged, a maintainer just needs to verify the generated changelog, then
merge the bot's release PR. This will automatically publish to crates.io!

[release-plz]: https://github.com/MarcoIeni/release-plz
