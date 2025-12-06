# Creating a Release

Our release strategy is:

> Release major versions with detailed summaries when necessary, while releasing minor versions
> weekly or as needed without extensive announcements.
>
> Versioning scheme being `0.x.y`, where `x` is the major version and `y` is the minor version.

[crates.io](https://crates.io/crates/ratatui) releases are automated via [GitHub
actions](.github/workflows/cd.yml) and triggered by pushing a tag.

1. Record a new demo gif if necessary. The preferred tool for this is
[vhs](https://github.com/charmbracelet/vhs) (installation instructions in README).

   ```shell
   cargo build --example demo2
   vhs examples/demo2.tape
   ```

1. Switch branches to the images branch and copy demo2.gif to examples/, commit, and push.
1. Grab the permalink from <https://github.com/ratatui/ratatui/blob/images/examples/demo2.gif> and
   append `?raw=true` to redirect to the actual image url. Then update the link in the main README.
   Avoid adding the gif to the git repo as binary files tend to bloat repositories.

1. Bump the version in [Cargo.toml](Cargo.toml).
1. Ensure [CHANGELOG.md](CHANGELOG.md) is updated. [git-cliff](https://github.com/orhun/git-cliff)
   can be used for generating the entries.
1. Ensure that any breaking changes are documented in [BREAKING-CHANGES.md](./BREAKING-CHANGES.md)
1. Commit and push the changes.
1. Create a new tag: `git tag -a v[0.x.y]`
1. Push the tag: `git push --tags`
1. Wait for [Continuous Deployment](https://github.com/ratatui/ratatui/actions) workflow to
   finish.

## Alpha Releases

Alpha releases are automatically released every Saturday via [cd.yml](./.github/workflows/cd.yml)
and can be manually created when necessary by triggering the [Continuous
Deployment](https://github.com/ratatui/ratatui/actions/workflows/cd.yml) workflow.

We automatically release an alpha release with a patch level bump + alpha.num weekly (and when we
need to manually). E.g. the last release was 0.22.0, and the most recent alpha release is
0.22.1-alpha.1.

These releases will have whatever happened to be in main at the time of release, so they're useful
for apps that need to get releases from crates.io, but may contain more bugs and be generally less
tested than normal releases.

See [#147](https://github.com/ratatui/ratatui/issues/147) and
[#359](https://github.com/ratatui/ratatui/pull/359) for more info on the alpha release process.
