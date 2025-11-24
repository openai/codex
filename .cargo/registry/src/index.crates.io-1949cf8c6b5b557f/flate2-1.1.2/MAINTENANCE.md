This document explains how to perform the project's maintenance tasks.

### Repository Configuration

Some repository settings, including branch protection and required approvals, are managed in the
[Rust team repo](https://github.com/rust-lang/team/blob/master/repos/rust-lang/flate2-rs.toml).
See the [repository schema](https://github.com/rust-lang/team/blob/aaa8039a2420a07cab9bbaca3885b3ad95a256a7/docs/toml-schema.md?plain=1#L248)
for supported directives.

### Creating a new release

#### Artifacts

* a tag of the version number
* a new [crate version](https://crates.io/crates/flate2/versions)

#### Process

To generate all the artifacts above, one proceeds as follows:

1. `git checkout -b release-<next-version>` - create a branch to make changes to the repository. *Changes cannot be made to `main` as it is protected.*
2. Edit `Cargo.toml` to update the package version to `<next-version>`.
3. Create a new PR for the branch (e.g. using `gh pr create`) and **get it approved and merged**.
4. Pull the `main` branch and ensure that you are at the merged commit.
4. `cargo publish` to create a new release on `crates.io`.
5. `git tag <next-version>` to remember the commit.
6. `git push --tags` to push the new tag.
7. Go to the newly created release page on GitHub and edit it by pressing the "Generate Release Notes" and the `@` button. Save the release.
