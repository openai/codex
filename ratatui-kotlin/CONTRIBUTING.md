# Contribution guidelines

First off, thank you for considering contributing to Ratatui.

If your contribution is not straightforward, please first discuss the change you wish to make by
creating a new issue before making the change, or starting a discussion on
[discord](https://discord.gg/pMCEU9hNEj).

## AI Generated Content

We welcome high quality PRs, whether they are human generated or made with the assistance of AI
tools, but we ask that you follow these guidelines:

- **Attribution**: Tell us about your use of AI tools, don't make us guess whether you're using it.
- **Review**: Make sure you review every line of AI generated content for correctness and relevance.
- **Quality**: AI-generated content should meet the same quality standards as human-written content.
- **Quantity**: Avoid submitting large amounts of AI generated content in a single PR. Remember that
  quality is usually more important than quantity.
- **License**: Ensure that the AI-generated content is compatible with Ratatui's
  [License](./LICENSE).

> [!IMPORTANT]
> Remember that AI tools can assist in generating content, but you are responsible for the final
> quality and accuracy of the contributions and communicating about it.

## Reporting issues

Before reporting an issue on the [issue tracker](https://github.com/ratatui/ratatui/issues),
please check that it has not already been reported by searching for some related keywords. Please
also check [`tui-rs` issues](https://github.com/fdehau/tui-rs/issues/) and link any related issues
found.

## Pull requests

All contributions are obviously welcome. Please include as many details as possible in your PR
description to help the reviewer (follow the provided template). Make sure to highlight changes
which may need additional attention, or you are uncertain about. Any idea with a large scale impact
on the crate or its users should ideally be discussed in a "Feature Request" issue beforehand.

### Keep PRs small, intentional, and focused

Try to do one pull request per change. The time taken to review a PR grows exponentially with the size
of the change. Small focused PRs will generally be much faster to review. PRs that include both
refactoring (or reformatting) with actual changes are more difficult to review as every line of the
change becomes a place where a bug may have been introduced. Consider splitting refactoring /
reformatting changes into a separate PR from those that make a behavioral change, as the tests help
guarantee that the behavior is unchanged.

Guidelines for PR size:

- Aim for PRs under 500 lines of changes when possible.
- Split large features into incremental PRs that build on each other.
- Separate refactoring, formatting, and functional changes into different PRs.
- If a large PR is unavoidable, clearly explain why in the PR description.

### Breaking changes and backwards compatibility

We prioritize maintaining backwards compatibility and minimizing disruption to users:

- **Prefer deprecation over removal**: Add deprecation warnings rather than immediately removing
  public APIs
- **Provide migration paths**: Include clear upgrade instructions for any breaking changes
- **Follow our deprecation policy**: Wait at least two versions before removing deprecated items
- **Consider feature flags**: Use feature flags for experimental or potentially disruptive changes
- **Document breaking changes**: Clearly mark breaking changes in commit messages and PR descriptions

See our [deprecation notice policy](#deprecation-notice) for more details.

### Code formatting

Run `cargo xtask format` before committing to ensure that code is consistently formatted with
rustfmt. Configuration is in [`rustfmt.toml`](./rustfmt.toml). We use some unstable formatting
options as they lead to subjectively better formatting. These require a nightly version of Rust
to be installed when running rustfmt. You can install the nightly version of Rust using
[`rustup`](https://rustup.rs/):

```shell
rustup install nightly
```

> [!IMPORTANT]
> Do not modify formatting configuration (`rustfmt.toml`, `.clippy.toml`) without
> prior discussion. These changes affect all contributors and should be carefully considered.

### Search `tui-rs` for similar work

The original fork of Ratatui, [`tui-rs`](https://github.com/fdehau/tui-rs/), has a large amount of
history of the project. Please search, read, link, and summarize any relevant
[issues](https://github.com/fdehau/tui-rs/issues/),
[discussions](https://github.com/fdehau/tui-rs/discussions/) and [pull
requests](https://github.com/fdehau/tui-rs/pulls).

### Use conventional commits

We use [conventional commits](https://www.conventionalcommits.org/en/v1.0.0/) and check for them as
a lint build step. To help adhere to the format, we recommend to install
[Commitizen](https://commitizen-tools.github.io/commitizen/). By using this tool you automatically
follow the configuration defined in [.cz.toml](.cz.toml). Your commit messages should have enough
information to help someone reading the [CHANGELOG](./CHANGELOG.md) understand what is new just from
the title. The summary helps expand on that to provide information that helps provide more context,
describes the nature of the problem that the commit is solving and any unintuitive effects of the
change. It's rare that code changes can easily communicate intent, so make sure this is clearly
documented.

### Run CI tests before pushing a PR

Running `cargo xtask ci` before pushing will perform the same checks that we do in the CI process.
It's not mandatory to do this before pushing, however it may save you time to do so instead of
waiting for GitHub to run the checks.

Available xtask commands:

- `cargo xtask ci` - Run all CI checks
- `cargo xtask format` - Format code
- `cargo xtask lint` - Run linting checks
- `cargo xtask test` - Run all tests

Run `cargo xtask --help` to see all available commands.

### Sign your commits

We use commit signature verification, which will block commits from being merged via the UI unless
they are signed. To set up your machine to sign commits, see [managing commit signature
verification](https://docs.github.com/en/authentication/managing-commit-signature-verification/about-commit-signature-verification)
in GitHub docs.

### Configuration and build system changes

Changes to project configuration files require special consideration:

- Linting configuration (`.clippy.toml`, `rustfmt.toml`): Affects all contributors.
- CI configuration (`.github/workflows/`): Affects build and deployment.
- Build system (`xtask/`, `Cargo.toml` workspace config): Affects development workflow.
- Dependencies: Consider MSRV compatibility and licensing.

Please discuss these changes in an issue before implementing them.

### Collaborative development

We may occasionally make changes directly to your branch—such as force-pushes—to help move a PR
forward, speed up review, or ensure it meets our quality standards. If you would prefer we do not do
this, or if your workflow depends on us avoiding force-pushes (for example, if your app points to
your branch in `Cargo.toml`), please mention this in your PR description and we will respect your
preference.

## Implementation Guidelines

### Setup

TL;DR: Clone the repo and build it using `cargo xtask`.

Ratatui is an ordinary Rust project where common tasks are managed with
[cargo-xtask](https://github.com/matklad/cargo-xtask). It wraps common `cargo` commands with sane
defaults depending on your platform of choice. Building the project should be as easy as running
`cargo xtask build`.

```shell
git clone https://github.com/ratatui/ratatui.git
cd ratatui
cargo xtask build
```

### Architecture

For an understanding of the crate organization and design decisions, see [ARCHITECTURE.md]. This
document explains the modular workspace structure introduced in version 0.30.0 and provides
guidance on which crate to use for different use cases.

When making changes, consider:

- Which crate should contain your changes per the modular structure,
- Whether your changes affect the public API of `ratatui-core` (requires extra care),
- And how your changes fit into the overall architecture.

[ARCHITECTURE.md]: https://github.com/ratatui/ratatui/blob/main/ARCHITECTURE.md

### Tests

The [test coverage](https://app.codecov.io/gh/ratatui/ratatui) of the crate is reasonably
good, but this can always be improved. Focus on keeping the tests simple and obvious and write unit
tests for all new or modified code. Beside the usual doc and unit tests, one of the most valuable
test you can write for Ratatui is a test against the `TestBackend`. It allows you to assert the
content of the output buffer that would have been flushed to the terminal after a given draw call.
See `widgets_block_renders` in [ratatui/tests/widgets_block.rs](./ratatui/tests/widgets_block.rs) for an example.

When writing tests, generally prefer to write unit tests and doc tests directly in the code file
being tested rather than integration tests in the `tests/` folder.

If an area that you're making a change in is not tested, write tests to characterize the existing
behavior before changing it. This helps ensure that we don't introduce bugs to existing software
using Ratatui (and helps make it easy to migrate apps still using `tui-rs`).

> [!IMPORTANT]
> Do not remove existing tests without clear justification. If tests need to be
> modified due to API changes, explain why in your PR description.

For coverage, we have two [bacon](https://dystroy.org/bacon/) jobs (one for all tests, and one for
unit tests, keyboard shortcuts `v` and `u` respectively) that run
[cargo-llvm-cov](https://github.com/taiki-e/cargo-llvm-cov) to report the coverage. Several plugins
exist to show coverage directly in your editor. E.g.:

- <https://marketplace.visualstudio.com/items?itemName=ryanluker.vscode-coverage-gutters>
- <https://github.com/alepez/vim-llvmcov>

### Documentation

Here are some guidelines for writing documentation in Ratatui.

Every public API **must** be documented.

Keep in mind that Ratatui tends to attract beginner Rust users that may not be familiar with Rust
concepts.

#### Content

The main doc comment should talk about the general features that the widget supports and introduce
the concepts pointing to the various methods. Focus on interaction with various features and giving
enough information that helps understand why you might want something.

Examples should help users understand a particular usage, not test a feature. They should be as
simple as possible. Prefer hiding imports and using wildcards to keep things concise. Some imports
may still be shown to demonstrate a particular non-obvious import (e.g. `Stylize` trait to use style
methods). Speaking of `Stylize`, you should use it over the more verbose style setters:

```rust
let style = Style::new().red().bold();
// not
let style = Style::default().fg(Color::Red).add_modifier(Modifiers::BOLD);
```

#### Format

- First line is summary, second is blank, third onward is more detail

```rust
/// Summary
///
/// A detailed description
/// with examples.
fn foo() {}
```

- Max line length is 100 characters
See [VS Code rewrap extension](https://marketplace.visualstudio.com/items?itemName=stkb.rewrap)

- Doc comments are above macros
i.e.

```rust
/// doc comment
#[derive(Debug)]
struct Foo {}
```

- Code items should be between backticks
i.e. ``[`Block`]``, **NOT** ``[Block]``

### Deprecation notice

We generally want to wait at least two versions before removing deprecated items, so users have
time to update. However, if a deprecation is blocking for us to implement a new feature we may
*consider* removing it in a one version notice.

Deprecation process:

1. Add `#[deprecated]` attribute with a clear message.
2. Update documentation to point to the replacement.
3. Add an entry to `BREAKING-CHANGES.md` if applicable.
4. Wait at least two versions before removal.
5. Consider the impact on the ecosystem before removing.

### Use of unsafe for optimization purposes

We don't currently use any unsafe code in Ratatui, and would like to keep it that way. However, there
may be specific cases that this becomes necessary in order to avoid slowness. Please see [this
discussion](https://github.com/ratatui/ratatui/discussions/66) for more about the decision.

## Continuous Integration

We use GitHub Actions for the CI where we perform the following checks:

- The code should compile on `stable` and the Minimum Supported Rust Version (MSRV).
- The tests (docs, lib, tests and examples) should pass.
- The code should conform to the default format enforced by `rustfmt`.
- The code should not contain common style issues `clippy`.

You can also check most of those things yourself locally using `cargo xtask ci` which will offer you
a shorter feedback loop than pushing to github.

## Relationship with `tui-rs`

This project was forked from [`tui-rs`](https://github.com/fdehau/tui-rs/) in February 2023, with the
[blessing of the original author](https://github.com/fdehau/tui-rs/issues/654), Florian Dehau
([@fdehau](https://github.com/fdehau)).

The original repository contains all the issues, PRs, and discussion that were raised originally, and
it is useful to refer to when contributing code, documentation, or issues with Ratatui.

We imported all the PRs from the original repository, implemented many of the smaller ones, and
made notes on the leftovers. These are marked as draft PRs and labelled as [imported from
tui](https://github.com/ratatui/ratatui/pulls?q=is%3Apr+is%3Aopen+label%3A%22imported+from+tui%22).
We have documented the current state of those PRs, and anyone is welcome to pick them up and
continue the work on them.

We have not imported all issues opened on the previous repository. For that reason, anyone wanting
to **work on or discuss** an issue will have to follow the following workflow:

- Recreate the issue
- Start by referencing the **original issue**: ```Referencing issue #[<issue number>](<original
  issue link>)```
- Then, paste the original issues **opening** text

You can then resume the conversation by replying to this new issue you have created.
