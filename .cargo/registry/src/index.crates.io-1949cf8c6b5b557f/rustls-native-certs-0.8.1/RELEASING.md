# Making a rustls-native-certs release

This is a checklist for steps to make before/after making a rustls release.

1. Attend to the README.md: this appears on crates.io for the release, and can't be edited after
   the fact.
   - Ensure the version has a good set of release notes.  Move old release notes to OLDCHANGES.md
     if this is getting excessively long.
   - Write the version and date of the release.
2. Run `cargo update` followed by `cargo outdated`, to check if we have any
   dependency updates which are not already automatically taken by their semver specs.
   - If we do, take them if possible with separate commits (but there should've been
     dependabot PRs submitted for these already.)
3. Now run `cargo test --all-features` to ensure our tests continue to pass with the
   updated dependencies.
4. Update `Cargo.toml` to set the correct version.
5. Make a commit with the above changes, something like 'Prepare $VERSION'.  This
   should not contain functional changes: just versions numbers, and markdown changes.
6. Do a dry run: check `cargo publish --dry-run`
7. Push the above commit.  Wait for CI to confirm it as green.
   - Any red _should_ naturally block the release.
   - If rustc nightly is broken, this _may_ be acceptable if the reason is understood
     and does not point to a defect.
8. Tag the released version: `git tag -m '0.20.0' v/0.20.0`
9. Push the tag: `git push --tags`
10. Do the release: `cargo publish`.
