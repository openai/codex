# What to do to publish a new release

1. Ensure all notable changes are in the changelog under "Unreleased".

2. Execute `cargo release <level>` to bump version(s), tag and publish
   everything. External subcommand, must be installed with `cargo install
   cargo-release`.
   
   `<level>` can be one of `major|minor|patch`. If this is the first release
   (`0.1.0`), use `minor`, since the version starts out as `0.0.0`.

3. Go to the GitHub releases, edit the just-pushed tag. Copy the release notes
   from the changelog.
