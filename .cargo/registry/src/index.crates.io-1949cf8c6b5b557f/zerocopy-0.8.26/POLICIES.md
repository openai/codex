<!-- Copyright 2023 The Fuchsia Authors

Licensed under a BSD-style license <LICENSE-BSD>, Apache License, Version 2.0
<LICENSE-APACHE or https://www.apache.org/licenses/LICENSE-2.0>, or the MIT
license <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your option.
This file may not be copied, modified, or distributed except according to
those terms. -->

# Zerocopy's Policies

## Soundness

Zerocopy is expressly designed for use in security-critical contexts. It is used
in hardware security firmware, cryptographic implementations, hypervisors, and
more. We understand that software in these contexts has a very high bar for
correctness, and we take our responsibility to meet that bar very seriously.

This section describes policies which are designed to ensure the correctness and
soundness of our code and prevent regressions.

### Forwards-compatibility

Rust does not currently have a formal memory model. As such, while Rust provides
guarantees about the semantics of some operations, the semantics of many
operations is up in the air and subject to change.

Zerocopy strives to ensure that our code - and code emitted by our custom
derives - is sound under any version of Rust as early as our MSRV, and will
continue to be sound under any future version of Rust. The policies in this
section are designed to help ensure that we live up to this goal.

### Safety comments

Each non-test `unsafe` block must be annotated with a "safety comment" which
provides a rationale for its soundness. In order to ensure that our soundness is
forwards-compatible, safety comments must satisfy the following criteria:
- Safety comments must constitute a (possibly informal) proof that all of Rust's
  soundness rules are upheld.
- Safety comments must only rely for their correctness on statements which
  appear in the stable versions of the [Rust Reference] or standard library
  documentation (ie, the docs for [core], [alloc], and [std]); arguments which
  rely on text from the beta or nightly versions of these documents are not
  considered complete.
- All statements from the Reference or standard library documentation which are
  relied upon for soundness must be quoted in the safety comment. This ensures
  that there is no ambiguity as to what aspect of the text is being cited. This
  is especially important in cases where the text of these documents changes in
  the future. Such changes are of course required to be backwards-compatible,
  but may change the manner in which a particular guarantee is explained.

We use the [`clippy::undocumented_unsafe_blocks`] lint to ensure that `unsafe`
blocks cannot be added without a safety comment. Note that there are a few
outstanding uncommented `unsafe` blocks which are tracked in [#429]. Our goal is
to reach 100% safety comment coverage and not regress once we've reached it.

[Rust Reference]: https://doc.rust-lang.org/reference/
[core]: https://doc.rust-lang.org/stable/core/
[alloc]: https://doc.rust-lang.org/stable/alloc/
[std]: https://doc.rust-lang.org/stable/std/
[`clippy::undocumented_unsafe_blocks`]: https://rust-lang.github.io/rust-clippy/master/index.html#/undocumented_unsafe_blocks
[#429]: https://github.com/google/zerocopy/issues/429

#### Exceptions to our safety comment policy

In rare circumstances, the soundness of an `unsafe` block may depend upon
semantics which are widely agreed upon but not formally guaranteed. In order to
avoid slowing down zerocopy's development to an unreasonable degree, a safety
comment may violate our safety comment policy so long as all of the following
hold:
- The safety comment's correctness may rely on semantics which are not
  guaranteed in official Rust documentation *so long as* a member of the Rust
  team has articulated in an official communication (e.g. a comment on a Rust
  GitHub repo) that Rust intends to guarantee particular semantics.
- There exists an active effort to formalize the guarantee in Rust's official
  documentation.

### Target architecture support

Zerocopy bases its soundness on guarantees made about the semantics of Rust
which appear in the Rust Reference or standard library documentation; zerocopy
is sound so long as these guarantees hold. There are known cases in which these
guarantees do not hold on certain target architectures (see
[rust-lang/unsafe-code-guidelines#461]); on such target architectures, zerocopy
may be unsound. We consider it outside of zerocopy's scope to reason about these
cases. Zerocopy makes no effort maintain soundness in cases where Rust's
documented guarantees do not hold.

[rust-lang/unsafe-code-guidelines#461]: https://github.com/rust-lang/unsafe-code-guidelines/issues/461

## MSRV

<!-- Our policy used to be simply that MSRV was a breaking change in all
circumstances. This implicitly relied on syn having the same MSRV policy, which
it does not. See #1085 and #1088. -->

Without the `derive` feature enabled, zerocopy's minimum supported Rust version
(MSRV) is encoded the `package.rust-version` field in its `Cargo.toml` file. For
zerocopy, we consider an increase in MSRV to be a semver-breaking change, and
will only increase our MSRV during semver-breaking version changes (e.g., 0.1 ->
0.2, 1.0 -> 2.0, etc).

For zerocopy with the `derive` feature enabled, and for the zerocopy-derive
crate, we inherit the maximum MSRV any of our dependencies. As of this writing
(2024-10-03), at least one dependency (syn) does *not* consider MSRV increases
to be semver-breaking changes. Thus, using the `derive` feature may result in
the effective MSRV increasing within a semver version train.

## Yanking

Whenever a bug or regression is identified, we will yank any affected versions
which are part of the current version train. For example, if the most recent
version is 0.10.20 and a bug is uncovered, we will release a fix in 0.10.21 and
yank all 0.10.X versions which are affected. We *may* also yank versions in
previous version trains on a case-by-case basis, but we don't guarantee it.

For information about a particular yanked or un-yanked version, see our [yank
log][yank-log].

[yank-log]: https://github.com/google/zerocopy/blob/main/CHANGELOG.md#yanks-and-regressions
