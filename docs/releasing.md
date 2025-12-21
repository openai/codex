## Releasing Codexel (npm)

Codexel is published as `@ixe1/codexel` with prebuilt native binaries bundled in
`codex-cli/vendor/`. Publishing is handled by GitHub Actions.

### Release checklist

- Update `codex-cli/package.json` to the release version (no `-dev` suffix).
- Merge the release commit to `main`.
- Create and push a tag:
  - Stable: `codexel-vX.Y.Z`
  - Pre-release: `codexel-vX.Y.Z-alpha.N` or `codexel-vX.Y.Z-beta.N`
- In GitHub Actions, run the `npm-publish-codexel` workflow and pass the tag you just pushed.

### What the workflow does

The `npm-publish-codexel` workflow:

- Builds the `codexel` binary for all supported targets.
- Assembles `codex-cli/vendor/<target>/codex/codexel(.exe)`.
- Packs an npm tarball and runs a smoke test (`codexel --help`).
- Creates a GitHub Release containing the per-target binaries and npm tarball.
- Publishes `@ixe1/codexel` using npm Trusted Publishing (OIDC) when `publish_npm` is enabled.
- If `HOMEBREW_TAP_GITHUB_TOKEN` is set, updates the `codexel` cask in `Ixe1/homebrew-tap`.

If you only want to publish/update Homebrew artifacts (no npm), run the `homebrew-cask-codexel` workflow manually with an existing `codexel-v*` tag.

### One-time setup

Before the first publish, configure npm Trusted Publishing for `@ixe1/codexel`
to trust this repository and the `npm-publish-codexel` workflow in the npm UI.

If you want Homebrew installs (`brew install --cask Ixe1/tap/codexel`) to track releases:

- Create the tap repository `Ixe1/homebrew-tap`.
- Add a repository secret `HOMEBREW_TAP_GITHUB_TOKEN` (a GitHub token with permission to push to `Ixe1/homebrew-tap`).
  - Recommended: a fine-grained Personal Access Token scoped to `Ixe1/homebrew-tap` with `Contents: Read and write`.
  - Create it in GitHub: `Settings -> Developer settings -> Personal access tokens -> Fine-grained tokens`.
