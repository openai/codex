# Contributing to anstyle

Thanks for wanting to contribute! There are many ways to contribute and we
appreciate any level you're willing to do.

## Feature Requests

Need some new functionality to help?  You can let us know by opening an
[issue][new issue]. It's helpful to look through [all issues][all issues] in
case it's already being talked about.

## Bug Reports

Please let us know about what problems you run into, whether in behavior or
ergonomics of API.  You can do this by opening an [issue][new issue]. It's
helpful to look through [all issues][all issues] in case it's already being
talked about.

## Pull Requests

Looking for an idea? Check our [issues][issues]. If the issue looks open ended,
it is probably best to post on the issue how you are thinking of resolving the
issue so you can get feedback early in the process. We want you to be
successful and it can be discouraging to find out a lot of re-work is needed.

Already have an idea?  It might be good to first [create an issue][new issue]
to propose it so we can make sure we are aligned and lower the risk of having
to re-work some of it and the discouragement that goes along with that.

### Process

As a heads up, we'll be running your PR through the following gauntlet:
- warnings turned to compile errors
- `cargo test`
- `rustfmt`
- `clippy`
- `rustdoc`
- [`committed`](https://github.com/crate-ci/committed) as we use [Conventional](https://www.conventionalcommits.org) commit style
- [`typos`](https://github.com/crate-ci/typos) to check spelling

Not everything can be checked automatically though.

We request that the commit history gets cleaned up.

We ask that commits are atomic, meaning they are complete and have a single responsibility.
A complete commit should build, pass tests, update documentation and tests, and not have dead code.

PRs should tell a cohesive story, with refactor and test commits that keep the
fix or feature commits simple and clear.

Specifically, we would encourage
- File renames be isolated into their own commit
- Add tests in a commit before their feature or fix, showing the current behavior (i.e. they should pass).
  The diff for the feature/fix commit will then show how the behavior changed,
  making the commit's intent clearer to reviewers and the community, and showing people that the
  test is verifying the expected state.
  - e.g. [clap#5520](https://github.com/clap-rs/clap/pull/5520)

Note that we are talking about ideals.
We understand having a clean history requires more advanced git skills;
feel free to ask us for help!
We might even suggest where it would work to be lax.
We also understand that editing some early commits may cause a lot of churn
with merge conflicts which can make it not worth editing all of the history.

For code organization, we recommend
- Grouping `impl` blocks next to their type (or trait)
- Grouping private items after the `pub` item that uses them.
  - The intent is to help people quickly find the "relevant" details, allowing them to "dig deeper" as needed.  Or put another way, the `pub` items serve as a table-of-contents.
  - The exact order is fuzzy; do what makes sense

## Releasing

Pre-requisites
- Running `cargo login`
- A member of `rust-cli:Maintainers`
- Push permission to the repo
- [`cargo-release`](https://github.com/crate-ci/cargo-release/)

When we're ready to release, a project owner should do the following
1. Update the changelog (see `cargo release changes` for ideas)
2. Determine what the next version is, according to semver
3. Run [`cargo release -x <level>`](https://github.com/crate-ci/cargo-release)

## Resources

- [HN post on syntax history](https://news.ycombinator.com/item?id=35137153#35157058)

[issues]: https://github.com/rust-cli/anstyle/issues
[new issue]: https://github.com/rust-cli/anstyle/issues/new
[all issues]: https://github.com/rust-cli/anstyle/issues?utf8=%E2%9C%93&q=is%3Aissue
