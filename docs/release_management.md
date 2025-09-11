# Release Management

Currently, we made Codex binaries available in three places:

- GitHub Releases https://github.com/openai/codex/releases/
- `@openai/codex` on npm: https://www.npmjs.com/package/@openai/codex
- `codex` on Homebrew: https://formulae.brew.sh/formula/codex

# Cutting a Release

Run the `codex-rs/scripts/create_github_release` script in the repository to publish a new release. The script will choose the appropriate version number depending on the type of release you are creating.

To cut a new alpha release from `main` (feel free to cut alphas liberally):

```
./codex-rs/scripts/create_github_release --publish-alpha
```

To cut a new _public_ release from `main` (which requires more caution), run:

```
./codex-rs/scripts/create_github_release --publish-release
```

TIP: Add the `--dry-run` flag to report the next version number for the respective release and exit.

Running the publishing script will kick off a GitHub Action to build the release, so go to https://github.com/openai/codex/actions/workflows/rust-release.yml to find the corresponding workflow. (Note: we should automate finding the workflow URL with `gh`.)

When the workflow finishes, the GitHub Release is "done," but you still have to consider npm and Homebrew.

## Publishing to npm

The GitHub Action is responsible for publishing to npm.

## Publishing to Homebrew

For Homebrew, we are properly set up with their automation system, so every few hours or so it will check our GitHub repo to see if there is a new release. When it finds one, it will put up a PR to create the equivalent Homebrew release, which entails building Codex CLI from source on various versions of macOS.

Inevitably, you just have to refresh this page periodically to see if the release has been picked up by their automation system:

https://github.com/Homebrew/homebrew-core/pulls?q=%3Apr+codex

Once everything builds, a Homebrew admin has to approve the PR. Again, the whole process takes several hours and we don't have total control over it, but it seems to work pretty well.

For reference, our Homebrew formula lives at:

https://github.com/Homebrew/homebrew-core/blob/main/Formula/c/codex.rb
